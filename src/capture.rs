//! Named capture sets and a lab-profile dump of the positive replies.
//!
//! Each set is a slice of the reads the lab profile and the traced screens already
//! name. A range walks one header. The clone text is the `ecu` / `did` / `lid`
//! form `fakeobd` parses. Headers with no recorded response id stay comments.

use crate::scan::{did, gmlan, profile_bytes, profile_note, profile_table, scan_table, ScanRow};

/// One answered read. `data` is the diagnostic payload, including the service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read {
    /// The header and identifier that were asked.
    pub row: ScanRow,
    /// The payload `payload` returned, or empty when the reply was not hex.
    pub data: Vec<u8>,
}

const CHARGER: &[u16] = &[
    0x436C, 0x436B, 0x43AE, 0x4531, 0x83E1, 0x83E2, 0x4368, 0x4369, 0x4532, 0x4530, 0x452F, 0x4533,
    0x4545, 0x4546, 0x4372, 0x439B, 0x452D, 0x452E, 0x436F, 0x4425, 0x41B7, 0x43CB, 0x43CC, 0x43CD,
    0x43CE, 0x43CF,
];

const CLIMATE: &[u16] = &[0x801F, 0x82B2, 0x817B, 0x41B4, 0x41B2, 0x82B5, 0x451E];

const PART_HEADERS: [(u16, Option<u16>); 5] = [
    (0x7E0, None),
    (0x7E1, None),
    (0x7E4, None),
    (0x7E7, None),
    (0x241, Some(0x641)),
];

// Appendix pairs the lab profile does not already ask. Physical headers set no ATCRA.
const PRESENCE_HEADERS: [(u16, Option<u16>); 9] = [
    (0x242, Some(0x642)),
    (0x24B, Some(0x64B)),
    (0x251, Some(0x651)),
    (0x255, Some(0x655)),
    (0x257, Some(0x657)),
    (0x258, Some(0x658)),
    (0x7E2, None),
    (0x7E5, None),
    (0x7E6, None),
];

const SPS_EXTRA: [u8; 23] = [
    0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x60, 0x62, 0x64, 0x66, 0x67,
    0x68, 0x69, 0x6A, 0x71, 0x73, 0x74, 0x76,
];

/// Set name and the one line `--list` prints.
pub const SETS: &[(&str, &str)] = &[
    ("all", "the lab profile capture, including mode 03 on 7DF"),
    (
        "scan",
        "the 2026-11-14 trial only, including mode 03 on 7DF",
    ),
    ("hpcm", "header 7E0"),
    ("powertrain", "header 7E1"),
    ("becm", "header 7E4"),
    (
        "charger",
        "the charger screen on 7E4, including 1A A4 and 1A 75",
    ),
    (
        "cells",
        "7E7 cell table 4181-4240, catalog tail 4241-4250, plus 82A3 and 40D4",
    ),
    ("climate", "the climate reads on 241"),
    ("body", "the other 241 reads"),
    ("cluster", "header 244"),
    ("identity", "every $1A read in the lab capture"),
    ("parts", "$1A C0 through CA on 7E0, 7E1, 7E4, 7E7, and 241"),
    (
        "presence",
        "$1A 90 on 242, 24B, 251, 255, 257, 258, 7E2, 7E5, and 7E6",
    ),
    (
        "sps-extra",
        "$1A 40-4A, 60, 62, 64, 66-6A, 71, 73, 74, and 76 on the parts headers",
    ),
];

/// The rows for `name`, or `None` when `name` is not a set.
#[must_use]
pub fn capture_set(name: &str) -> Option<Vec<ScanRow>> {
    let rows = match name {
        "all" => return Some(profile_table()),
        "scan" => return Some(scan_table().to_vec()),
        "parts" => return Some(part_rows()),
        "presence" => return Some(presence_rows()),
        "sps-extra" => return Some(sps_extra_rows()),
        "hpcm" => on_header(0x7E0),
        "powertrain" => on_header(0x7E1),
        "becm" => on_header(0x7E4),
        "cluster" => on_header(0x244),
        "charger" => profile_table()
            .into_iter()
            .filter(|row| {
                row.header == 0x7E4
                    && ((row.service == 0x22 && CHARGER.contains(&row.identifier))
                        || (row.service == 0x1A && matches!(row.identifier, 0xA4 | 0x75)))
            })
            .collect(),
        "cells" => profile_table()
            .into_iter()
            .filter(|row| {
                row.header == 0x7E7
                    && row.service == 0x22
                    && ((0x4181..=0x4250).contains(&row.identifier)
                        || matches!(row.identifier, 0x82A3 | 0x40D4))
            })
            .collect(),
        "climate" => profile_table()
            .into_iter()
            .filter(|row| {
                row.header == 0x241 && row.service == 0x22 && CLIMATE.contains(&row.identifier)
            })
            .collect(),
        "body" => profile_table()
            .into_iter()
            .filter(|row| {
                row.header == 0x241 && !(row.service == 0x22 && CLIMATE.contains(&row.identifier))
            })
            .collect(),
        "identity" => profile_table()
            .into_iter()
            .filter(|row| row.service == 0x1A)
            .collect(),
        _ => return None,
    };
    Some(rows)
}

