//! Scaled fields for six `$22` reads that are already in the scan table.
//!
//! A reply is a positive `$22` payload: `62`, the identifier, then the data bytes.
//! `002F` is the first data byte divided by 255. `8334` is that byte times 100 divided
//! by 255. `43AF` is the big-endian uint16 times 100 divided by 65535. `41A3` is the
//! uint16 divided by 10. `41A6` is the uint16 divided by 64, printed in kilometres.
//! `434F` is the first data byte minus 40, printed in Celsius. Scaling a reply does
//! not open a socket.

use crate::elm::{payload, Error, Session};
use crate::scan::{command_lines, scan_table, ScanRow, INIT};

/// Gauge identifiers, in the order the command sends and prints them.
const GAUGE_IDS: [u16; 6] = [0x002F, 0x8334, 0x43AF, 0x41A3, 0x41A6, 0x434F];

/// Init lines, then the six gauge reads with the headers from the scan table.
#[must_use]
pub fn gauge_session_lines() -> Vec<String> {
    let rows = gauge_rows();
    INIT.into_iter()
        .map(str::to_string)
        .chain(command_lines(&rows))
        .collect()
}

fn gauge_rows() -> [ScanRow; 6] {
    GAUGE_IDS.map(|id| {
        scan_table()
            .iter()
            .find(|row| row.service == 0x22 && row.identifier == id)
            .copied()
            .expect("gauge did is in the scan table")
    })
}

/// Send the gauge reads and return the scaled printout.
///
/// # Errors
///
/// Returns the session error when a line cannot be written or the prompt does not arrive.
pub fn exchange_gauges(session: &mut Session) -> Result<String, Error> {
    let mut payloads = Vec::with_capacity(GAUGE_IDS.len());
    for line in gauge_session_lines() {
        let reply = session.exchange(&line)?;
        if line.starts_with("22") {
            payloads.push(payload(&line, &reply.lines));
        }
    }
    debug_assert_eq!(payloads.len(), GAUGE_IDS.len());
    Ok(format_gauges(&payloads))
}

fn format_gauges(payloads: &[Option<Vec<u8>>]) -> String {
    let mut out = String::new();
    for (i, id) in GAUGE_IDS.iter().enumerate() {
        let bytes = payloads.get(i).and_then(|p| p.as_deref());
        out.push_str(&field_line(*id, bytes));
        out.push('\n');
    }
    out
}

fn field_line(id: u16, payload: Option<&[u8]>) -> String {
    match payload.and_then(|bytes| scaled(id, bytes)) {
        Some(text) => text,
        None => format!("{id:04X} no data"),
    }
}

fn scaled(id: u16, bytes: &[u8]) -> Option<String> {
    if bytes.first() != Some(&0x62) || bytes.len() < 3 {
        return None;
    }
    let got = u16::from_be_bytes([bytes[1], bytes[2]]);
    if got != id {
        return None;
    }
    let data = &bytes[3..];
    match id {
        0x002F => Some(format!(
            "{id:04X} {}",
            fmt_div(u32::from(*data.first()?), 255)
        )),
        0x8334 => Some(format!(
            "{id:04X} {}",
            fmt_div(u32::from(*data.first()?) * 100, 255)
        )),
        0x43AF => Some(format!(
            "{id:04X} {}",
            fmt_div(u32::from(word(data)?) * 100, 65_535)
        )),
        0x41A3 => Some(format!("{id:04X} {}", fmt_div(u32::from(word(data)?), 10))),
        0x41A6 => Some(format!(
            "{id:04X} {} kilometres",
            fmt_div(u32::from(word(data)?), 64)
        )),
        0x434F => Some(format!(
            "{id:04X} {} Celsius",
            i32::from(*data.first()?) - 40
        )),
        _ => None,
    }
}

fn word(data: &[u8]) -> Option<u16> {
    Some(u16::from_be_bytes([*data.first()?, *data.get(1)?]))
}

