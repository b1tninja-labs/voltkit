//! Stage the VCX programming-data collect, without running it.
//!
//! Phase 2 of the wizard detects and identifies a VCX-NANO (the safe GetInfo
//! doctor) and writes a *skeleton* of the SPS programming-data read: which device
//! would collect, and which module reads it would fill, with empty values and a
//! gated status. The live SPS vehicle read is **not** sent here — the VCX
//! pass-through `VCX_PRO_Recv` byte layout is not recovered, and `vcxtoolkit`
//! keeps that read gated for owner sign-off.
//!
//! The reads come from voltkit's own capture model (the `parts` and `hardware`
//! sets: `$1A` `C1`–`CA` part numbers, the `CB` hardware id, and the `$1A 90`
//! VIN, per programming header). voltkit does not copy spskit's controller rule
//! table (device ids, request methods); spskit maps these headers to its
//! controllers when it fills the skeleton. `collect_reads` and `skeleton_json`
//! are pure and tested without a device.

use std::collections::BTreeSet;

/// One programming-data read the SPS collect would fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpsRead {
    /// The ECU request header, such as `7E0`.
    pub header: u16,
    /// The service, always `0x1A` (GMLAN ReadDataByIdentifier) for these reads.
    pub service: u8,
    /// The identifier: `0x90` (VIN), `0xC1`–`0xCA` (part numbers), or `0xCB` (hardware).
    pub did: u16,
}

/// The identity of the VCX that would run the collect, from the GetInfo doctor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VcxIdentity {
    /// `DevInfo.HwName`, such as `VCX-NANO`.
    pub model: String,
    /// `DevInfo.FwVersion`, as `a.b.c.d`.
    pub firmware: String,
    /// `DevInfo.FwDate`, as `YYYY-MM-DD`.
    pub firmware_date: String,
    /// `DevInfo.Serial`.
    pub serial: String,
    /// The USB COM port the doctor answered on.
    pub port: u32,
}

/// The programming-data reads the SPS collect fills, grouped by programming
/// header: the VIN, the part numbers `C1`–`CA`, then the hardware id `CB`.
///
/// Drawn from voltkit's `parts` and `hardware` capture sets, so the headers and
/// identifiers are the same vehicle-derived ones the ELM path already uses.
#[must_use]
pub fn collect_reads() -> Vec<SpsRead> {
    let parts = crate::capture_set("parts").unwrap_or_default();
    let hardware = crate::capture_set("hardware").unwrap_or_default();
    let mut headers: Vec<u16> = Vec::new();
    for row in &parts {
        if !headers.contains(&row.header) {
            headers.push(row.header);
        }
    }
    let mut reads = Vec::new();
    let mut seen = BTreeSet::new();
    let mut push = |header: u16, did: u16| {
        if seen.insert((header, did)) {
            reads.push(SpsRead {
                header,
                service: 0x1A,
                did,
            });
        }
    };
    for header in headers {
        push(header, 0x90);
        for row in &parts {
            if row.header == header && (0xC1..=0xCA).contains(&row.identifier) {
                push(header, row.identifier);
            }
        }
        for row in &hardware {
            if row.header == header && row.identifier == 0xCB {
                push(header, row.identifier);
            }
        }
    }
    reads
}

/// The gated SPS collect skeleton as JSON, with empty read values.
///
/// `status` is `gated` and `note` says the live read was not run. Every read
/// carries `value: null`, which the (gated) VCX collect or spskit fills later.
#[must_use]
pub fn skeleton_json(vcx: &VcxIdentity, reads: &[SpsRead]) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"schema\": \"voltkit-sps-collect-skeleton/1\",\n");
    out.push_str("  \"status\": \"gated\",\n");
    out.push_str(
        "  \"note\": \"Live SPS/VCX pass-through read not run. The VCX_PRO_Recv byte layout is \
         not recovered and vcxtoolkit keeps the read gated; fill these values once it is ungated, \
         then hand to spskit.\",\n",
    );
    out.push_str("  \"vcx\": {\n");
    out.push_str(&format!("    \"model\": \"{}\",\n", escape(&vcx.model)));
    out.push_str(&format!(
        "    \"firmware\": \"{}\",\n",
        escape(&vcx.firmware)
    ));
    out.push_str(&format!(
        "    \"firmware_date\": \"{}\",\n",
        escape(&vcx.firmware_date)
    ));
    out.push_str(&format!("    \"serial\": \"{}\",\n", escape(&vcx.serial)));
    out.push_str(&format!("    \"port\": \"COM{}\"\n", vcx.port));
    out.push_str("  },\n");
    out.push_str("  \"reads\": [");
    for (index, read) in reads.iter().enumerate() {
        out.push_str(if index == 0 { "\n" } else { ",\n" });
        out.push_str(&format!(
            "    {{ \"header\": \"{:03X}\", \"service\": \"{:02X}\", \"did\": \"{:02X}\", \"value\": null }}",
            read.header, read.service, read.did
        ));
    }
    if reads.is_empty() {
        out.push_str("]\n");
    } else {
        out.push_str("\n  ]\n");
    }
    out.push_str("}\n");
    out
}

