//! Print a Voltage read session, the six scaled gauge fields, or a profile capture.

use std::process::ExitCode;
use voltkit::{
    capture_set, clone_profile, exchange_gauges, hex_spaced, lines_for, payload, profile_note,
    profile_table, range_rows, session_lines, split_reply, Read, Session, SETS,
};

const USAGE: &str = "\
usage: voltkit HOST:PORT
       voltkit gauges --host 127.0.0.1:PORT
       voltkit profile --list
       voltkit profile TARGET [--set NAME]... [--range HEADER 22|1A START END]... [--clone]
       TARGET is HOST:PORT, or on Windows COMn [--baud 115200]";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("voltkit: {e}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let Some(first) = args.next() else {
        return Err(USAGE.to_string());
    };
    if first == "gauges" {
        return gauges(args);
    }
    if first == "profile" {
        return profile(args);
    }
    if args.next().is_some() || first.starts_with('-') {
        return Err(USAGE.to_string());
    }
    println!("# voltkit");
    println!("# peer: {first}");
    let mut session = Session::connect(&first).map_err(|e| e.to_string())?;
    for line in session_lines() {
        let reply = session.exchange(&line).map_err(|e| e.to_string())?;
        println!("> {line}");
        for l in split_reply(&reply.raw) {
            println!("< {l}");
        }
        if let Some(bytes) = payload(&line, &reply.lines) {
            println!("= {}", hex_spaced(&bytes));
        }
    }
    Ok(())
}

fn gauges(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let Some(flag) = args.next() else {
        return Err(USAGE.to_string());
    };
    let Some(host) = args.next() else {
        return Err(USAGE.to_string());
    };
    if flag != "--host" || args.next().is_some() {
        return Err(USAGE.to_string());
    }
    let mut session = Session::connect_gauge_host(&host).map_err(|e| e.to_string())?;
    let report = exchange_gauges(&mut session).map_err(|e| e.to_string())?;
    print!("{report}");
    Ok(())
}

struct ProfileArgs {
    target: Option<String>,
    baud: Option<u32>,
    sets: Vec<String>,
    ranges: Vec<(u16, u8, u16, u16)>,
    clone: bool,
    list: bool,
}

fn profile(args: impl Iterator<Item = String>) -> Result<(), String> {
    let parsed = parse_profile(args)?;
    if parsed.list {
        for (name, about) in SETS {
            println!("{name:<12} {about}");
        }
        if parsed.target.is_none() {
            return Ok(());
        }
    }
    let Some(target) = parsed.target else {
        return Err(USAGE.to_string());
    };
    let rows = selected_rows(&parsed.sets, &parsed.ranges)?;
    let mut session = open_profile(&target, parsed.baud)?;
    let mut reads = rows.iter();
    let mut captured = Vec::new();
    let mut answered = 0_u32;
    let mut negative = 0_u32;
    let mut silent = 0_u32;
    for line in lines_for(&rows) {
        let reply = session.exchange(&line).map_err(|e| e.to_string())?;
        emit(parsed.clone, &format!("> {line}"));
        for item in split_reply(&reply.raw) {
            emit(parsed.clone, &format!("< {item}"));
        }
        let adapter = line.len() >= 2 && line.as_bytes()[..2].eq_ignore_ascii_case(b"AT");
        if adapter {
            continue;
        }
        let Some(row) = reads.next() else {
            return Err("capture rows and request lines diverged".to_string());
        };
        match payload(&line, &reply.lines) {
            Some(bytes) => {
                emit(parsed.clone, &format!("= {}", hex_spaced(&bytes)));
                if let Some(note) = profile_note(row, &bytes) {
                    answered += 1;
                    emit(parsed.clone, &note);
                } else {
                    negative += 1;
                }
                captured.push(Read {
                    row: *row,
                    data: bytes,
                });
            }
            None => silent += 1,
        }
    }
    if parsed.clone {
        eprintln!("# answered {answered}, negative {negative}, silent {silent}");
        print!("{}", clone_profile(&captured));
    }
    Ok(())
}

fn emit(clone: bool, line: &str) {
    if clone {
        eprintln!("{line}");
    } else {
        println!("{line}");
    }
}

fn selected_rows(
    sets: &[String],
    ranges: &[(u16, u8, u16, u16)],
) -> Result<Vec<voltkit::ScanRow>, String> {
    if sets.is_empty() && ranges.is_empty() {
        return Ok(profile_table());
    }
    let mut rows = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for name in sets {
        let Some(set) = capture_set(name) else {
            return Err(format!("unknown set {name}; voltkit profile --list"));
        };
        for row in set {
            if seen.insert((row.header, row.service, row.identifier)) {
                rows.push(row);
            }
        }
    }
    for (header, service, start, end) in ranges {
        for row in range_rows(*header, *service, *start, *end)? {
            if seen.insert((row.header, row.service, row.identifier)) {
                rows.push(row);
            }
        }
    }
    if rows.is_empty() {
        return Err("the capture has no reads".to_string());
    }
    Ok(rows)
}

fn parse_profile(mut args: impl Iterator<Item = String>) -> Result<ProfileArgs, String> {
    let mut parsed = ProfileArgs {
        target: None,
        baud: None,
        sets: Vec::new(),
        ranges: Vec::new(),
        clone: false,
        list: false,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--list" => parsed.list = true,
            "--clone" => parsed.clone = true,
            "--baud" => {
                let text = args.next().ok_or_else(|| USAGE.to_string())?;
                parsed.baud = Some(text.parse::<u32>().map_err(|_| USAGE.to_string())?);
            }
            "--set" => parsed
                .sets
                .push(args.next().ok_or_else(|| USAGE.to_string())?),
            "--range" => {
                let header = hex_id(&args.next().ok_or_else(|| USAGE.to_string())?)?;
                let service = match args.next().ok_or_else(|| USAGE.to_string())?.as_str() {
                    "22" => 0x22,
                    "1A" | "1a" => 0x1A,
                    _ => return Err(USAGE.to_string()),
                };
                let start = hex_id(&args.next().ok_or_else(|| USAGE.to_string())?)?;
                let end = hex_id(&args.next().ok_or_else(|| USAGE.to_string())?)?;
                parsed.ranges.push((header, service, start, end));
            }
            other if parsed.target.is_none() && !other.starts_with('-') => {
                parsed.target = Some(other.to_string());
            }
            _ => return Err(USAGE.to_string()),
        }
    }
    Ok(parsed)
}

fn hex_id(text: &str) -> Result<u16, String> {
    u16::from_str_radix(text, 16).map_err(|_| USAGE.to_string())
}

fn open_profile(target: &str, baud: Option<u32>) -> Result<Session, String> {
    if target.contains(':') {
        if baud.is_some() {
            return Err(USAGE.to_string());
        }
        return Session::connect_host(target).map_err(|e| e.to_string());
    }
    let port = target.to_ascii_uppercase();
    if !port.starts_with("COM") {
        return Err(USAGE.to_string());
    }
    #[cfg(windows)]
    {
        Session::connect_serial(&port, baud.unwrap_or(115_200)).map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = (port, baud);
        Err("profile serial is Windows only".to_string())
    }
}