/// One header, one service, inclusive.
///
/// `$22` identifiers are 16-bit. `$1A` identifiers are one byte. The span is at
/// most 256 identifiers. Service `$27` is refused here, before a socket exists.
/// A header above 11 bits is refused because the session is `ATSP6`.
///
/// # Errors
///
/// Returns a message when the header, the service, or the span is outside that.
pub fn range_rows(header: u16, service: u8, start: u16, end: u16) -> Result<Vec<ScanRow>, String> {
    if header == 0 || header > 0x7FF {
        return Err("range header is an 11-bit id".to_string());
    }
    if service == 0x27 {
        return Err("security access is not sent".to_string());
    }
    if service != 0x22 && service != 0x1A {
        return Err("range service is 22 or 1A".to_string());
    }
    if end < start {
        return Err("range end is before the start".to_string());
    }
    let count = u32::from(end) - u32::from(start) + 1;
    if count > 256 {
        return Err("a range is at most 256 identifiers".to_string());
    }
    if service == 0x1A && end > 0xFF {
        return Err("a $1A identifier is one byte".to_string());
    }
    let receive = recorded_receive(header);
    let mut rows = Vec::with_capacity(count as usize);
    for id in start..=end {
        rows.push(if service == 0x1A {
            gmlan(header, receive, id as u8)
        } else {
            did(header, receive, id)
        });
    }
    Ok(rows)
}

/// Lab profile text for the positive replies.
///
/// One `ecu` block per header that has a recorded response id. A positive
/// `$1A` `C0`–`CA` line carries the decimal of its first four data bytes, which
/// is the part-number slice. Other headers are comments, so a missing response
/// id is not invented. A positive mode `03` reply, which starts with `43`, is a
/// comment of the raw hex. A negative reply is left out.
#[must_use]
pub fn clone_profile(reads: &[Read]) -> String {
    let mut seen = std::collections::BTreeSet::new();
    let mut grouped: Vec<(u16, Vec<String>)> = Vec::new();
    let mut notes = Vec::new();
    for read in reads {
        if read.row.service == 0x03 {
            if read.data.first() == Some(&0x43)
                && seen.insert((read.row.header, read.row.service, read.row.identifier))
            {
                let hex = read
                    .data
                    .iter()
                    .map(|byte| format!("{byte:02X}"))
                    .collect::<String>();
                notes.push(format!("# {hex}"));
                if read.row.receive.is_none() {
                    notes.push(format!(
                        "# header {:03X} has no recorded response id",
                        read.row.header
                    ));
                }
            }
            continue;
        }
        let Some(mut line) = profile_note(&read.row, &read.data) else {
            continue;
        };
        if !seen.insert((read.row.header, read.row.service, read.row.identifier)) {
            continue;
        }
        if read.row.service == 0x1A && (0xC0..=0xCA).contains(&read.row.identifier) {
            if let Some(bytes) = profile_bytes(&read.row, &read.data) {
                if bytes.len() >= 4 {
                    let n = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    line.push_str(&format!("    # decimal {n}"));
                }
            }
        }
        if ecu_line(read.row.header).is_some() {
            if let Some((_, lines)) = grouped
                .iter_mut()
                .find(|(header, _)| *header == read.row.header)
            {
                lines.push(line);
            } else {
                grouped.push((read.row.header, vec![line]));
            }
        } else {
            notes.push(format!("# {line}"));
            notes.push(format!(
                "# header {:03X} has no recorded response id",
                read.row.header
            ));
        }
    }
    let mut out = String::from(
        "# Captured module bytes. A did or lid line is the data after the service and the identifier.\n\
         # Latency is the lab profile's value. Mode 01 and freeze frames are not in this capture.\n\
         # A positive mode 03 reply is a comment of the raw hex.\n\
         s3 5000\n",
    );
    const PREFERRED: [u16; 7] = [0x7E0, 0x7E4, 0x244, 0x241, 0x24C, 0x7E1, 0x7E7];
    let mut emitted = std::collections::BTreeSet::new();
    for header in PREFERRED {
        if let Some((_, lines)) = grouped.iter().find(|(id, _)| *id == header) {
            emitted.insert(header);
            push_ecu(&mut out, header, lines);
        }
    }
    for (header, lines) in &grouped {
        if emitted.insert(*header) {
            push_ecu(&mut out, *header, lines);
        }
    }
    for note in &notes {
        out.push_str(note);
        out.push('\n');
    }
    if grouped.is_empty() && notes.is_empty() {
        out.push_str("# no positive reply\n");
    }
    out
}

