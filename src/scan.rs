//! The Wi-Fi read that the 2026-11-14 Voltage trial actually sent.
//!
//! Headers come from that trial (`fakeobd` `docs/AVD-TRIAL.md`), not from a guess about
//! which module owns a DID. The only receive address the trial set is `ATCRA641` for
//! header `241`. Other headers keep the adapter's default receive filter. `621` is
//! asked for `2889` and has no recorded response id, so no receive address is set.

const R641: Option<u16> = Some(0x641);

/// Datasheet reset-and-format sequence, in the order the trial sent it.
///
/// `ATSP6` is ELM protocol 6: ISO 15765-4 CAN, 11-bit ID, 500 kbaud (datasheet
/// protocol list). It is not a claim about ISO 15765-2.
pub const INIT: [&str; 7] = ["ATZ", "ATL0", "ATH0", "ATS0", "ATSP6", "ATE0", "ATCAF1"];

/// One identifier the trial read, and the header it used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanRow {
    /// `ATSH` value recorded for this read.
    pub header: u16,
    /// `ATCRA` value recorded for this read. `None` means the trial set none.
    pub receive: Option<u16>,
    /// Service identifier: `0x03`, `0x1A`, or `0x22`.
    pub service: u8,
    /// `$1A` identifier, the `$22` data identifier, or zero for service `0x03`.
    pub identifier: u16,
    /// The trial turned headers on before this read.
    pub headers_on: bool,
    /// The trial set the `241` flow-control override before this read.
    pub flow_override: bool,
}

pub(crate) const fn gmlan(header: u16, receive: Option<u16>, id: u8) -> ScanRow {
    ScanRow {
        header,
        receive,
        service: 0x1A,
        identifier: id as u16,
        headers_on: false,
        flow_override: false,
    }
}

pub(crate) const fn did(header: u16, receive: Option<u16>, id: u16) -> ScanRow {
    ScanRow {
        header,
        receive,
        service: 0x22,
        identifier: id,
        headers_on: false,
        flow_override: false,
    }
}

const fn gmlan_flow(header: u16, receive: Option<u16>, id: u8) -> ScanRow {
    let mut row = gmlan(header, receive, id);
    row.flow_override = true;
    row
}

const fn gmlan_headers(header: u16, receive: Option<u16>, id: u8) -> ScanRow {
    let mut row = gmlan(header, receive, id);
    row.headers_on = true;
    row
}

const fn mode03(header: u16) -> ScanRow {
    ScanRow {
        header,
        receive: None,
        service: 0x03,
        identifier: 0,
        headers_on: true,
        flow_override: false,
    }
}

const TABLE: &[ScanRow] = &[
    gmlan(0x7E4, None, 0x90),
    gmlan(0x241, R641, 0x90),
    gmlan_flow(0x241, R641, 0xDF),
    gmlan_headers(0x7DF, None, 0xB0),
    mode03(0x7DF),
    gmlan(0x7E0, None, 0x99),
    gmlan(0x7E0, None, 0xCC),
    gmlan(0x7E0, None, 0x6F),
    gmlan(0x7E0, None, 0x6D),
    gmlan(0x7E1, None, 0x99),
    gmlan(0x7E1, None, 0xCC),
    gmlan(0x7E1, None, 0x6F),
    gmlan(0x7E1, None, 0x6D),
    gmlan(0x7E4, None, 0x99),
    gmlan(0x7E4, None, 0xCC),
    gmlan(0x7E4, None, 0x6F),
    gmlan(0x7E4, None, 0x6D),
    gmlan(0x7E7, None, 0x99),
    gmlan(0x7E7, None, 0xCC),
    gmlan(0x7E7, None, 0x6F),
    gmlan(0x7E7, None, 0x6D),
    did(0x7E0, None, 0x002F),
    did(0x7E4, None, 0x40E9),
    did(0x7E4, None, 0x41A3),
    did(0x7E4, None, 0x41A6),
    did(0x7E4, None, 0x41AD),
    did(0x7E4, None, 0x41C5),
    did(0x7E4, None, 0x41C6),
    did(0x7E4, None, 0x432D),
    did(0x7E4, None, 0x434F),
    did(0x7E4, None, 0x4356),
    did(0x7E4, None, 0x4368),
    did(0x7E4, None, 0x4369),
    did(0x7E4, None, 0x437D),
    did(0x7E4, None, 0x4389),
    did(0x7E4, None, 0x43A5),
    did(0x7E4, None, 0x43A6),
    did(0x7E4, None, 0x43AE),
    did(0x7E4, None, 0x43AF),
    did(0x7E4, None, 0x4470),
    did(0x7E4, None, 0x45FF),
    did(0x7E4, None, 0x8334),
    did(0x7E4, None, 0x83E2),
    did(0x7E1, None, 0x2889),
    did(0x621, None, 0x2889),
    did(0x7E7, None, 0x4181),
    did(0x241, R641, 0x8005),
    did(0x241, R641, 0x8090),
    did(0x241, R641, 0x809C),
    did(0x241, R641, 0x8537),
    did(0x241, R641, 0xCB31),
];