/// The skeleton file name, from the VCX serial and the time.
///
/// `sps-collect-<serial>-<YYYYMMDD-HHMMSS>.json`. No VIN is in the name; the VIN,
/// when the gated read later fills it, lives in the file.
#[must_use]
pub fn skeleton_file_name(serial: &str, unix_secs: u64) -> String {
    let tag = if serial.trim().is_empty() {
        "vcx".to_string()
    } else {
        serial
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_lowercase()
    };
    let tag = if tag.is_empty() {
        "vcx".to_string()
    } else {
        tag
    };
    format!(
        "sps-collect-{tag}-{}.json",
        crate::detect::timestamp(unix_secs)
    )
}

fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Run the SAFE VCX doctor and write the gated skeleton under `dir`.
///
/// This opens the USB port and sends GetInfo only — no vehicle channel, no SPS
/// read. Returns the file it wrote. Built without the `vcx` feature or off
/// Windows, there is no VCX path.
///
/// # Errors
///
/// Returns a message when the VCX does not answer the doctor or the file cannot
/// be written.
#[cfg(all(windows, feature = "vcx"))]
pub fn stage(
    port: u32,
    dir: &std::path::Path,
    unix_secs: u64,
) -> Result<std::path::PathBuf, String> {
    let info = crate::detect::vcx_doctor(port)
        .ok_or_else(|| format!("no VCX-NANO answered the doctor on COM{port}"))?;
    let identity = VcxIdentity {
        model: info.hw_name,
        firmware: format!(
            "{}.{}.{}.{}",
            info.fw_version[0], info.fw_version[1], info.fw_version[2], info.fw_version[3]
        ),
        firmware_date: info.fw_date.to_string(),
        serial: info.serial,
        port,
    };
    let text = skeleton_json(&identity, &collect_reads());
    let path = dir.join(skeleton_file_name(&identity.serial, unix_secs));
    std::fs::create_dir_all(dir)
        .and_then(|()| std::fs::write(&path, &text))
        .map_err(|err| format!("could not write {}: {err}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reads_are_vin_parts_and_hardware_per_programming_header() {
        let reads = collect_reads();
        assert!(!reads.is_empty());
        assert!(reads.iter().all(|r| r.service == 0x1A));
        // Every read is a VIN, a part number, or the hardware id — nothing else,
        // and never a write service.
        assert!(reads
            .iter()
            .all(|r| r.did == 0x90 || (0xC1..=0xCA).contains(&r.did) || r.did == 0xCB));
        // The parts headers each carry the VIN and the hardware id.
        for header in [0x7E0u16, 0x7E1, 0x7E4, 0x7E7, 0x241] {
            assert!(
                reads.iter().any(|r| r.header == header && r.did == 0x90),
                "VIN on {header:03X}"
            );
            assert!(
                reads.iter().any(|r| r.header == header && r.did == 0xCB),
                "hardware on {header:03X}"
            );
            assert!(
                reads
                    .iter()
                    .any(|r| r.header == header && (0xC1..=0xCA).contains(&r.did)),
                "parts on {header:03X}"
            );
        }
        // No duplicate read.
        let mut seen = BTreeSet::new();
        assert!(reads.iter().all(|r| seen.insert((r.header, r.did))));
    }

    #[test]
    fn the_skeleton_is_gated_json_with_empty_values() {
        let vcx = VcxIdentity {
            model: "VCX-NANO".to_string(),
            firmware: "1.9.4.2".to_string(),
            firmware_date: "2023-03-31".to_string(),
            serial: "NCAGM26507".to_string(),
            port: 4,
        };
        let reads = vec![
            SpsRead {
                header: 0x7E0,
                service: 0x1A,
                did: 0x90,
            },
            SpsRead {
                header: 0x7E0,
                service: 0x1A,
                did: 0xC1,
            },
        ];
        let json = skeleton_json(&vcx, &reads);
        assert!(json.contains("\"status\": \"gated\""));
        assert!(json.contains("Live SPS/VCX pass-through read not run"));
        assert!(json.contains("\"model\": \"VCX-NANO\""));
        assert!(json.contains("\"firmware\": \"1.9.4.2\""));
        assert!(json.contains("\"serial\": \"NCAGM26507\""));
        assert!(json.contains("\"port\": \"COM4\""));
        assert!(json.contains(
            "{ \"header\": \"7E0\", \"service\": \"1A\", \"did\": \"90\", \"value\": null }"
        ));
        assert!(json.contains("\"did\": \"C1\""));
        // Every read value is null: nothing was read from a vehicle.
        assert_eq!(json.matches("\"value\": null").count(), reads.len());
        assert!(!json.contains("\"value\": \""));
    }

    #[test]
    fn the_skeleton_name_is_the_serial_and_the_time_without_a_vin() {
        assert_eq!(
            skeleton_file_name("NCAGM26507", 1_700_000_000),
            "sps-collect-ncagm26507-20231114-221320.json"
        );
        assert_eq!(
            skeleton_file_name("", 0),
            "sps-collect-vcx-19700101-000000.json"
        );
    }

    #[test]
    fn a_name_or_serial_with_a_quote_is_escaped() {
        assert_eq!(escape("a\"b\\c"), "a\\\"b\\\\c");
        let vcx = VcxIdentity {
            model: "V\"X".to_string(),
            firmware: "1.0.0.0".to_string(),
            firmware_date: "2023-01-01".to_string(),
            serial: "S\\1".to_string(),
            port: 1,
        };
        let json = skeleton_json(&vcx, &[]);
        assert!(json.contains("\"model\": \"V\\\"X\""));
        assert!(json.contains("\"serial\": \"S\\\\1\""));
        assert!(json.contains("\"reads\": []"));
    }
}
