//! Launch the bench peer and dial it.
//!
//! The peer is the `dangle` TCP-bridge process on [`ADDRESS`]. `voltkit bench`
//! spawns that binary and opens a [`Session`] to it; this is the optional
//! bring-the-bridge path (for example to reach a radio from an emulator), beside
//! the wizard's native in-process BLE. `dangle` is not a crate dependency here —
//! its BLE *library* is what the wizard links; this command spawns its *binary*.
//! The command line is `--name OBDLink`. The `dangle` README default for
//! `--listen` is [`ADDRESS`], so this command does not pass `--listen`. An
//! inherited `DANGLE_LISTEN` would move that socket, and the child does not
//! receive the variable. Dial waits until the process writes a stderr line that
//! says it is listening on [`ADDRESS`]. `dangle` prints that line after the bind
//! succeeds. A listener already bound on the port is not this peer, and a
//! bind-failure line is not that report. voltkit does not open a Bluetooth radio,
//! a COM port, or a vehicle here. The session still refuses `$27` and the write
//! services before a byte is written.

use std::ffi::{OsStr, OsString};
use std::io::{self, Read, Write};
use std::process::{Child, ChildStderr, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crate::{Error, Session};

/// The loopback socket `dangle` listens on for the bench peer.
///
/// This is the README default. The command does not pass `--listen`.
pub const ADDRESS: &str = "127.0.0.1:35000";

/// Environment variable that names the `dangle` binary.
///
/// When it is unset, the command is `dangle` on `PATH`.
pub const PROGRAM_ENV: &str = "VOLTKIT_BENCH_PEER";

/// `program` when the operator passed one, otherwise `dangle`.
#[must_use]
pub fn program(override_path: Option<OsString>) -> OsString {
    override_path.unwrap_or_else(|| OsString::from("dangle"))
}

/// Arguments that start the bench peer.
///
/// `--name OBDLink` selects the BLE adapter by name. The listen address stays
/// the `dangle` default, [`ADDRESS`].
#[must_use]
pub fn bridge_args() -> [&'static str; 2] {
    ["--name", "OBDLink"]
}

/// A running `dangle --name OBDLink` process.
///
/// Dropping this value stops the process.
#[derive(Debug)]
#[must_use = "the bench peer is stopped when this value is dropped"]
pub struct BenchPeer {
    child: Child,
}

impl BenchPeer {
    /// Spawn `program` with [`bridge_args`].
    ///
    /// `DANGLE_LISTEN` is removed from the child so the README default
    /// stays [`ADDRESS`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the process cannot be started.
    pub fn start(program: impl AsRef<OsStr>) -> Result<Self, Error> {
        let program = program.as_ref();
        let child = Command::new(program)
            .args(bridge_args())
            .env_remove("DANGLE_LISTEN")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| {
                Error::Io(io::Error::other(format!(
                    "could not start bench peer {}: {err}",
                    program.to_string_lossy()
                )))
            })?;
        Ok(Self { child })
    }

    /// Dial [`ADDRESS`] after the peer reports that it is listening there.
    ///
    /// `dangle` prints `listening on 127.0.0.1:35000` after the listen
    /// socket exists. A connection is not opened before that line, so some
    /// other process already bound to the port is not treated as this peer.
    /// A later line, or a line about a different socket, does not count.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the peer exits, closes stderr without that
    /// line, or does not accept [`ADDRESS`] once it has reported that it is
    /// listening.
    pub fn dial(&mut self) -> Result<Session, Error> {
        self.wait_listening()?;
        let session = Session::connect(ADDRESS)?;
        if let Some(status) = self.child.try_wait()? {
            return Err(exited(status));
        }
        Ok(session)
    }

    /// Wait until the peer reports it is listening on [`ADDRESS`], without dialing.
    ///
    /// [`dial`](Self::dial) calls this before it opens the socket, so a listener some
    /// other process already bound to the port is not treated as this peer.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the peer exits or closes stderr before it reports
    /// that it is listening.
    fn wait_listening(&mut self) -> Result<(), Error> {
        let stderr =
            self.child.stderr.take().ok_or_else(|| {
                Error::Io(io::Error::other("bench peer stderr was already taken"))
            })?;
        let ready = wait_until_listening(stderr);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait()? {
                return Err(exited(status));
            }
            match ready.recv_timeout(Duration::from_millis(50)) {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(err)) => return Err(Error::Io(err)),
                Err(mpsc::RecvTimeoutError::Timeout) if Instant::now() < deadline => continue,
                Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(Error::Io(io::Error::other(format!(
                        "bench peer did not report that it is listening on {ADDRESS}"
                    ))));
                }
            }
        }
    }
}