/// `$1A` identifiers `90`, `DF`, `B0`, `99`, `CC`, `6F`, `6D`, one mode `03` on `7DF`,
/// then the 29 `$22` DIDs.
///
/// `2889` is one DID asked on two headers (`7E1` and `621`), so the table has 30 `$22`
/// rows and 29 identifiers. The mode `03` row has headers on and no receive address.
#[must_use]
pub fn scan_table() -> &'static [ScanRow] {
    TABLE
}

/// Adapter and request lines for [`scan_table`], assuming [`INIT`] was sent first.
///
/// `ATH0` is sent again after the headers-on reads on `7DF`. The trial
/// did not record turning headers back off; the later reads use the init setting so
/// the reply is the diagnostic payload.
#[must_use]
pub fn scan_lines() -> Vec<String> {
    command_lines(scan_table())
}

/// Adapter and request lines for `rows`, with the same header rules as [`scan_lines`].
pub(crate) fn command_lines(rows: &[ScanRow]) -> Vec<String> {
    let mut out = Vec::new();
    let mut header = None;
    let mut receive = None;
    let mut headers_on = false;
    for row in rows {
        if header != Some(row.header) {
            out.push(format!("ATSH{:03X}", row.header));
            header = Some(row.header);
        }
        if receive != row.receive {
            match row.receive {
                Some(id) => out.push(format!("ATCRA{id:03X}")),
                None => out.push("ATCRA".to_string()),
            }
            receive = row.receive;
        }
        if headers_on != row.headers_on {
            out.push(if row.headers_on { "ATH1" } else { "ATH0" }.to_string());
            headers_on = row.headers_on;
        }
        if row.flow_override {
            out.push("ATAL".to_string());
            out.push(format!("ATFCSH{:03X}", row.header));
            out.push("ATFCSD300014".to_string());
            out.push("ATFCSM1".to_string());
        }
        out.push(request_line(row));
        if row.flow_override {
            out.push("ATAR".to_string());
            out.push("ATFCSM0".to_string());
        }
    }
    out
}

/// [`INIT`] followed by [`scan_lines`].
#[must_use]
pub fn session_lines() -> Vec<String> {
    INIT.into_iter()
        .map(str::to_string)
        .chain(scan_lines())
        .collect()
}

