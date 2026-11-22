//! Print a Voltage read session, the six scaled gauge fields, or a profile capture.

mod tui;

use std::path::PathBuf;
use std::process::ExitCode;

use voltkit::{
    clone_profile, exchange_gauges, hex_spaced, lines_for, payload, profile_note, select_rows,
    session_lines, split_reply, Read, Session, SETS,
};

const USAGE: &str = "\
usage: voltkit wizard
       voltkit lab
       voltkit bench
       voltkit HOST:PORT
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
    // No command runs the wizard, so double-clicking voltkit.exe opens it with no
    // terminal and no typed command.
    let Some(first) = args.next() else {
        return run_wizard();
    };
    if first == "wizard" {
        if args.next().is_some() {
            return Err(USAGE.to_string());
        }
        return run_wizard();
    }
    if first == "gauges" {
        return gauges(args);
    }
    if first == "profile" {
        return profile(args);
    }
    if first == "lab" {
        if args.next().is_some() {
            return Err(USAGE.to_string());
        }
        return lab();
    }
    if first == "bench" {
        if args.next().is_some() {
            return Err(USAGE.to_string());
        }
        return bench();
    }
    if args.next().is_some() || first.starts_with('-') {
        return Err(USAGE.to_string());
    }
    let session = Session::connect(&first).map_err(|e| e.to_string())?;
    print_session(&first, session)
}

/// Run the wizard: the full-screen TUI on a terminal, or the line-prompt flow when
/// piped. This is the default when `voltkit` is launched with no command.
fn run_wizard() -> Result<(), String> {
    if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
        tui::run()
    } else {
        voltkit::wizard::run(|plan| run_capture(&profile_args(plan)))
    }
}

#[cfg(not(feature = "lab"))]
fn lab() -> Result<(), String> {
    let program = voltkit::lab::program(std::env::var_os(voltkit::lab::PROGRAM_ENV));
    let mut peer = voltkit::lab::LabPeer::start(&program).map_err(|e| e.to_string())?;
    let session = peer.dial().map_err(|e| e.to_string())?;
    print_session(voltkit::lab::ADDRESS, session)
}

// With the `lab` feature the bench is served in-process from the fakeobd library,
// so no `fakeobd` binary has to be on PATH.
#[cfg(feature = "lab")]
fn lab() -> Result<(), String> {
    let session = voltkit::lab::embed::start().map_err(|e| e.to_string())?;
    print_session(voltkit::lab::ADDRESS, session)
}

fn bench() -> Result<(), String> {
    let program = voltkit::bench::program(std::env::var_os(voltkit::bench::PROGRAM_ENV));
    let mut peer = voltkit::bench::BenchPeer::start(&program).map_err(|e| e.to_string())?;
    let session = peer.dial().map_err(|e| e.to_string())?;
    print_session(voltkit::bench::ADDRESS, session)
}

fn print_session(peer: &str, mut session: Session) -> Result<(), String> {
    println!("# voltkit");
    println!("# peer: {peer}");
    for line in session_lines() {
        let reply = session.exchange(&line).map_err(|e| e.to_string())?;
        println!("> {line}");
        for item in split_reply(&reply.raw) {
            println!("< {item}");
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
    output: Option<PathBuf>,
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
    run_capture(&parsed)
}

fn profile_args(plan: &voltkit::wizard::Plan) -> ProfileArgs {
    ProfileArgs {
        target: Some(plan.target.clone()),
        baud: plan.baud,
        sets: plan.sets.clone(),
        ranges: plan.ranges.clone(),
        clone: plan.clone,
        list: false,
        output: plan.output.clone(),
    }
}

struct CaptureReport {
    answered: u32,
    negative: u32,
    silent: u32,
    profile: Option<String>,
}

fn run_capture(parsed: &ProfileArgs) -> Result<(), String> {
    let report = run_capture_logged(parsed, |line| emit(parsed.clone, line))?;
    finish_clone(parsed, report)
}

fn run_capture_logged(
    parsed: &ProfileArgs,
    mut log: impl FnMut(&str),
) -> Result<CaptureReport, String> {
    let Some(target) = parsed.target.as_deref() else {
        return Err(USAGE.to_string());
    };
    if let Some(path) = &parsed.output {
        if path.is_dir() {
            return Err(format!("{} is a directory", path.display()));
        }
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                return Err(format!("directory {} does not exist", parent.display()));
            }
        }
    }
    let rows = select_rows(&parsed.sets, &parsed.ranges)?;
    let mut session = open_profile(target, parsed.baud)?;
    let mut reads = rows.iter();
    let mut captured = Vec::new();
    let mut answered = 0_u32;
    let mut negative = 0_u32;
    let mut silent = 0_u32;
    for line in lines_for(&rows) {
        let reply = session.exchange(&line).map_err(|e| e.to_string())?;
        log(&format!("> {line}"));
        for item in split_reply(&reply.raw) {
            log(&format!("< {item}"));
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
                log(&format!("= {}", hex_spaced(&bytes)));
                // Mode `03` answers with the request SID raised by `0x40` (so `0x43`), the
                // positive-response offset `udskit::services` owns.
                let mode03_positive = udskit::services::positive_response_sid(row.service).ok();
                if row.service == 0x03 && bytes.first().copied() == mode03_positive {
                    answered += 1;
                } else if let Some(note) = profile_note(row, &bytes) {
                    answered += 1;
                    log(&note);
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
    let profile = if parsed.clone {
        Some(clone_profile(&captured))
    } else {
        None
    };
    Ok(CaptureReport {
        answered,
        negative,
        silent,
        profile,
    })
}

fn finish_clone(parsed: &ProfileArgs, report: CaptureReport) -> Result<(), String> {
    if !parsed.clone {
        return Ok(());
    }
    eprintln!(
        "# answered {}, negative {}, silent {}",
        report.answered, report.negative, report.silent
    );
    let Some(text) = report.profile else {
        return Ok(());
    };
    if let Some(path) = &parsed.output {
        if let Err(err) = write_profile(path, &text) {
            print!("{text}");
            return Err(err);
        }
        eprintln!("# wrote {}", path.display());
    } else {
        print!("{text}");
    }
    Ok(())
}

fn write_profile(path: &std::path::Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|err| format!("could not write {}: {err}", path.display()))
}

fn emit(clone: bool, line: &str) {
    if clone {
        eprintln!("{line}");
    } else {
        println!("{line}");
    }
}

fn parse_profile(mut args: impl Iterator<Item = String>) -> Result<ProfileArgs, String> {
    let mut parsed = ProfileArgs {
        target: None,
        baud: None,
        sets: Vec::new(),
        ranges: Vec::new(),
        clone: false,
        list: false,
        output: None,
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
    if let Some(address) = voltkit::detect::parse_ble_target(target) {
        if baud.is_some() {
            return Err(USAGE.to_string());
        }
        return open_ble(address);
    }
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

/// Open a native BLE dongle at `address` over the host radio.
///
/// The real transport is Windows + the `ble` feature (the only BLE backend voltkit
/// has). A build without it still has to compile, so it reports that it was built
/// without BLE support, the way the VCX path reports a missing feature.
#[cfg(all(windows, feature = "ble"))]
fn open_ble(address: u64) -> Result<Session, String> {
    Session::connect_ble(address).map_err(|e| e.to_string())
}

#[cfg(not(all(windows, feature = "ble")))]
fn open_ble(_address: u64) -> Result<Session, String> {
    Err("voltkit was built without BLE support".to_string())
}
