//! `voltkit bench` against a stub binary. Nothing here opens a serial port,
//! a Bluetooth radio, a vehicle, or the dangle tree.

mod common;

use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use voltkit::session_lines;

const STUB_SRC: &str = r#"use std::io::{Read, Write};

fn main() {
    let log = std::env::var("VOLTKIT_BENCH_STUB_LOG").expect("VOLTKIT_BENCH_STUB_LOG");
    let mut recorded = String::new();
    for arg in std::env::args().skip(1) {
        recorded.push_str(&arg);
        recorded.push('\n');
    }
    if let Ok(listen) = std::env::var("DANGLE_LISTEN") {
        if !listen.is_empty() {
            recorded.push_str("DANGLE_LISTEN=");
            recorded.push_str(&listen);
            recorded.push('\n');
        }
    }
    std::fs::write(&log, &recorded).unwrap();
    let mut err = std::io::stderr();
    let listener = match std::net::TcpListener::bind("127.0.0.1:35000") {
        Ok(listener) => listener,
        Err(_) => {
            writeln!(err, "not bound").unwrap();
            err.flush().unwrap();
            std::process::exit(1);
        }
    };
    writeln!(err, "listening on 127.0.0.1:35000").unwrap();
    err.flush().unwrap();
    let (mut sock, _) = listener.accept().unwrap();
    sock.set_read_timeout(Some(std::time::Duration::from_secs(3)))
        .unwrap();
    let seen = format!("{log}.seen");
    std::fs::write(&seen, "").unwrap();
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    loop {
        match sock.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                while let Some(i) = buf.iter().position(|b| *b == b'\r') {
                    let line = buf.drain(..=i).collect::<Vec<_>>();
                    if line.contains(&b'\n') {
                        break;
                    }
                    let text = String::from_utf8(line[..line.len() - 1].to_vec()).unwrap();
                    let mut file = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&seen)
                        .unwrap();
                    writeln!(file, "{text}").unwrap();
                    file.flush().unwrap();
                    sock.write_all(b"OK\0\r>").unwrap();
                }
            }
            Err(err)
                if err.kind() == std::io::ErrorKind::TimedOut
                    || err.kind() == std::io::ErrorKind::WouldBlock =>
            {
                break;
            }
            Err(_) => break,
        }
    }
}
"#;

#[test]
fn the_bench_command_starts_dangle_named_obdlink_and_dials_127_0_0_1_35000() {
    let stub = stub_program();
    let _port = common::hold_35000();
    assert_port_free();
    let log = std::env::temp_dir().join(format!("voltkit-bench-dial-{}.log", std::process::id()));
    let seen = PathBuf::from(format!("{}.seen", log.display()));
    let _ = std::fs::remove_file(&log);
    let _ = std::fs::remove_file(&seen);

    let output = Command::new(env!("CARGO_BIN_EXE_voltkit"))
        .arg("bench")
        .env("VOLTKIT_BENCH_PEER", &stub)
        .env("VOLTKIT_BENCH_STUB_LOG", &log)
        .env("DANGLE_LISTEN", "127.0.0.1:9")
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.starts_with("# voltkit\n# peer: 127.0.0.1:35000\n"),
        "{stdout}"
    );
    assert!(stdout.contains("> ATZ\n< OK\n"), "{stdout}");
    assert!(
        !stdout.lines().any(|line| line.starts_with("> 27")),
        "{stdout}"
    );
    assert!(stderr.contains("listening on 127.0.0.1:35000"), "{stderr}");

    let recorded = std::fs::read_to_string(&log).unwrap();
    assert_eq!(recorded.lines().collect::<Vec<_>>(), ["--name", "OBDLink"]);
    let got: Vec<String> = std::fs::read_to_string(&seen)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(got, session_lines());
    for line in &got {
        assert!(is_adapter_or_read(line), "{line}");
    }
    let _ = std::fs::remove_file(&log);
    let _ = std::fs::remove_file(&seen);
    assert_port_free();
}

#[test]
fn bench_does_not_dial_a_listener_already_bound_on_127_0_0_1_35000() {
    let stub = stub_program();
    let _port = common::hold_35000();
    assert_port_free();
    let listener = TcpListener::bind("127.0.0.1:35000").unwrap();
    let log = std::env::temp_dir().join(format!("voltkit-bench-bound-{}.log", std::process::id()));
    let seen = PathBuf::from(format!("{}.seen", log.display()));
    let _ = std::fs::remove_file(&log);
    let _ = std::fs::remove_file(&seen);

    let output = Command::new(env!("CARGO_BIN_EXE_voltkit"))
        .arg("bench")
        .env("VOLTKIT_BENCH_PEER", &stub)
        .env("VOLTKIT_BENCH_STUB_LOG", &log)
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        !output.status.success(),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("not bound"), "{stderr}");
    listener.set_nonblocking(true).unwrap();
    match listener.accept() {
        Ok(_) => panic!("dialed the listener that was already bound on 127.0.0.1:35000"),
        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
        Err(err) => panic!("accept: {err}"),
    }
    let recorded = std::fs::read_to_string(&log).unwrap();
    assert_eq!(recorded.lines().collect::<Vec<_>>(), ["--name", "OBDLink"]);
    assert!(!seen.is_file(), "the stub accepted a session");
    drop(listener);
    let _ = std::fs::remove_file(&log);
    assert_port_free();
}

#[test]
fn bench_takes_no_address_argument() {
    let output = Command::new(env!("CARGO_BIN_EXE_voltkit"))
        .args(["bench", "127.0.0.1:9"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let err = String::from_utf8(output.stderr).unwrap();
    assert!(err.contains("voltkit bench"), "{err}");
}

fn is_adapter_or_read(line: &str) -> bool {
    let compact: String = line.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    let upper = compact.to_ascii_uppercase();
    if upper.len() >= 2 && upper.as_bytes()[..2] == *b"AT" {
        return true;
    }
    let body = if upper.len() % 2 == 1 {
        &upper[..upper.len() - 1]
    } else {
        upper.as_str()
    };
    matches!(body.get(..2), Some("01" | "03" | "1A" | "22"))
}

fn assert_port_free() {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match TcpListener::bind("127.0.0.1:35000") {
            Ok(listener) => {
                drop(listener);
                return;
            }
            Err(err) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(20));
                let _ = err;
            }
            Err(err) => panic!("127.0.0.1:35000 is still in use: {err}"),
        }
    }
}

fn stub_program() -> PathBuf {
    static COMPILE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = COMPILE.lock().unwrap();
    let dir = std::env::temp_dir().join("voltkit-bench-peer-stub");
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("stub.rs");
    let exe = dir.join("stub.exe");
    if std::fs::read_to_string(&src).ok().as_deref() != Some(STUB_SRC) || !exe.is_file() {
        std::fs::write(&src, STUB_SRC).unwrap();
        let out = Command::new("rustc")
            .current_dir(&dir)
            .args(["--edition", "2021", "-o"])
            .arg(&exe)
            .arg(&src)
            .output()
            .expect("rustc is installed with the toolchain");
        assert!(
            out.status.success(),
            "rustc stub failed\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    exe
}
