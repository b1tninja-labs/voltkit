//! The session against an in-process TCP peer. Nothing here listens outside loopback.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use voltkit::{session_lines, Error, Refuse, Session, INIT};

#[test]
fn the_session_is_carriage_return_lines_and_does_not_send_security_access() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&seen);
    let server = thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        sock.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut buf = Vec::new();
        let mut tmp = [0u8; 512];
        loop {
            match sock.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    while let Some(i) = buf.iter().position(|b| *b == b'\r') {
                        let line = buf.drain(..=i).collect::<Vec<_>>();
                        assert!(!line.contains(&b'\n'), "request contained a linefeed");
                        assert_eq!(*line.last().unwrap(), b'\r');
                        let text = String::from_utf8(line[..line.len() - 1].to_vec()).unwrap();
                        record.lock().unwrap().push(text);
                        sock.write_all(b"OK\0\r>").unwrap();
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                Err(e) => panic!("{e}"),
            }
        }
    });

    let mut session = Session::connect(&addr.to_string()).unwrap();
    let lines = session_lines();
    for line in &lines {
        let reply = session.exchange(line).unwrap();
        assert_eq!(reply.lines, vec!["OK".to_string()]);
        assert!(!reply.raw.contains(&0), "NUL bytes are dropped");
    }
    drop(session);
    server.join().unwrap();
    let got = seen.lock().unwrap().clone();
    assert_eq!(got, lines);
    assert!(got.iter().take(INIT.len()).map(String::as_str).eq(INIT));
    assert!(!got.iter().any(|l| l.starts_with("27")));
}

#[test]
fn security_access_is_refused_without_writing() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(0u32));
    let count = Arc::clone(&seen);
    let server = thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        sock.set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let mut tmp = [0u8; 64];
        match sock.read(&mut tmp) {
            Ok(0) | Err(_) => {}
            Ok(n) => {
                *count.lock().unwrap() += u32::try_from(n).unwrap();
            }
        }
    });
    let mut session = Session::connect(&addr.to_string()).unwrap();
    session
        .set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    let err = session.exchange("2701").unwrap_err();
    assert!(matches!(err, Error::Refuse(Refuse::SecurityAccess)));
    drop(session);
    server.join().unwrap();
    assert_eq!(*seen.lock().unwrap(), 0);
}

#[test]
fn a_non_loopback_address_is_refused() {
    let err = Session::connect("8.8.8.8:35000").unwrap_err();
    assert!(matches!(err, Error::NotLoopback));
}