/// Decimal of `num / den`, up to 12 fractional digits, truncated toward zero.
fn fmt_div(num: u32, den: u32) -> String {
    assert!(den != 0, "scale denominator");
    let whole = num / den;
    let mut rem = num % den;
    if rem == 0 {
        return whole.to_string();
    }
    let mut frac = String::new();
    for _ in 0..12 {
        rem *= 10;
        let digit = u8::try_from(rem / den).expect("one digit");
        frac.push(char::from(b'0' + digit));
        rem %= den;
        if rem == 0 {
            break;
        }
    }
    format!("{whole}.{frac}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elm::{classify, Line};

    const LINES: &[&str] = &[
        "ATZ", "ATL0", "ATH0", "ATS0", "ATSP6", "ATE0", "ATCAF1", "ATSH7E0", "22002F", "ATSH7E4",
        "228334", "2243AF", "2241A3", "2241A6", "22434F",
    ];

    #[test]
    fn gauge_lines_are_the_six_mapped_reads() {
        let lines = gauge_session_lines();
        assert_eq!(lines, LINES);
        let header = |id: u16| {
            let rows: Vec<_> = scan_table()
                .iter()
                .filter(|row| row.service == 0x22 && row.identifier == id)
                .collect();
            assert_eq!(rows.len(), 1, "{id:04X}");
            rows[0].header
        };
        assert_eq!(header(0x002F), 0x7E0);
        for id in [0x8334u16, 0x43AF, 0x41A3, 0x41A6, 0x434F] {
            assert_eq!(header(id), 0x7E4, "{id:04X}");
        }
        let reads: Vec<_> = lines
            .iter()
            .filter(|line| matches!(classify(line), Ok(Line::Read(_))))
            .cloned()
            .collect();
        assert_eq!(
            reads,
            ["22002F", "228334", "2243AF", "2241A3", "2241A6", "22434F"]
        );
        for line in &lines {
            match classify(line) {
                Ok(Line::Adapter) | Ok(Line::Read(0x22)) => {}
                other => panic!("{line} classified as {other:?}"),
            }
            for banned in ["AE", "3B", "27", "2E", "2F", "31"] {
                assert!(!line.starts_with(banned), "{line} starts with {banned}");
            }
        }
    }

    #[test]
    fn scaled_fields_come_from_reply_bytes_without_a_socket() {
        let payloads = [
            Some(vec![0x62, 0x00, 0x2F, 0x6C]),
            Some(vec![0x62, 0x83, 0x34, 0x80, 0x00]),
            Some(vec![0x62, 0x43, 0xAF, 0xF8, 0x00]),
            Some(vec![0x62, 0x41, 0xA3, 0x01, 0xC2]),
            Some(vec![0x62, 0x41, 0xA6, 0x10, 0x18]),
            Some(vec![0x62, 0x43, 0x4F, 0x41, 0x00]),
        ];
        assert_eq!(
            format_gauges(&payloads),
            "\
002F 0.423529411764
8334 50.196078431372
43AF 96.876478217746
41A3 45
41A6 64.375 kilometres
434F 25 Celsius
"
        );

        assert_eq!(
            field_line(0x8334, Some(&[0x62, 0x83, 0x34, 0x02, 0x00])),
            "8334 0.784313725490"
        );
        assert_eq!(
            field_line(0x434F, Some(&[0x62, 0x43, 0x4F, 0x06])),
            "434F -34 Celsius"
        );
        assert_eq!(
            field_line(0x434F, Some(&[0x62, 0x43, 0x4F, 0x00, 0x41])),
            "434F -40 Celsius"
        );
        assert_eq!(
            field_line(0x002F, Some(&[0x62, 0x00, 0x2F, 0x00])),
            "002F 0"
        );
        assert_eq!(
            field_line(0x002F, Some(&[0x62, 0x00, 0x2F, 0xFF])),
            "002F 1"
        );
        assert_eq!(
            field_line(0x8334, Some(&[0x62, 0x83, 0x34, 0xFF])),
            "8334 100"
        );
        assert_eq!(
            field_line(0x43AF, Some(&[0x62, 0x43, 0xAF, 0xFF, 0xFF])),
            "43AF 100"
        );
        assert_eq!(
            field_line(0x41A3, Some(&[0x62, 0x41, 0xA3, 0x05, 0xDC])),
            "41A3 150"
        );
        assert_eq!(
            field_line(0x41A3, Some(&[0x62, 0x41, 0xA3, 0x00, 0x01])),
            "41A3 0.1"
        );
        assert_eq!(
            field_line(0x41A6, Some(&[0x62, 0x41, 0xA6, 0x00, 0x01])),
            "41A6 0.015625 kilometres"
        );
        assert_eq!(field_line(0x002F, None), "002F no data");
        assert_eq!(
            field_line(0x43AF, Some(&[0x62, 0x43, 0xAF, 0xF8])),
            "43AF no data"
        );
        assert_eq!(
            field_line(0x002F, Some(&[0x62, 0x83, 0x34, 0x80])),
            "002F no data"
        );
        assert_eq!(
            field_line(0x434F, Some(&[0x7F, 0x22, 0x11])),
            "434F no data"
        );

        let mut shifted: [Option<Vec<u8>>; 6] = Default::default();
        shifted[0] = Some(vec![0x62, 0x00, 0x2F, 0x6C]);
        shifted[2] = Some(vec![0x62, 0x43, 0xAF, 0xF8, 0x00]);
        let lines: Vec<_> = format_gauges(&shifted)
            .lines()
            .map(str::to_string)
            .collect();
        assert_eq!(lines[0], "002F 0.423529411764");
        assert_eq!(lines[1], "8334 no data");
        assert_eq!(lines[2], "43AF 96.876478217746");
        assert_eq!(lines[3], "41A3 no data");
    }
}
