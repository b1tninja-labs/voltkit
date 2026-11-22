//! Find the adapters the wizard can offer, so the operator selects one instead
//! of typing a COM port.
//!
//! A [`Device`] is one selectable adapter: a Windows COM port with its friendly
//! name, a VCX-NANO identified by a GetInfo read, an OBDLink MX found natively over
//! the host Bluetooth radio, or a remembered Wi-Fi/TCP target. [`Link`] is how a
//! chosen device is reached: a TCP address, a COM port and baud, a VCX USB port, or
//! a BLE device address opened in-process through the `dangle` radio library.
//!
//! The device enumeration touches hardware and is gated by platform and feature.
//! The labelling, the BLE capture target, and the auto-naming are pure and tested
//! without a device: a BLE address becomes a capture target by [`ble_target`] (and
//! back by [`parse_ble_target`]), a capture file is named by [`auto_profile_path`],
//! and a chosen link becomes a capture target by [`Link::target`].

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
    /// An OBDLink MX (or any FFF0-style BLE serial dongle) reached natively over the
    /// host Bluetooth radio.
    ///
    /// voltkit opens the dongle in-process through the `dangle` radio library at this
    /// 48-bit Bluetooth device address and drives the ELM327 session over the GATT
    /// serial service. No process is spawned and no TCP socket is used.
    Ble {
        /// The advertiser name, for the summary.
        name: String,
        /// The 48-bit Bluetooth device address to connect to.
        address: u64,
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
    /// A [`Link::Ble`] captures natively over the host radio; its target is the
    /// `ble:<address>` form [`ble_target`] produces, which [`crate::Session`] opens
    /// in-process. A [`Link::Vcx`] does not capture, so it has no target.
    #[must_use]
    pub fn target(&self) -> Option<(String, Option<u32>)> {
        match self {
            Link::Tcp(addr) => Some((addr.clone(), None)),
            Link::Serial { port, baud } => Some((port.clone(), *baud)),
            Link::Ble { address, .. } => Some((ble_target(*address), None)),
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
}

/// The `voltkit profile` target string for a BLE device at `address`.
///
/// The form is `ble:AA:BB:CC:DD:EE:FF`. The `ble:` scheme tells the capture path to
/// open the dongle natively over the host radio rather than dial a socket or a COM
/// port; [`parse_ble_target`] reads it back to the 48-bit address. This is pure, so
/// the round-trip is tested without a radio.
#[must_use]
pub fn ble_target(address: u64) -> String {
    let b = address.to_be_bytes();
    format!(
        "ble:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        b[2], b[3], b[4], b[5], b[6], b[7]
    )
}

/// The 48-bit Bluetooth address of a `ble:AA:BB:CC:DD:EE:FF` target, or `None`.
///
/// This is the inverse of [`ble_target`]: the capture path uses it to recover the
/// address a native BLE connect needs. Any target without the `ble:` scheme, or with
/// the wrong number of hex digits, yields `None`.
#[must_use]
pub fn parse_ble_target(target: &str) -> Option<u64> {
    let rest = target.strip_prefix("ble:")?;
    let hex: String = rest.chars().filter(|c| *c != ':').collect();
    if hex.len() != 12 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u64::from_str_radix(&hex, 16).ok()
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

/// One selectable native-BLE device for a serial-service advertiser.
///
/// `name` is the advertised name (or `<unnamed>`), `address` the 48-bit Bluetooth
/// address the native connect uses, and `rssi` the signal in dBm. The device carries
/// its address in a [`Link::Ble`], so picking it opens the dongle over the host radio
/// directly. This is pure: the label, detail, and link are built without a radio, so
/// the scan's host-specific half only has to map its advertisers onto it.
#[must_use]
pub fn ble_device(name: &str, address: u64, rssi: i16) -> Device {
    let name = if name.is_empty() { "<unnamed>" } else { name };
    Device::new(
        format!("{name}  (Bluetooth LE)"),
        format!(
            "Phase 1 — module clone (ELM327 over native BLE). {} {rssi} dBm",
            ble_target(address).trim_start_matches("ble:")
        ),
        Link::Ble {
            name: name.to_string(),
            address,
        },
    )
}

/// The MX devices the host Bluetooth radio sees right now, or none off a build that
/// cannot scan.
///
/// This runs `dangle`'s advertisement watcher in-process ([`dangle::scan_advertisers`])
/// and keeps only the advertisers carrying the serial GATT service (default `FFF0`),
/// strongest signal first. It opens no GATT connection and binds no socket — the scan
/// is a passive BLE listen. Built without the `ble` feature (or off Windows) there is
/// no radio, so this yields nothing.
#[cfg(all(windows, feature = "ble"))]
#[must_use]
pub fn ble_devices(scan_timeout: Duration) -> Vec<Device> {
    let Ok(adverts) = dangle::scan_advertisers(
        scan_timeout,
        &dangle::ScanFilter::default(),
        dangle::DEFAULT_SERVICE_UUID,
    ) else {
        return Vec::new();
    };
    adverts
        .iter()
        .filter(|a| a.has_serial_service)
        .map(|a| ble_device(&a.name, a.address, a.rssi))
        .collect()
}

/// No native BLE backend in this build (off Windows, or the `ble` feature is off),
/// so there is no radio to scan: this yields nothing.
#[cfg(not(all(windows, feature = "ble")))]
#[must_use]
pub fn ble_devices(_scan_timeout: Duration) -> Vec<Device> {
    Vec::new()
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
/// identified, then the MX devices the host Bluetooth radio saw, then the remembered
/// Wi-Fi/TCP target. The list is empty of hardware on a platform or build that
/// cannot look, and the wizard always offers manual entry beside it.
///
/// `scan_timeout` bounds the native BLE scan; `remembered` is the last TCP address.
#[must_use]
pub fn devices(scan_timeout: Duration, remembered: Option<&str>) -> Vec<Device> {
    let serial = serial_devices();
    let vcx = vcx_devices(&serial);
    let ble = ble_devices(scan_timeout);
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
            "Phase 1 — module clone. The remembered adapter address".to_string(),
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
                "Phase 1 — module clone (ELM327 over serial/USB)".to_string(),
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
        if let Some(info) = vcx_doctor(port) {
            out.push(Device::new(
                format!("{}  (VCX-NANO on COM{port})", info.hw_name),
                format!(
                    "Phase 2 — programming-data staging. firmware {}.{}.{}.{}, {} — doctor only, live SPS read gated",
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
/// sends a vehicle read. `None` means no VCX-NANO answered. The SPS collect
/// staging reuses this doctor; the live SPS read stays gated in vcxtoolkit.
#[cfg(all(windows, feature = "vcx"))]
pub fn vcx_doctor(port: u32) -> Option<vcxtoolkit::DeviceInfo> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ble_target_round_trips_through_its_address() {
        // The capture target carries the 48-bit address as `ble:AA:BB:...`, and the
        // capture path reads it back without a radio.
        let addr = 0x0000_AABB_CCDD_EEFF;
        assert_eq!(ble_target(addr), "ble:AA:BB:CC:DD:EE:FF");
        assert_eq!(parse_ble_target("ble:AA:BB:CC:DD:EE:FF"), Some(addr));
        assert_eq!(parse_ble_target("ble:aabbccddeeff"), Some(addr));
        // Anything that is not a `ble:` target of 12 hex digits is not one.
        assert_eq!(parse_ble_target("127.0.0.1:35000"), None);
        assert_eq!(parse_ble_target("COM5"), None);
        assert_eq!(parse_ble_target("ble:AA:BB"), None);
        assert_eq!(parse_ble_target("ble:ZZBBCCDDEEFF"), None);
    }

    #[test]
    fn a_serial_advertiser_becomes_a_native_ble_device() {
        let device = ble_device("OBDLink MX", 0x0000_AABB_CCDD_EEFF, -58);
        assert_eq!(
            device.link,
            Link::Ble {
                name: "OBDLink MX".to_string(),
                address: 0x0000_AABB_CCDD_EEFF,
            }
        );
        assert_eq!(
            device.link.target(),
            Some(("ble:AA:BB:CC:DD:EE:FF".to_string(), None))
        );
        assert!(device.link.capturable());
        assert!(device.label.contains("OBDLink MX"));
        assert!(device.label.contains("Bluetooth LE"));
        assert!(device.detail.contains("AA:BB:CC:DD:EE:FF"));
        assert!(device.detail.contains("-58 dBm"));
        // An unnamed advertiser still names a device.
        assert!(ble_device("", 1, -90).label.contains("<unnamed>"));
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
        assert_eq!(
            Link::Ble {
                name: "OBDLink MX".to_string(),
                address: 0x0000_AABB_CCDD_EEFF,
            }
            .target(),
            Some(("ble:AA:BB:CC:DD:EE:FF".to_string(), None))
        );
        let vcx = Link::Vcx { port: 4 };
        assert_eq!(vcx.target(), None);
        assert!(!vcx.capturable());
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
        // without opening a COM port or a radio.
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
            Link::Ble {
                name: "OBDLink MX".to_string(),
                address: 0x0000_AABB_CCDD_EEFF,
            },
        )];
        let found = assemble(vcx, serial, ble, Some("127.0.0.1:35000"));
        let kinds: Vec<&str> = found
            .iter()
            .map(|d| match d.link {
                Link::Vcx { .. } => "vcx",
                Link::Serial { .. } => "serial",
                Link::Ble { .. } => "ble",
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
