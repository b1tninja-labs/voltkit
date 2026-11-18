//! Print a Voltage read session, or the six scaled gauge fields, against a loopback peer.

use std::process::ExitCode;
use voltkit::{exchange_gauges, hex_spaced, payload, session_lines, split_reply, Session};

const USAGE: &str = "usage: voltkit HOST:PORT | voltkit gauges --host 127.0.0.1:PORT";

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
