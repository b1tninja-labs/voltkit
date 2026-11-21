//! Launch the lab peer and dial it.
//!
//! The peer is the `fakeobd serve` process on [`ADDRESS`]. This module spawns
//! that binary and opens a [`Session`] to it. `fakeobd` is not a crate
//! dependency. The command line is `serve --listen 127.0.0.1:35000`: no serial
//! port, no Bluetooth server, and no profile path. Dial waits until that
//! process writes a line to stderr, which `fakeobd serve` does after the
//! socket is bound, and then connects. The session still refuses `$27` and
//! the write services before a byte is written.

use std::ffi::{OsStr, OsString};
use std::io::{self, Read, Write};
use std::process::{Child, ChildStderr, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crate::{Error, Session};

/// The loopback socket `fakeobd serve` listens on for the lab peer.
pub const ADDRESS: &str = "127.0.0.1:35000";

/// Environment variable that names the `fakeobd` binary.
///
/// When it is unset, the command is `fakeobd` on `PATH`.
pub const PROGRAM_ENV: &str = "VOLTKIT_LAB_PEER";

/// `program` when the operator passed one, otherwise `fakeobd`.
#[must_use]
pub fn program(override_path: Option<OsString>) -> OsString {
    override_path.unwrap_or_else(|| OsString::from("fakeobd"))
}

/// Arguments that start the lab peer on [`ADDRESS`].
#[must_use]
pub fn serve_args() -> [&'static str; 3] {
    ["serve", "--listen", ADDRESS]
}

/// A running `fakeobd serve` process.
///
/// Dropping this value stops the process.
#[derive(Debug)]
#[must_use = "the lab peer is stopped when this value is dropped"]
pub struct LabPeer {
    child: Child,
}

impl LabPeer {
    /// Spawn `program` with [`serve_args`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the process cannot be started.
    pub fn start(program: impl AsRef<OsStr>) -> Result<Self, Error> {
        let program = program.as_ref();
        let child = Command::new(program)
            .args(serve_args())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| {
                Error::Io(io::Error::other(format!(
                    "could not start lab peer {}: {err}",
                    program.to_string_lossy()
                )))
            })?;
        Ok(Self { child })
    }

    /// Dial [`ADDRESS`] after the peer writes a status line to stderr.
    ///
    /// `fakeobd serve` prints that line after the listen socket exists. A
    /// connection is not opened before then, so some other process already
    /// bound to the port is not treated as this peer.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the peer exits, closes stderr without a
    /// line, or does not accept [`ADDRESS`] once it has reported that it is
    /// listening.
    pub fn dial(&mut self) -> Result<Session, Error> {
        let stderr = self
            .child
            .stderr
            .take()
            .ok_or_else(|| Error::Io(io::Error::other("lab peer stderr was already taken")))?;
        let ready = wait_until_listening(stderr);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait()? {
                return Err(exited(status));
            }
            match ready.recv_timeout(Duration::from_millis(50)) {
                Ok(Ok(())) => break,
                Ok(Err(err)) => return Err(Error::Io(err)),
                Err(mpsc::RecvTimeoutError::Timeout) if Instant::now() < deadline => continue,
                Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(Error::Io(io::Error::other(format!(
                        "lab peer did not report that it is listening on {ADDRESS}"
                    ))));
                }
            }
        }
        if let Some(status) = self.child.try_wait()? {
            return Err(exited(status));
        }
        let session = Session::connect(ADDRESS)?;
        if let Some(status) = self.child.try_wait()? {
            return Err(exited(status));
        }
        Ok(session)
    }
}

fn exited(status: std::process::ExitStatus) -> Error {
    Error::Io(io::Error::other(format!(
        "lab peer exited ({status}) before {ADDRESS} accepted a connection"
    )))
}

fn wait_until_listening(mut stderr: ChildStderr) -> Receiver<io::Result<()>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 256];
        loop {
            match stderr.read(&mut tmp) {
                Ok(0) => {
                    let _ = tx.send(Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "lab peer closed stderr before it was listening",
                    )));
                    break;
                }
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    if let Some(i) = buf.iter().position(|b| *b == b'\n') {
                        let line: Vec<u8> = buf.drain(..=i).collect();
                        let mut out = io::stderr();
                        let _ = out.write_all(&line);
                        let _ = out.write_all(&buf);
                        let _ = tx.send(Ok(()));
                        let _ = io::copy(&mut stderr, &mut out);
                        break;
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

impl Drop for LabPeer {
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
    fn serve_listens_on_127_0_0_1_35000() {
        assert_eq!(ADDRESS, "127.0.0.1:35000");
        let addr: SocketAddr = ADDRESS.parse().unwrap();
        assert!(addr.ip().is_loopback());
        assert_eq!(addr.port(), 35_000);
        assert_eq!(serve_args(), ["serve", "--listen", ADDRESS]);
        let line = serve_args().join(" ");
        let lower = line.to_ascii_lowercase();
        for word in [
            "com",
            "serial",
            "bluetooth",
            "ble",
            "tty",
            "j2534",
            "passthru",
        ] {
            assert!(!lower.contains(word), "{word} in {line}");
        }
    }

    #[test]
    fn the_program_defaults_to_fakeobd() {
        assert_eq!(program(None), OsString::from("fakeobd"));
        assert_eq!(
            program(Some(OsString::from(r"D:\lab-stub.exe"))),
            OsString::from(r"D:\lab-stub.exe")
        );
    }
}