fn exited(status: std::process::ExitStatus) -> Error {
    Error::Io(io::Error::other(format!(
        "bench peer exited ({status}) before {ADDRESS} accepted a connection"
    )))
}

fn reports_listening(line: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(line) else {
        return false;
    };
    let mark = format!("listening on {ADDRESS}");
    let Some(at) = text.find(&mark) else {
        return false;
    };
    match text[at + mark.len()..].chars().next() {
        None => true,
        Some(c) if c.is_ascii_digit() => false,
        Some(_) => true,
    }
}

fn wait_until_listening(mut stderr: ChildStderr) -> Receiver<io::Result<()>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 256];
        let mut out = io::stderr();
        loop {
            match stderr.read(&mut tmp) {
                Ok(0) => {
                    let _ = out.write_all(&buf);
                    let _ = out.flush();
                    let _ = tx.send(Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        format!("bench peer closed stderr before it was listening on {ADDRESS}"),
                    )));
                    break;
                }
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    while let Some(i) = buf.iter().position(|b| *b == b'\n') {
                        let line: Vec<u8> = buf.drain(..=i).collect();
                        let _ = out.write_all(&line);
                        let _ = out.flush();
                        if reports_listening(&line) {
                            let _ = out.write_all(&buf);
                            let _ = out.flush();
                            let _ = tx.send(Ok(()));
                            let _ = io::copy(&mut stderr, &mut out);
                            return;
                        }
                    }
                }
                Err(err) => {
                    let _ = tx.send(Err(err));
                    break;
                }
            }
        }
    });
    rx
}

impl Drop for BenchPeer {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use super::*;

    #[test]
    fn bridge_args_are_name_obdlink_and_the_socket_is_127_0_0_1_35000() {
        assert_eq!(ADDRESS, "127.0.0.1:35000");
        let addr: SocketAddr = ADDRESS.parse().unwrap();
        assert!(addr.ip().is_loopback());
        assert_eq!(addr.port(), 35_000);
        assert_eq!(bridge_args(), ["--name", "OBDLink"]);
        let line = bridge_args().join(" ");
        let lower = line.to_ascii_lowercase();
        for word in [
            "com",
            "serial",
            "bluetooth",
            "ble",
            "tty",
            "j2534",
            "passthru",
            "listen",
            "mac",
            "service",
            "fff0",
            "radio",
        ] {
            assert!(!lower.contains(word), "{word} in {line}");
        }
    }

    #[test]
    fn the_program_defaults_to_dangle() {
        assert_eq!(program(None), OsString::from("dangle"));
        assert_eq!(
            program(Some(OsString::from(r"D:\bench-stub.exe"))),
            OsString::from(r"D:\bench-stub.exe")
        );
    }

    #[test]
    fn only_a_listening_line_for_this_socket_names_the_bench_peer() {
        assert!(reports_listening(b"dangle: listening on 127.0.0.1:35000\n"));
        assert!(reports_listening(b"listening on 127.0.0.1:35000\n"));
        assert!(!reports_listening(
            b"dangle: Only one usage of each socket address is normally permitted\n"
        ));
        assert!(!reports_listening(b"not bound\n"));
        assert!(!reports_listening(b"listening on 127.0.0.1:9\n"));
        assert!(!reports_listening(b"listening on 0.0.0.0:35000\n"));
        assert!(!reports_listening(b"listening on 127.0.0.1:350009\n"));
    }
}
