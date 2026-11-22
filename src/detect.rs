//! Find the adapters the wizard can offer, so the operator selects one instead
//! of typing a COM port.
//!
//! A [`Device`] is one selectable adapter: a Windows COM port with its friendly
//! name, a VCX-NANO identified by a GetInfo read, an OBDLink MX seen over BLE by
//! `obd-bridge scan`, or a remembered Wi-Fi/TCP target. [`Link`] is how a chosen
//! device is reached: a TCP address, a COM port and baud, a VCX USB port, or the
//! loopback socket that `obd-bridge` republishes an MX on.
//!
//! The device enumeration touches hardware and is gated by platform and feature.
//! The parsing, labelling, and auto-naming are pure and tested without a device:
//! `obd-bridge scan` output is parsed by [`parse_bridge_scan`], a capture file is
//! named by [`auto_profile_path`], and a chosen link becomes a capture target by
//! [`Link::target`].

use std::path::{Path, PathBuf};
use std::time::Duration;

/// How a chosen [`Device`] is reached for a capture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Link {
    /// A `HOST:PORT` ELM327-over-TCP peer (Wi-Fi, or a loopback bridge).
    Tcp(String),
    /// A Windows COM port and the baud to open it at. `None` keeps 115200.
    Serial {
        /// The `COMn` name.
        port: String,
        /// The baud, or `None` for 115200.
        baud: Option<u32>,
    },
    /// An OBDLink MX reached through a managed `obd-bridge` on the loopback socket.
    ///
    /// voltkit spawns `obd-bridge --name OBDLink`, which dials the BLE radio, and
    /// connects to [`crate::bench::ADDRESS`]. voltkit does not open the radio.
    BleBridge {
        /// The advertiser name, for the summary.
        name: String,
    },
    /// A VCX-NANO on a USB COM port number. Identify-only in the wizard: the live
    /// vehicle read stays gated in vcxtoolkit, so this link does not capture.
    Vcx {
        /// The `COMn` number `open_usb` takes.
        port: u32,
    },
}

impl Link {
    /// The `voltkit profile` target and baud for this link.
    ///
    /// A [`Link::BleBridge`] captures through the managed bridge on
    /// [`crate::bench::ADDRESS`]. A [`Link::Vcx`] does not capture, so it has no
    /// target.
    #[must_use]
    pub fn target(&self) -> Option<(String, Option<u32>)> {
        match self {
            Link::Tcp(addr) => Some((addr.clone(), None)),
            Link::Serial { port, baud } => Some((port.clone(), *baud)),
            Link::BleBridge { .. } => Some((crate::bench::ADDRESS.to_string(), None)),
            Link::Vcx { .. } => None,
        }
    }

    /// Whether choosing this link runs a capture.
    ///
    /// Every link captures except [`Link::Vcx`], whose live read is gated.
    #[must_use]
    pub fn capturable(&self) -> bool {
        !matches!(self, Link::Vcx { .. })
    }

    /// Whether voltkit spawns `obd-bridge` for this link before the capture.
    #[must_use]
    pub fn managed_bridge(&self) -> bool {
        matches!(self, Link::BleBridge { .. })
    }
}

/// One adapter the wizard offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    /// The line shown in the picker, such as `COM5  OBDLink Wi-Fi/USB`.
    pub label: String,
    /// A second line of detail, such as the firmware of a VCX-NANO.
    pub detail: String,
    /// How the device is reached.
    pub link: Link,
}

impl Device {
    /// A capturable device whose kind word prefixes the label.
    fn new(label: impl Into<String>, detail: impl Into<String>, link: Link) -> Self {
        Self {
            label: label.into(),
            detail: detail.into(),
            link,
        }
    }
}

/// One advertiser parsed from an `obd-bridge scan` row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BleAdvert {
    /// The `AA:BB:CC:DD:EE:FF` address.
    pub addr: String,
    /// The advertised name, or `<unnamed>`.
    pub name: String,
    /// The signal, such as `-72 dBm`.
    pub rssi: String,
    /// Whether the row was marked as carrying the serial service.
    pub serial: bool,
}

