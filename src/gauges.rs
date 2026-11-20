//! Scaled fields for six `$22` reads that are already in the scan table.
//!
//! A reply is a positive `$22` payload: `62`, the identifier, then the data bytes.
//! The session asks for those six identifiers and no others. `002F` is the first data
//! byte divided by 255. `8334` is that byte times 100 divided by 255. `43AF` is the
//! big-endian uint16 times 100 divided by 65535. `41A3` is the uint16 divided by 10,
//! and that capacity divided by 51.8 Ah times 100. `41A6` is the uint16 divided by 64,
//! printed in kilometres, then times 0.62137119 and printed in miles. `434F` is the
//! first data byte minus 40, printed in Celsius and in Fahrenheit.
//!
//! The same decoder applies the other scales when a reply is already in hand. The
//! session does not ask for those identifiers. `40E9` is the uint16 divided by 2.
//! `43A6` is the first byte times 25. `43A5` is the uint16. `4470` is the uint16
//! divided by 10. `41C5` and `41C6` are four bytes divided by 64. `41AD` is the
//! uint16 times 0.125. `45FF` is the uint16 divided by 100. `4181` through `4240` are
//! the uint16 times 5 divided by 65535, and a zero word is not a voltage. `432A` and
//! `432C` are the first byte minus 1. `82A3` is the first byte times 2. `40D4` is the
//! signed int16 divided by 20 and then negated. Watts are that voltage times that
//! current. Scaling a reply does not open a socket.

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
            fmt_div(u64::from(*data.first()?), 255)
        )),
        0x8334 => Some(format!(
            "{id:04X} {}",
            fmt_div(u64::from(*data.first()?) * 100, 255)
        )),
        0x43AF => Some(format!(
            "{id:04X} {}",
            fmt_div(u64::from(word(data)?) * 100, 65_535)
        )),
        0x41A3 => {
            let raw = word(data)?;
            Some(format!(
                "{id:04X} {} {} percent",
                fmt_div(u64::from(raw), 10),
                fmt_div(u64::from(raw) * 100, 518)
            ))
        }
        0x41A6 => {
            let raw = word(data)?;
            Some(format!(
                "{id:04X} {} kilometres {} miles",
                fmt_div(u64::from(raw), 64),
                fmt_div(u64::from(raw) * 62_137_119, 6_400_000_000)
            ))
        }
        0x434F => {
            let celsius = i32::from(*data.first()?) - 40;
            Some(format!(
                "{id:04X} {celsius} Celsius {} Fahrenheit",
                fahrenheit(celsius)
            ))
        }
        0x40E9 => Some(format!("{id:04X} {}", fmt_div(u64::from(word(data)?), 2))),
        0x43A6 => Some(format!("{id:04X} {}", u32::from(*data.first()?) * 25)),
        0x43A5 => Some(format!("{id:04X} {}", word(data)?)),
        0x4470 => Some(format!("{id:04X} {}", fmt_div(u64::from(word(data)?), 10))),
        0x41C5 | 0x41C6 => Some(format!("{id:04X} {}", fmt_div(u64::from(dword(data)?), 64))),
        0x41AD => Some(format!("{id:04X} {}", fmt_div(u64::from(word(data)?), 8))),
        0x45FF => Some(format!("{id:04X} {}", fmt_div(u64::from(word(data)?), 100))),
        0x4181..=0x4240 => {
            let raw = word(data)?;
            if raw == 0 {
                None
            } else {
                Some(format!("{id:04X} {}", fmt_div(u64::from(raw) * 5, 65_535)))
            }
        }
        0x432A | 0x432C => Some(format!("{id:04X} {}", i32::from(*data.first()?) - 1)),
        0x82A3 => Some(format!("{id:04X} {}", u32::from(*data.first()?) * 2)),
        0x40D4 => {
            let raw = i32::from(i16::from_be_bytes([*data.first()?, *data.get(1)?]));
            Some(format!("{id:04X} {}", fmt_signed_div(-raw, 20)))
        }
        _ => None,
    }
}

fn word(data: &[u8]) -> Option<u16> {
    Some(u16::from_be_bytes([*data.first()?, *data.get(1)?]))
}

fn dword(data: &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes([
        *data.first()?,
        *data.get(1)?,
        *data.get(2)?,
        *data.get(3)?,
    ]))
}

/// `(celsius * 9 / 5) + 32`, with division truncating toward zero.
///
/// −34 °C is −29 °F.
fn fahrenheit(celsius: i32) -> i32 {
    celsius * 9 / 5 + 32
}