/// The trial scan plus every identifier the lab profile answers, and the
/// charger and climate reads traced after that profile was written.
///
/// Headers that begin with `2` set the receive address the profile names
/// (`641`, `644`, `64C`). `621` stays without one. No row is service `$27`.
#[must_use]
pub fn profile_table() -> Vec<ScanRow> {
    let mut rows = scan_table().to_vec();
    let mut seen: std::collections::BTreeSet<(u16, u8, u16)> = rows
        .iter()
        .map(|row| (row.header, row.service, row.identifier))
        .collect();
    let mut add = |row: ScanRow| {
        if seen.insert((row.header, row.service, row.identifier)) {
            rows.push(row);
        }
    };
    for id in [
        0x1C1E_u16, 0x1C38, 0x1C39, 0x1C47, 0x41AD, 0x41C6, 0x43AF, 0x4184, 0x4185, 0x4202, 0x42A0,
        0x90B1, 0x90F8, 0x90FB, 0x5005, 0x000D, 0x000C, 0x0005,
    ] {
        add(did(0x7E0, None, id));
    }
    for id in [0x90_u8, 0xDF, 0xB0] {
        add(gmlan(0x7E0, None, id));
    }
    for id in [
        0x2414_u16, 0x2439, 0x2C05, 0x002C, 0x1154, 0x1940, 0x3040, 0x3041, 0x000D, 0x000C, 0x0005,
        0x43AF, 0x8334, 0x41A6, 0x24AC, 0x801F, 0x434F, 0x41A4, 0x435A, 0x41B6, 0x439E, 0x0042,
        0x40D7, 0x40D8, 0x40D9, 0x40DA, 0x40DB, 0x40DC, 0x40DD, 0x40DE, 0x40DF, 0x40E0, 0x40E1,
        0x4470, 0x41C5, 0x41C6, 0x41AD, 0x432A, 0x432C, 0x4181, 0x4201,
    ] {
        add(did(0x7E1, None, id));
    }
    for id in [0x90_u8, 0xDF, 0xB0] {
        add(gmlan(0x7E1, None, id));
    }
    for id in [
        0x41B0_u16, 0x41B7, 0x436F, 0x4372, 0x439B, 0x441E, 0x4425, 0x43CB, 0x43CC, 0x43CD, 0x43CE,
        0x43CF, 0x452D, 0x452E, 0x4531, 0x4532, 0x452F, 0x4530, 0x4533, 0x4545, 0x4546, 0x8375,
        0x8377, 0x4210, 0x4211, 0x4212, 0x41DA, 0x432A, 0x432C, 0x436B, 0x436C, 0x83E1,
    ] {
        add(did(0x7E4, None, id));
    }
    for id in [0xDF_u8, 0xB0, 0xA4, 0x75] {
        add(gmlan(0x7E4, None, id));
    }
    for id in [0x83E1_u16, 0x4368, 0x8000] {
        add(did(0x244, Some(0x644), id));
    }
    for id in [
        0x5005_u16, 0x5006, 0x5007, 0x5008, 0x900D, 0x9025, 0x9037, 0x90B1, 0x90F8, 0x90FB, 0x801F,
        0x82B2, 0x817B, 0x41B4, 0x41B2, 0x82B5, 0x451E,
    ] {
        add(did(0x241, Some(0x641), id));
    }
    for id in [0x99_u8, 0xCC, 0xB0, 0x6F, 0x6D] {
        add(gmlan(0x241, Some(0x641), id));
    }
    for id in [0x90_u8, 0x99, 0xCC, 0xDF, 0xB0, 0x6F, 0x6D] {
        add(gmlan(0x24C, Some(0x64C), id));
    }
    for id in 0x4182_u16..=0x4250 {
        add(did(0x7E7, None, id));
    }
    for id in [0x82A3_u16, 0x40D4] {
        add(did(0x7E7, None, id));
    }
    for id in [0x90_u8, 0xDF, 0xB0] {
        add(gmlan(0x7E7, None, id));
    }
    rows
}

/// [`INIT`] followed by the adapter and request lines for `rows`.
#[must_use]
pub fn lines_for(rows: &[ScanRow]) -> Vec<String> {
    INIT.into_iter()
        .map(str::to_string)
        .chain(command_lines(rows))
        .collect()
}

/// [`INIT`] followed by [`profile_table`].
#[must_use]
pub fn profile_lines() -> Vec<String> {
    lines_for(&profile_table())
}

/// Bytes after the positive service and the identifier.
#[must_use]
pub fn profile_bytes<'a>(row: &ScanRow, data: &'a [u8]) -> Option<&'a [u8]> {
    match row.service {
        0x22 if data.len() >= 3
            && data[0] == 0x62
            && u16::from_be_bytes([data[1], data[2]]) == row.identifier =>
        {
            Some(&data[3..])
        }
        0x1A if data.len() >= 2 && data[0] == 0x5A && u16::from(data[1]) == row.identifier => {
            Some(&data[2..])
        }
        _ => None,
    }
}

