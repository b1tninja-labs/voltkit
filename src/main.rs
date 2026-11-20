//! Print a Voltage read session, the six scaled gauge fields, or a profile capture.

use std::process::ExitCode;
use voltkit::{
    exchange_gauges, hex_spaced, payload, profile_lines, profile_note, profile_table,
    session_lines, split_reply, Session,
};

const USAGE: &str = "\
usage: voltkit HOST:PORT
       voltkit gauges --host 127.0.0.1:PORT
       voltkit profile HOST:PORT
       voltkit profile COMn [--baud 115200]";

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

fn profile(args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut session = open_profile(args)?;
    let rows = profile_table();
    let mut reads = rows.iter();
    println!("# voltkit profile");
    for line in profile_lines() {
        let reply = session.exchange(&line).map_err(|e| e.to_string())?;
        println!("> {line}");
        for item in split_reply(&reply.raw) {
            println!("< {item}");
        }
        let adapter = line.len() >= 2 && line.as_bytes()[..2].eq_ignore_ascii_case(b"AT");
        if let Some(bytes) = payload(&line, &reply.lines) {
            println!("= {}", hex_spaced(&bytes));
            if !adapter {
                if let Some(note) = reads.next().and_then(|row| profile_note(row, &bytes)) {
                    println!("{note}");
                }
            }
        } else if !adapter {
            let _ = reads.next();
        }
    }
    Ok(())
}

fn open_profile(mut args: impl Iterator<Item = String>) -> Result<Session, String> {
    let Some(target) = args.next() else {
        return Err(USAGE.to_string());
    };
    let baud = match args.next() {
        None => None,
        Some(flag) if flag == "--baud" => {
            let Some(text) = args.next() else {
                return Err(USAGE.to_string());
            };
            if args.next().is_some() {
                return Err(USAGE.to_string());
            }
            Some(text.parse::<u32>().map_err(|_| USAGE.to_string())?)
        }
        Some(_) => return Err(USAGE.to_string()),
    };
    if target.contains(':') {
        if baud.is_some() {
            return Err(USAGE.to_string());
        }
        return Session::connect_host(&target).map_err(|e| e.to_string());
    }
    let port = target.to_ascii_uppercase();
    if !port.starts_with("COM") {
        return Err(USAGE.to_string());
    }
    if baud.is_some() && !cfg!(windows) {
        return Err(USAGE.to_string());
    }
    #[cfg(windows)]
    {
        Session::connect_serial(&port, baud.unwrap_or(115_200)).map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = baud;
        Err("profile serial is Windows only".to_string())
    }
}
