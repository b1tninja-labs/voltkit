//! Print the Wi-Fi read session against a loopback ELM327-over-TCP peer.

use std::process::ExitCode;
use voltkit::{hex_spaced, payload, session_lines, split_reply, Session};

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
    let Some(addr) = args.next() else {
        return Err("usage: voltkit HOST:PORT".to_string());
    };
    if args.next().is_some() || addr.starts_with('-') {
        return Err("usage: voltkit HOST:PORT".to_string());
    }
    println!("# voltkit");
    println!("# peer: {addr}");
    let mut session = Session::connect(&addr).map_err(|e| e.to_string())?;
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