/// Parse the advertiser rows `obd-bridge scan` prints.
///
/// A doctor row is `AA:BB:CC:DD:EE:FF  -72 dBm  NAME  [FFF0, ...]` and the serial
/// advertiser also ends `<- serial service (FFF0)`. Header, count, and status
/// lines are ignored. The serial advertisers come first, then by signal, the
/// order `obd-bridge` already sorts them, so the first serial row is the MX.
#[must_use]
pub fn parse_bridge_scan(stdout: &str) -> Vec<BleAdvert> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        let Some((addr_rest, _)) = trimmed.split_once(" dBm") else {
            continue;
        };
        // The address is the first whitespace-delimited token, and it is a
        // colon-separated MAC. Anything else on this line is not a row.
        let mut tokens = addr_rest.split_whitespace();
        let Some(addr) = tokens.next() else { continue };
        if !is_mac(addr) {
            continue;
        }
        let Some(rssi_value) = tokens.next() else {
            continue;
        };
        let rssi = format!("{rssi_value} dBm");
        let serial = trimmed.contains("<- serial service");
        // The name is the text between the dBm field and the service bracket.
        let after_rssi = addr_rest
            .split_once(rssi_value)
            .map(|(_, rest)| rest)
            .unwrap_or("");
        let name_text = trimmed
            .split_once(" dBm")
            .map(|(_, rest)| rest)
            .unwrap_or(after_rssi);
        let name = name_text.split('[').next().unwrap_or("").trim().to_string();
        out.push(BleAdvert {
            addr: addr.to_string(),
            name: if name.is_empty() {
                "<unnamed>".to_string()
            } else {
                name
            },
            rssi,
            serial,
        });
    }
    out
}

