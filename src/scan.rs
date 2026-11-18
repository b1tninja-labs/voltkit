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
    /// Service identifier, `0x1A` or `0x22`.
    pub service: u8,
    /// `$1A` identifier, or the `$22` data identifier.
    pub identifier: u16,
    /// The trial turned headers on before this read.
    pub headers_on: bool,
    /// The trial set the `241` flow-control override before this read.
    pub flow_override: bool,
}

const fn gmlan(header: u16, receive: Option<u16>, id: u8) -> ScanRow {
    ScanRow {
        header,
        receive,
        service: 0x1A,
        identifier: id as u16,
        headers_on: false,
        flow_override: false,
    }
}

const fn did(header: u16, receive: Option<u16>, id: u16) -> ScanRow {
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

const TABLE: &[ScanRow] = &[
    gmlan(0x7E4, None, 0x90),
    gmlan(0x241, R641, 0x90),
    gmlan_flow(0x241, R641, 0xDF),
    gmlan_headers(0x7DF, None, 0xB0),
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

/// `$1A` identifiers `90`, `DF`, `B0`, `99`, `CC`, `6F`, `6D`, then the 29 `$22` DIDs.
///
/// `2889` is one DID asked on two headers (`7E1` and `621`), so the table has 30 `$22`
/// rows and 29 identifiers.
#[must_use]
pub fn scan_table() -> &'static [ScanRow] {
    TABLE
}

/// Adapter and request lines for [`scan_table`], assuming [`INIT`] was sent first.
///
/// `ATH0` is sent again after the one read the trial made with headers on. The trial
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

fn request_line(row: &ScanRow) -> String {
    if row.service == 0x1A {
        format!("1A{:02X}", row.identifier)
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
        assert!(scan.windows(4).any(|w| {
            w.iter()
                .map(String::as_str)
                .eq(["ATSH7DF", "ATCRA", "ATH1", "1AB0"])
        }));
        assert!(scan
            .windows(2)
            .any(|w| w.iter().map(String::as_str).eq(["ATSH621", "222889"])));
        assert!(!scan
            .iter()
            .any(|l| l.contains("621") && l.starts_with("ATCRA")));
        for line in &lines {
            match classify(line) {
                Ok(Line::Adapter) | Ok(Line::Read(0x1A | 0x22)) => {}
                other => panic!("{line} classified as {other:?}"),
            }
        }
    }
}