fn on_header(header: u16) -> Vec<ScanRow> {
    profile_table()
        .into_iter()
        .filter(|row| row.header == header)
        .collect()
}

fn part_rows() -> Vec<ScanRow> {
    let mut rows = Vec::new();
    for (header, receive) in PART_HEADERS {
        for id in 0xC0u8..=0xCA {
            rows.push(gmlan(header, receive, id));
        }
    }
    rows
}

fn presence_rows() -> Vec<ScanRow> {
    let mut rows = Vec::new();
    for (header, receive) in PRESENCE_HEADERS {
        rows.push(gmlan(header, receive, 0x90));
    }
    rows
}

fn sps_extra_rows() -> Vec<ScanRow> {
    let mut rows = Vec::new();
    for (header, receive) in PART_HEADERS {
        for id in SPS_EXTRA {
            rows.push(gmlan(header, receive, id));
        }
    }
    rows
}

fn recorded_receive(header: u16) -> Option<u16> {
    match header {
        0x241 => Some(0x641),
        0x244 => Some(0x644),
        0x24C => Some(0x64C),
        _ => None,
    }
}

fn ecu_line(header: u16) -> Option<String> {
    let (name, resp, functional, latency) = match header {
        0x7E0 => ("HPCM2", 0x7E8_u16, "yes", 3_u32),
        0x7E1 => ("M7E1", 0x7E9, "no", 4),
        0x7E4 => ("BECM", 0x7EC, "yes", 4),
        0x7E7 => ("M7E7", 0x7EF, "no", 4),
        0x241 => ("M241", 0x641, "no", 5),
        0x244 => ("IPC", 0x644, "no", 6),
        0x24C => ("M24C", 0x64C, "no", 5),
        _ => return None,
    };
    Some(format!(
        "ecu {name} req={header:03X} resp={resp:03X} functional={functional} pad=00 latency={latency}"
    ))
}