fn is_mac(token: &str) -> bool {
    let parts: Vec<&str> = token.split(':').collect();
    parts.len() == 6
        && parts
            .iter()
            .all(|p| p.len() == 2 && p.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// The MX devices among the advertisers `obd-bridge scan` reported.
///
/// Only the serial-service advertisers become selectable devices, each reached
/// through the managed bridge. A scan that saw no serial advertiser yields none.
#[must_use]
pub fn ble_devices(adverts: &[BleAdvert]) -> Vec<Device> {
    adverts
        .iter()
        .filter(|a| a.serial)
        .map(|a| {
            let name = a.name.clone();
            Device::new(
                format!("{name}  (Bluetooth LE)"),
                format!("{} {}  — managed through obd-bridge", a.addr, a.rssi),
                Link::BleBridge { name },
            )
        })
        .collect()
}

/// A friendly label for a serial port and its USB product string.
///
/// `COM5  OBDLink MX` when the product is known, `COM5  USB serial` for a USB port
/// with no product, and `COM5  serial port` for anything else.
#[must_use]
pub fn serial_label(port: &str, product: Option<&str>) -> String {
    match product.map(str::trim).filter(|p| !p.is_empty()) {
        Some(name) => format!("{port}  {name}"),
        None => format!("{port}  serial port"),
    }
}

/// The capture file this run writes, under `dir`, named from the job and the time.
///
/// The name is `volt-<slug>-<YYYYMMDD-HHMMSS>.profile`, where `slug` is the job
/// reduced to lowercase words joined by `-`. A VIN is never put in the name: the
/// capture is named before any identifier is read, and a VIN belongs in the file,
/// not its path.
#[must_use]
pub fn auto_profile_path(dir: &Path, job: &str, unix_secs: u64) -> PathBuf {
    dir.join(format!(
        "volt-{}-{}.profile",
        slug(job),
        timestamp(unix_secs)
    ))
}

fn slug(job: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for ch in job.chars() {
        if ch.is_ascii_alphanumeric() {
            out.extend(ch.to_lowercase());
            dash = false;
        } else if !out.is_empty() && !dash {
            out.push('-');
            dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "capture".to_string()
    } else {
        trimmed
    }
}

/// `YYYYMMDD-HHMMSS` in UTC for a Unix timestamp, with no external crate.
#[must_use]
pub fn timestamp(unix_secs: u64) -> String {
    let days = (unix_secs / 86_400) as i64;
    let secs_of_day = unix_secs % 86_400;
    let (y, mo, d) = civil_from_days(days);
    let (h, mi, s) = (
        secs_of_day / 3_600,
        (secs_of_day % 3_600) / 60,
        secs_of_day % 60,
    );
    format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}")
}

/// Howard Hinnant's civil-from-days: the calendar date `days` after 1970-01-01.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The directory captures are written to: `<home>/voltkit-captures`, outside any
/// repository. The caller creates it before writing.
#[must_use]
pub fn default_capture_dir() -> PathBuf {
    #[allow(deprecated)]
    let base = std::env::home_dir().unwrap_or_else(std::env::temp_dir);
    base.join("voltkit-captures")
}

/// The COM-port numbers the VCX doctor probes when none are known from the serial
/// scan: `1` through `max`. The real scan feeds the ports it already enumerated.
#[must_use]
pub fn vcx_candidate_ports(max: u32) -> Vec<u32> {
    (1..=max).collect()
}

/// The parsed port number of a `COMn` name, for the VCX doctor.
#[must_use]
pub fn com_number(port: &str) -> Option<u32> {
    let upper = port.to_ascii_uppercase();
    upper.strip_prefix("COM").and_then(|n| n.parse().ok())
}

/// Every adapter the wizard can offer right now, in menu order.
///
/// Serial ports first (each with its friendly name), then any VCX-NANO the doctor
/// identified, then the MX devices `obd-bridge scan` saw, then the remembered
/// Wi-Fi/TCP target. The list is empty of hardware on a platform or build that
/// cannot look, and the wizard always offers manual entry beside it.
///
/// `scan_timeout` bounds the BLE scan; `remembered` is the last TCP address.
#[must_use]
pub fn devices(scan_timeout: Duration, remembered: Option<&str>) -> Vec<Device> {
    let serial = serial_devices();
    let vcx = vcx_devices(&serial);
    let ble = bridge_devices(scan_timeout);
    assemble(vcx, serial, ble, remembered)
}

/// Build the menu-ordered list from the parts the probes found: VCX devices, then
/// serial ports, then MX/BLE devices, then the remembered Wi-Fi/TCP target.
///
/// This is the pure half of [`devices`]: it touches no hardware, so the ordering
/// and the remembered entry are tested without a device.
#[must_use]
pub fn assemble(
    vcx: Vec<Device>,
    serial: Vec<Device>,
    ble: Vec<Device>,
    remembered: Option<&str>,
) -> Vec<Device> {
    let mut out = Vec::new();
    out.extend(vcx);
    out.extend(serial);
    out.extend(ble);
    if let Some(addr) = remembered {
        out.push(Device::new(
            format!("{addr}  (Wi-Fi / TCP)"),
            "The remembered adapter address".to_string(),
            Link::Tcp(addr.to_string()),
        ));
    }
    out
}

/// The COM ports present, each labelled with its USB product when known.
#[cfg(windows)]
fn serial_devices() -> Vec<Device> {
    let Ok(ports) = serialport::available_ports() else {
        return Vec::new();
    };
    ports
        .into_iter()
        .filter(|p| p.port_name.to_ascii_uppercase().starts_with("COM"))
        .map(|p| {
            let product = match &p.port_type {
                serialport::SerialPortType::UsbPort(info) => info.product.clone(),
                _ => None,
            };
            Device::new(
                serial_label(&p.port_name, product.as_deref()),
                "Serial / USB ELM327".to_string(),
                Link::Serial {
                    port: p.port_name.to_ascii_uppercase(),
                    baud: None,
                },
            )
        })
        .collect()
}

#[cfg(not(windows))]
fn serial_devices() -> Vec<Device> {
    Vec::new()
}

/// Any VCX-NANO the doctor identifies on the COM ports already seen.
///
/// The doctor is `open_usb` + a GetInfo read. The first read after a USB connect
/// can miss, so each port is retried. The live vehicle read is not sent; this is
/// the device query only. Built without the `vcx` feature (or off Windows), the
/// probe returns nothing.
#[cfg(all(windows, feature = "vcx"))]
fn vcx_devices(serial: &[Device]) -> Vec<Device> {
    let ports: Vec<u32> = serial
        .iter()
        .filter_map(|d| match &d.link {
            Link::Serial { port, .. } => com_number(port),
            _ => None,
        })
        .collect();
    let probe: Vec<u32> = if ports.is_empty() {
        vcx_candidate_ports(16)
    } else {
        ports
    };
    let mut out = Vec::new();
    for port in probe {
        if let Some(info) = probe_vcx(port) {
            out.push(Device::new(
                format!("{}  (VCX-NANO on COM{port})", info.hw_name),
                format!(
                    "firmware {}.{}.{}.{}, {} — identify only, live read gated",
                    info.fw_version[0],
                    info.fw_version[1],
                    info.fw_version[2],
                    info.fw_version[3],
                    info.fw_date
                ),
                Link::Vcx { port },
            ));
        }
    }
    out
}

#[cfg(not(all(windows, feature = "vcx")))]
fn vcx_devices(_serial: &[Device]) -> Vec<Device> {
    Vec::new()
}

/// One GetInfo round-trip against a candidate VCX USB port, retried.
///
/// This opens the USB serial port and sends the GetInfo request only. It never
/// sends a vehicle read. `None` means no VCX-NANO answered.
#[cfg(all(windows, feature = "vcx"))]
fn probe_vcx(port: u32) -> Option<vcxtoolkit::DeviceInfo> {
    let device = vcxtoolkit::open_usb(port, 0).ok()?;
    for _ in 0..3 {
        if let Ok(frame) = vcxtoolkit::exchange(
            &device,
            vcxtoolkit::Command::GetInfo.header(0),
            &[],
            Duration::from_secs(2),
        ) {
            if let Ok(info) = vcxtoolkit::device_info(&frame) {
                return Some(info);
            }
        }
    }
    None
}

/// The MX devices `obd-bridge scan` reports, or none when the bridge is absent.
///
/// This spawns `obd-bridge scan --scan-timeout <secs>` and parses its rows. The
/// bridge opens no vehicle; the scan is a passive BLE listen. A missing bridge or
/// a non-Windows host yields nothing.
fn bridge_devices(scan_timeout: Duration) -> Vec<Device> {
    let program = crate::bench::program(std::env::var_os(crate::bench::PROGRAM_ENV));
    let secs = scan_timeout.as_secs().max(1).to_string();
    let output = std::process::Command::new(&program)
        .args(["scan", "--scan-timeout", &secs])
        .stdin(std::process::Stdio::null())
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    ble_devices(&parse_bridge_scan(&stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scan_row_parses_into_an_advertiser() {
        let scan = "\
host Bluetooth adapter: present
scanning for 10s ...
3 BLE advertiser(s) seen (serial service FFF0 marked):
  AA:BB:CC:DD:EE:FF    -58 dBm  OBDLink MX  [FFF0, 1801]  <- serial service (FFF0)
  11:22:33:44:55:66    -77 dBm  <unnamed>  [180F]
  AB:CD:EF:01:23:45    -80 dBm  Some Watch  [-]
";
        let adverts = parse_bridge_scan(scan);
        assert_eq!(adverts.len(), 3);
        assert_eq!(adverts[0].addr, "AA:BB:CC:DD:EE:FF");
        assert_eq!(adverts[0].name, "OBDLink MX");
        assert_eq!(adverts[0].rssi, "-58 dBm");
        assert!(adverts[0].serial);
        assert_eq!(adverts[1].name, "<unnamed>");
        assert!(!adverts[1].serial);
        assert!(!adverts[2].serial);
    }

    #[test]
    fn only_the_serial_advertiser_becomes_a_managed_mx_device() {
        let scan = "\
  AA:BB:CC:DD:EE:FF    -58 dBm  OBDLink MX  [FFF0]  <- serial service (FFF0)
  11:22:33:44:55:66    -77 dBm  Headphones  [180F]
";
        let devices = ble_devices(&parse_bridge_scan(scan));
        assert_eq!(devices.len(), 1);
        assert_eq!(
            devices[0].link,
            Link::BleBridge {
                name: "OBDLink MX".to_string()
            }
        );
        assert!(devices[0].link.managed_bridge());
        assert_eq!(
            devices[0].link.target(),
            Some((crate::bench::ADDRESS.to_string(), None))
        );
        assert!(devices[0].label.contains("OBDLink MX"));
    }

    #[test]
    fn a_link_becomes_a_capture_target_except_the_gated_vcx() {
        assert_eq!(
            Link::Tcp("192.168.0.10:35000".to_string()).target(),
            Some(("192.168.0.10:35000".to_string(), None))
        );
        assert_eq!(
            Link::Serial {
                port: "COM5".to_string(),
                baud: Some(38_400)
            }
            .target(),
            Some(("COM5".to_string(), Some(38_400)))
        );
        let vcx = Link::Vcx { port: 4 };
        assert_eq!(vcx.target(), None);
        assert!(!vcx.capturable());
        assert!(!vcx.managed_bridge());
        assert!(Link::Serial {
            port: "COM5".to_string(),
            baud: None
        }
        .capturable());
    }

    #[test]
    fn the_capture_file_is_named_from_the_job_and_the_time() {
        // 1_700_000_000 is 2023-11-14 22:13:20 UTC.
        let path = auto_profile_path(Path::new("/captures"), "Copy this car", 1_700_000_000);
        let name = path.file_name().unwrap().to_string_lossy();
        assert_eq!(name, "volt-copy-this-car-20231114-221320.profile");
        assert!(path.starts_with("/captures"));
        // No job still names a file, and a VIN never reaches the name.
        assert_eq!(
            auto_profile_path(Path::new("/c"), "", 0)
                .file_name()
                .unwrap(),
            "volt-capture-19700101-000000.profile"
        );
    }

    #[test]
    fn timestamp_matches_known_dates() {
        assert_eq!(timestamp(0), "19700101-000000");
        assert_eq!(timestamp(1_700_000_000), "20231114-221320");
        assert_eq!(timestamp(1_763_000_000), "20251113-021320");
    }

    #[test]
    fn a_serial_label_uses_the_product_when_there_is_one() {
        assert_eq!(serial_label("COM5", Some("OBDLink MX")), "COM5  OBDLink MX");
        assert_eq!(serial_label("COM5", Some("  ")), "COM5  serial port");
        assert_eq!(serial_label("COM3", None), "COM3  serial port");
    }

    #[test]
    fn a_com_name_yields_its_number() {
        assert_eq!(com_number("COM4"), Some(4));
        assert_eq!(com_number("com12"), Some(12));
        assert_eq!(com_number("192.168.0.10:35000"), None);
        assert_eq!(vcx_candidate_ports(3), vec![1, 2, 3]);
    }

    #[test]
    fn the_menu_order_is_vcx_then_serial_then_ble_then_remembered() {
        // The assembly is pure, so the order and the remembered entry are tested
        // without opening a COM port, a radio, or spawning obd-bridge.
        let vcx = vec![Device::new("VCX-NANO on COM4", "", Link::Vcx { port: 4 })];
        let serial = vec![Device::new(
            "COM5  OBDLink",
            "",
            Link::Serial {
                port: "COM5".to_string(),
                baud: None,
            },
        )];
        let ble = vec![Device::new(
            "OBDLink MX  (Bluetooth LE)",
            "",
            Link::BleBridge {
                name: "OBDLink MX".to_string(),
            },
        )];
        let found = assemble(vcx, serial, ble, Some("127.0.0.1:35000"));
        let kinds: Vec<&str> = found
            .iter()
            .map(|d| match d.link {
                Link::Vcx { .. } => "vcx",
                Link::Serial { .. } => "serial",
                Link::BleBridge { .. } => "ble",
                Link::Tcp(_) => "tcp",
            })
            .collect();
        assert_eq!(kinds, ["vcx", "serial", "ble", "tcp"]);
        assert_eq!(
            found.last().unwrap().link,
            Link::Tcp("127.0.0.1:35000".to_string())
        );
        // Nothing remembered leaves only the detected devices.
        assert_eq!(assemble(Vec::new(), Vec::new(), Vec::new(), None).len(), 0);
    }

    #[cfg(not(all(windows, feature = "vcx")))]
    #[test]
    fn without_the_vcx_feature_the_probe_is_empty() {
        assert!(vcx_devices(&[Device::new(
            "COM4  thing",
            "",
            Link::Serial {
                port: "COM4".to_string(),
                baud: None
            }
        )])
        .is_empty());
    }
}
