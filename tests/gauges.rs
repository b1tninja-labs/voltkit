//! The gauge command against an in-process peer, and the host flag with no peer.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use voltkit::gauge_session_lines;

#[test]
fn gauges_print_the_scaled_fields() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&seen);
    let server = thread::spawn(move || {
        let mut sock = accept_for(&listener, Duration::from_secs(3));
        sock.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut buf = Vec::new();
        let mut tmp = [0u8; 512];
        loop {
            match sock.read(&mut tmp) {
                Ok(0) => {
                    assert!(buf.is_empty(), "truncated request");
                    break;
                }
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    while let Some(i) = buf.iter().position(|b| *b == b'\r') {
                        let line = buf.drain(..=i).collect::<Vec<_>>();
                        assert!(!line.contains(&b'\n'), "request contained a linefeed");
                        let text = String::from_utf8(line[..line.len() - 1].to_vec()).unwrap();
                        let reply: &[u8] = match text.as_str() {
                            "22002F" => b"62002F6C\r>",
                            "228334" => b"6283348000\r>",
                            "2243AF" => b"6243AFF800\r>",
                            "2241A3" => b"6241A301C2\r>",
                            "2241A6" => b"6241A61018\r>",
                            "22434F" => b"62434F4100\r>",
                            _ => b"OK\r>",
                        };
                        record.lock().unwrap().push(text);
                        sock.write_all(reply).unwrap();
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                Err(e) => panic!("{e}"),
            }
        }
    });

    let output = Command::new(env!("CARGO_BIN_EXE_voltkit"))
        .args(["gauges", "--host", &addr])
        .output()
        .unwrap();
    server.join().unwrap();
    assert!(
        output.status.success(),
        "stderr {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "\
002F 0.423529411764
8334 50.196078431372
43AF 96.876478217746
41A3 45
41A6 64.375 kilometres
434F 25 Celsius
"
    );
    let got = seen.lock().unwrap().clone();
    assert_eq!(got, gauge_session_lines());
    for line in &got {
        for banned in ["AE", "3B", "27", "2E", "2F", "31"] {
            assert!(!line.starts_with(banned), "{line}");
        }
    }
}

#[test]
fn gauges_do_not_connect_without_the_host_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_voltkit"))
        .arg("gauges")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(err.contains("--host"), "{err}");
    assert!(output.stdout.is_empty());
}

#[test]
fn a_gauge_host_other_than_127_0_0_1_is_refused() {
    for host in ["8.8.8.8:35000", "127.0.0.2:9", "[::1]:9"] {
        let output = Command::new(env!("CARGO_BIN_EXE_voltkit"))
            .args(["gauges", "--host", host])
            .output()
            .unwrap();
        assert!(!output.status.success(), "{host}");
        let err = String::from_utf8_lossy(&output.stderr);
        assert!(
            err.contains("gauge host must be 127.0.0.1"),
            "{host}: {err}"
        );
        assert!(output.stdout.is_empty(), "{host}");
    }
}

fn accept_for(listener: &TcpListener, limit: Duration) -> std::net::TcpStream {
    listener.set_nonblocking(true).unwrap();
    let start = Instant::now();
    loop {
        match listener.accept() {
            Ok((sock, _)) => {
                sock.set_nonblocking(false).unwrap();
                return sock;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if start.elapsed() > limit {
                    panic!("no gauge client");
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => panic!("{e}"),
        }
    }
}