fn push_ecu(out: &mut String, header: u16, lines: &[String]) {
    let Some(ecu) = ecu_line(header) else {
        return;
    };
    out.push('\n');
    out.push_str(&ecu);
    out.push('\n');
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elm::{classify, Line};
    use crate::scan::command_lines;

    #[test]
    fn sets_split_the_capture_and_parts_stay_out_of_all() {
        let all = capture_set("all").unwrap();
        let cells = capture_set("cells").unwrap();
        assert!(cells.iter().any(|row| row.identifier == 0x4181));
        assert!(cells.iter().any(|row| row.identifier == 0x4240));
        assert!(cells.iter().any(|row| row.identifier == 0x4250));
        assert!(cells.iter().all(|row| row.identifier != 0x4251));
        assert!(cells.iter().all(|row| row.header == 0x7E7));
        assert!(capture_set("charger")
            .unwrap()
            .iter()
            .any(|row| row.identifier == 0x4532));
        assert!(capture_set("climate")
            .unwrap()
            .iter()
            .all(|row| row.header == 0x241 && CLIMATE.contains(&row.identifier)));
        let parts = capture_set("parts").unwrap();
        assert_eq!(parts.len(), 11 * 5);
        assert!(parts.iter().all(|row| row.service == 0x1A));
        assert!(parts
            .iter()
            .any(|row| row.header == 0x241 && row.receive == Some(0x641)));
        assert!(all
            .iter()
            .all(|row| { !(row.service == 0x1A && (0xC0..=0xCA).contains(&row.identifier)) }));
        assert!(capture_set("nope").is_none());
        let presence = capture_set("presence").unwrap();
        assert_eq!(presence.len(), PRESENCE_HEADERS.len());
        assert!(presence
            .iter()
            .all(|row| row.service == 0x1A && row.identifier == 0x90));
        assert!(presence
            .iter()
            .any(|row| row.header == 0x242 && row.receive == Some(0x642)));
        assert_eq!(
            presence
                .iter()
                .map(|row| (row.header, row.receive))
                .collect::<Vec<_>>(),
            PRESENCE_HEADERS
        );
        assert!(all.iter().all(|row| {
            !PRESENCE_HEADERS
                .iter()
                .any(|(header, _)| row.header == *header)
        }));
        let extra = capture_set("sps-extra").unwrap();
        assert_eq!(extra.len(), SPS_EXTRA.len() * PART_HEADERS.len());
        assert!(extra.iter().all(|row| row.service == 0x1A));
        assert!(extra.iter().all(|row| row.identifier != 0xC0));
        assert!(extra
            .iter()
            .all(|row| !matches!(row.identifier, 0x90 | 0xCB | 0xCC)));
        assert!(extra
            .iter()
            .any(|row| row.header == 0x241 && row.receive == Some(0x641)));
        assert!(all.iter().all(|row| {
            !(row.service == 0x1A && SPS_EXTRA.iter().any(|id| u16::from(*id) == row.identifier))
        }));
        let mode = all
            .iter()
            .find(|row| row.header == 0x7DF && row.service == 0x03)
            .unwrap();
        assert!(mode.headers_on);
        assert_eq!(mode.receive, None);
        let request = command_lines(std::slice::from_ref(mode))
            .into_iter()
            .find(|line| classify(line) == Ok(Line::Read(0x03)))
            .unwrap();
        assert_eq!(request, "03");
        assert!(all.iter().all(|row| row.service != 0x01));
        assert_eq!(SETS.len(), 14);
    }

    #[test]
    fn a_range_is_bounded_and_keeps_the_recorded_receive_address() {
        let rows = range_rows(0x241, 0x22, 0x8000, 0x8002).unwrap();
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|row| row.receive == Some(0x641)));
        assert!(range_rows(0x7E7, 0x22, 0x4181, 0x4281).is_err());
        assert!(range_rows(0x7E4, 0x27, 0x01, 0x01).is_err());
        assert!(range_rows(0x7E4, 0x1A, 0xC0, 0x100).is_err());
        let wide = range_rows(0x7E7, 0x22, 0x4200, 0x42FF).unwrap();
        assert_eq!(wide.len(), 256);
    }

    #[test]
    fn the_clone_is_one_ecu_per_answered_header() {
        let becm = did(0x7E4, None, 0x4532);
        let part = gmlan(0x7E4, None, 0xC1);
        let quiet = did(0x7E4, None, 0x000D);
        let unknown = did(0x621, None, 0x2889);
        let text = clone_profile(&[
            Read {
                row: becm,
                data: vec![0x62, 0x45, 0x32, 0x00, 0x01],
            },
            Read {
                row: part,
                data: vec![0x5A, 0xC1, 0x00, 0x00, 0x00, 0x2A],
            },
            Read {
                row: quiet,
                data: vec![0x7F, 0x22, 0x31],
            },
            Read {
                row: unknown,
                data: vec![0x62, 0x28, 0x89, 0x00],
            },
            Read {
                row: ScanRow {
                    header: 0x7DF,
                    receive: None,
                    service: 0x03,
                    identifier: 0,
                    headers_on: true,
                    flow_override: false,
                },
                data: vec![0x43, 0x01, 0x0A, 0x7E],
            },
            Read {
                row: ScanRow {
                    header: 0x7DF,
                    receive: None,
                    service: 0x03,
                    identifier: 0,
                    headers_on: true,
                    flow_override: false,
                },
                data: vec![0x7F, 0x03, 0x31],
            },
        ]);
        assert!(text.contains("ecu BECM req=7E4 resp=7EC functional=yes pad=00 latency=4"));
        assert!(text.contains("did 4532 0001"));
        assert!(text.contains("lid C1 0000002A    # decimal 42"));
        assert!(!text.contains("did 000D"));
        assert!(text.contains("# did 2889 00"));
        assert!(text.contains("header 621 has no recorded response id"));
        assert!(text.contains("# 43010A7E"));
        assert!(text.contains("header 7DF has no recorded response id"));
        assert!(!text.contains("dtc"));
        assert!(!text.contains("7F0331"));
        assert!(!text.contains("req=621"));
        assert_eq!(text.matches("ecu ").count(), 1);
        assert!(clone_profile(&[]).contains("# no positive reply"));
    }
}