/// Decimal of `num / den`, up to 12 fractional digits, truncated toward zero.
fn fmt_div(num: u64, den: u64) -> String {
    assert!(den != 0, "scale denominator");
    let whole = num / den;
    let mut rem = u128::from(num % den);
    let den = u128::from(den);
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

/// Decimal of `num / den`, truncated toward zero, with a minus when `num` is negative.
fn fmt_signed_div(num: i32, den: u32) -> String {
    let text = fmt_div(u64::from(num.unsigned_abs()), u64::from(den));
    if num < 0 {
        format!("-{text}")
    } else {
        text
    }
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
41A3 45 86.872586872586 percent
41A6 64.375 kilometres 40.00077035625 miles
434F 25 Celsius 77 Fahrenheit
"
        );

        assert_eq!(
            field_line(0x8334, Some(&[0x62, 0x83, 0x34, 0x02, 0x00])),
            "8334 0.784313725490"
        );
        assert_eq!(
            field_line(0x434F, Some(&[0x62, 0x43, 0x4F, 0x06])),
            "434F -34 Celsius -29 Fahrenheit"
        );
        assert_eq!(
            field_line(0x434F, Some(&[0x62, 0x43, 0x4F, 0x00, 0x41])),
            "434F -40 Celsius -40 Fahrenheit"
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
            "41A3 150 289.575289575289 percent"
        );
        assert_eq!(
            field_line(0x41A3, Some(&[0x62, 0x41, 0xA3, 0x00, 0x01])),
            "41A3 0.1 0.193050193050 percent"
        );
        assert_eq!(
            field_line(0x41A6, Some(&[0x62, 0x41, 0xA6, 0x00, 0x01])),
            "41A6 0.015625 kilometres 0.009708924843 miles"
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

    #[test]
    fn table_scales_come_from_reply_bytes_without_a_socket() {
        let cases: &[(u16, &[u8], &str)] = &[
            (0x40E9, &[0x62, 0x40, 0xE9, 0x01, 0x90], "40E9 200"),
            (0x40E9, &[0x62, 0x40, 0xE9, 0x05, 0x78], "40E9 700"),
            (0x40E9, &[0x62, 0x40, 0xE9, 0x00, 0x01], "40E9 0.5"),
            (0x43A6, &[0x62, 0x43, 0xA6, 0x0C, 0x00], "43A6 300"),
            (0x43A6, &[0x62, 0x43, 0xA6, 0x08], "43A6 200"),
            (0x43A5, &[0x62, 0x43, 0xA5, 0x03, 0x20], "43A5 800"),
            (0x43A5, &[0x62, 0x43, 0xA5, 0x08, 0x34], "43A5 2100"),
            (0x4470, &[0x62, 0x44, 0x70, 0x00, 0x78], "4470 12"),
            (0x4470, &[0x62, 0x44, 0x70, 0x00, 0x01], "4470 0.1"),
            (
                0x41C5,
                &[0x62, 0x41, 0xC5, 0x00, 0x00, 0x02, 0x80],
                "41C5 10",
            ),
            (
                0x41C5,
                &[0x62, 0x41, 0xC5, 0x00, 0x00, 0x00, 0x01],
                "41C5 0.015625",
            ),
            (
                0x41C6,
                &[0x62, 0x41, 0xC6, 0x00, 0x00, 0x00, 0x00],
                "41C6 0",
            ),
            (0x41AD, &[0x62, 0x41, 0xAD, 0x00, 0x00], "41AD 0"),
            (0x41AD, &[0x62, 0x41, 0xAD, 0x00, 0x01], "41AD 0.125"),
            (
                0x4181,
                &[0x62, 0x41, 0x81, 0xB0, 0x00],
                "4181 3.437552452887",
            ),
            (
                0x4181,
                &[0x62, 0x41, 0x81, 0x00, 0x01],
                "4181 0.000076295109",
            ),
            (0x4181, &[0x62, 0x41, 0x81, 0x00, 0x00], "4181 no data"),
            (
                0x4182,
                &[0x62, 0x41, 0x82, 0xB0, 0x00],
                "4182 3.437552452887",
            ),
            (
                0x4240,
                &[0x62, 0x42, 0x40, 0xBB, 0xE0],
                "4240 3.669489585717",
            ),
            (0x4241, &[0x62, 0x42, 0x41, 0xB0, 0x00], "4241 no data"),
            (0x45FF, &[0x62, 0x45, 0xFF, 0x01, 0xC2], "45FF 4.5"),
            (0x45FF, &[0x62, 0x45, 0xFF, 0x00, 0x01], "45FF 0.01"),
            (0x432A, &[0x62, 0x43, 0x2A, 0x01], "432A 0"),
            (0x432A, &[0x62, 0x43, 0x2A, 0x00], "432A -1"),
            (0x432C, &[0x62, 0x43, 0x2C, 0x01], "432C 0"),
            (0x82A3, &[0x62, 0x82, 0xA3, 0x0A], "82A3 20"),
            (0x82A3, &[0x62, 0x82, 0xA3, 0xFF], "82A3 510"),
            (0x40D4, &[0x62, 0x40, 0xD4, 0x00, 0x14], "40D4 -1"),
            (0x40D4, &[0x62, 0x40, 0xD4, 0xFF, 0xEC], "40D4 1"),
            (0x40D4, &[0x62, 0x40, 0xD4, 0x00, 0x01], "40D4 -0.05"),
            (0x40D4, &[0x62, 0x40, 0xD4, 0xFF, 0xFF], "40D4 0.05"),
            (0x40D4, &[0x62, 0x40, 0xD4, 0x00, 0x00], "40D4 0"),
            (0x40D4, &[0x62, 0x40, 0xD4, 0x80, 0x00], "40D4 1638.4"),
            (0x40D4, &[0x62, 0x40, 0xD4, 0x7F, 0xFF], "40D4 -1638.35"),
            (0x40E9, &[0x62, 0x40, 0xE9, 0x01], "40E9 no data"),
            (
                0x41C5,
                &[0x62, 0x41, 0xC5, 0x00, 0x00, 0x02],
                "41C5 no data",
            ),
            (0x40D4, &[0x62, 0x40, 0xD4, 0x00], "40D4 no data"),
        ];
        for (id, bytes, expect) in cases {
            assert_eq!(field_line(*id, Some(bytes)), *expect);
        }
        assert_eq!(scaled(0x1ADF, &[0x5A, 0xDF, 0x00, 0x00, 0x02, 0x80]), None);
        // (max − min) volts, in millivolts, for the lab cell ramp.
        let span = (0xBBE0u64 - 0xB000) * 5 * 1000;
        assert_eq!(fmt_div(span, 65_535), "231.937132829785");
    }
}