/// A profile line for a positive reply, data bytes only.
///
/// `$22` keeps the bytes after `62` and the identifier. `$1A` keeps the bytes
/// after `5A` and the identifier. Service `0x03` is `None` here; the clone
/// comments a positive `43` payload. A negative response is `None`. The line is a
/// `did` or `lid` directive the lab profile format accepts.
#[must_use]
pub fn profile_note(row: &ScanRow, data: &[u8]) -> Option<String> {
    if row.service == 0x03 {
        return None;
    }
    let value = profile_bytes(row, data)?;
    let hex = value
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<String>();
    Some(match row.service {
        0x22 => format!("did {:04X} {hex}", row.identifier),
        _ => format!("lid {:02X} {hex}", row.identifier),
    })
}

fn request_line(row: &ScanRow) -> String {
    if row.service == 0x1A {
        format!("1A{:02X}", row.identifier)
    } else if row.service == 0x03 {
        "03".to_string()
    } else {
        format!("22{:04X}", row.identifier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elm::{classify, Line};
    use std::collections::BTreeSet;

    const DIDS: [u16; 29] = [
        0x002F, 0x41A6, 0x41AD, 0x41C5, 0x41C6, 0x4368, 0x4369, 0x43AE, 0x43AF, 0x4470, 0x8334,
        0x83E2, 0x40E9, 0x41A3, 0x432D, 0x434F, 0x4356, 0x437D, 0x4389, 0x43A5, 0x43A6, 0x45FF,
        0x2889, 0x4181, 0x8005, 0x8090, 0x809C, 0x8537, 0xCB31,
    ];

    #[test]
    fn the_twenty_nine_dids_use_the_trial_headers() {
        let rows: Vec<_> = scan_table().iter().filter(|r| r.service == 0x22).collect();
        let set: BTreeSet<_> = rows.iter().map(|r| r.identifier).collect();
        assert_eq!(set, BTreeSet::from(DIDS));
        assert_eq!(rows.len(), 30, "2889 is asked on 7E1 and on 621");
        let at = |id: u16| -> Vec<(u16, Option<u16>)> {
            rows.iter()
                .filter(|r| r.identifier == id)
                .map(|r| (r.header, r.receive))
                .collect()
        };
        assert_eq!(at(0x002F), vec![(0x7E0, None)]);
        assert_eq!(at(0x2889), vec![(0x7E1, None), (0x621, None)]);
        assert_eq!(at(0x4181), vec![(0x7E7, None)]);
        for id in [0x8005u16, 0x8090, 0x809C, 0x8537, 0xCB31] {
            assert_eq!(at(id), vec![(0x241, Some(0x641))], "{id:04X}");
        }
        for id in DIDS {
            if matches!(
                id,
                0x002F | 0x2889 | 0x4181 | 0x8005 | 0x8090 | 0x809C | 0x8537 | 0xCB31
            ) {
                continue;
            }
            assert_eq!(at(id), vec![(0x7E4, None)], "{id:04X}");
        }
    }

    #[test]
    fn gmlan_identifiers_use_the_trial_headers() {
        let rows: Vec<_> = scan_table().iter().filter(|r| r.service == 0x1A).collect();
        let at = |id: u16| -> Vec<(u16, Option<u16>)> {
            rows.iter()
                .filter(|r| r.identifier == id)
                .map(|r| (r.header, r.receive))
                .collect()
        };
        let ids: BTreeSet<_> = rows.iter().map(|r| r.identifier).collect();
        assert_eq!(
            ids,
            BTreeSet::from([0x90u16, 0xDF, 0xB0, 0x99, 0xCC, 0x6F, 0x6D])
        );
        assert_eq!(at(0x90), vec![(0x7E4, None), (0x241, Some(0x641))]);
        assert_eq!(at(0xDF), vec![(0x241, Some(0x641))]);
        assert_eq!(at(0xB0), vec![(0x7DF, None)]);
        for id in [0x99u16, 0xCC, 0x6F, 0x6D] {
            assert_eq!(
                at(id),
                vec![(0x7E0, None), (0x7E1, None), (0x7E4, None), (0x7E7, None)],
                "{id:02X}"
            );
        }
        assert!(rows.iter().any(|r| r.identifier == 0xDF && r.flow_override));
        assert!(rows.iter().any(|r| r.identifier == 0xB0 && r.headers_on));
        assert!(rows.iter().filter(|r| r.flow_override).count() == 1);
    }

    #[test]
    fn the_session_is_the_init_then_reads_and_never_security_access() {
        let lines = session_lines();
        assert!(lines.iter().map(String::as_str).take(INIT.len()).eq(INIT));
        let scan = &lines[INIT.len()..];
        let df = scan.iter().position(|l| l == "1ADF").unwrap();
        let flow: Vec<&str> = scan[df - 4..df + 3].iter().map(String::as_str).collect();
        assert_eq!(
            flow,
            [
                "ATAL",
                "ATFCSH241",
                "ATFCSD300014",
                "ATFCSM1",
                "1ADF",
                "ATAR",
                "ATFCSM0"
            ]
        );
        assert!(scan.windows(5).any(|w| {
            w.iter()
                .map(String::as_str)
                .eq(["ATSH7DF", "ATCRA", "ATH1", "1AB0", "03"])
        }));
        assert!(scan
            .windows(2)
            .any(|w| w.iter().map(String::as_str).eq(["ATSH621", "222889"])));
        assert!(!scan
            .iter()
            .any(|l| l.contains("621") && l.starts_with("ATCRA")));
        for line in &lines {
            match classify(line) {
                Ok(Line::Adapter) | Ok(Line::Read(0x1A | 0x22 | 0x03)) => {}
                other => panic!("{line} classified as {other:?}"),
            }
        }
    }

    #[test]
    fn the_profile_capture_covers_the_lab_identifiers_and_stays_a_read() {
        let rows = profile_table();
        let has = |header, service, id| {
            rows.iter()
                .any(|row| row.header == header && row.service == service && row.identifier == id)
        };
        assert!(has(0x7E1, 0x22, 0x000D));
        assert!(has(0x7E1, 0x22, 0x40D8));
        assert!(has(0x7E4, 0x22, 0x4532));
        assert!(has(0x7E4, 0x1A, 0xA4));
        assert!(has(0x7E4, 0x1A, 0x75));
        assert!(has(0x241, 0x22, 0x801F));
        assert!(has(0x244, 0x22, 0x8000));
        assert!(has(0x7E7, 0x22, 0x4181));
        assert!(has(0x7E7, 0x22, 0x4240));
        assert!(has(0x7E7, 0x22, 0x4250));
        assert!(has(0x7E7, 0x22, 0x82A3));
        assert!(has(0x24C, 0x1A, 0x90));
        let mut seen = std::collections::BTreeSet::new();
        for row in &rows {
            assert!(seen.insert((row.header, row.service, row.identifier)));
            assert!(matches!(row.service, 0x03 | 0x1A | 0x22));
        }
        assert_eq!(rows.iter().filter(|row| row.service == 0x03).count(), 1);
        assert!(rows.iter().all(|row| row.service != 0x01));
        let speed = rows
            .iter()
            .find(|row| row.header == 0x7E1 && row.identifier == 0x000D)
            .unwrap();
        assert_eq!(
            profile_note(speed, &[0x62, 0x00, 0x0D, 0x28, 0x00]).as_deref(),
            Some("did 000D 2800")
        );
        assert!(profile_note(speed, &[0x7F, 0x22, 0x31]).is_none());
        for line in profile_lines() {
            match classify(&line) {
                Ok(Line::Adapter) | Ok(Line::Read(0x1A | 0x22 | 0x03)) => {}
                other => panic!("{line} classified as {other:?}"),
            }
        }
    }
}
