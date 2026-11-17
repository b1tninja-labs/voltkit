//! Carriage-return ELM327 session over TCP.
//!
//! The datasheet terminates a command with a carriage return and ends a reply at the
//! `>` prompt. Bytes of value 0 are ignored on the way in.

use std::io::{self, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

/// What a request line is allowed to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
    /// An `AT` adapter command.
    Adapter,
    /// A diagnostic read. The byte is the service identifier.
    Read(u8),
}

/// A line the session will not write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refuse {
    /// A blank line repeats the previous command.
    Empty,
    /// `$27` SecurityAccess.
    SecurityAccess,
    /// Anything that is not an `AT` command or a read in the allow set.
    NotARead,
}

impl std::fmt::Display for Refuse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refuse::Empty => write!(f, "empty line is not sent"),
            Refuse::SecurityAccess => write!(f, "security access is not sent"),
            Refuse::NotARead => write!(
                f,
                "only AT commands and services 01, 03, 1A, and 22 are sent"
            ),
        }
    }
}

/// Failure to connect or to complete one exchange.
#[derive(Debug)]
pub enum Error {
    /// The socket failed.
    Io(io::Error),
    /// The line was refused locally.
    Refuse(Refuse),
    /// The address was not a loopback address.
    NotLoopback,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Refuse(r) => write!(f, "{r}"),
            Error::NotLoopback => write!(f, "address is not loopback"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Error::Io(value)
    }
}

/// One reply, raw bytes through the prompt and the same bytes split into lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    /// Bytes received, including the prompt, with NUL bytes removed.
    pub raw: Vec<u8>,
    /// Lines with the carriage return, linefeed, and prompt removed.
    pub lines: Vec<String>,
}

/// A connected ELM327-over-TCP session.
#[derive(Debug)]
pub struct Session {
    stream: TcpStream,
}

impl Session {
    /// Connect to a loopback ELM327-over-TCP peer.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotLoopback`] when `addr` is not loopback, and [`Error::Io`]
    /// when the connection fails.
    pub fn connect(addr: &str) -> Result<Session, Error> {
        let sa = loopback(addr)?;
        let stream = TcpStream::connect_timeout(&sa, Duration::from_secs(3))?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        Ok(Session { stream })
    }

    /// How long a read waits for the next byte.
    ///
    /// # Errors
    ///
    /// Returns the socket error when the timeout cannot be set.
    pub fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.stream.set_read_timeout(timeout)
    }

    /// Write `line` and a carriage return, then read until the prompt.
    ///
    /// The line is judged before it is written. `$27` never reaches the socket.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Refuse`] for a line outside the read set, and [`Error::Io`]
    /// when the peer closes before the prompt.
    pub fn exchange(&mut self, line: &str) -> Result<Reply, Error> {
        classify(line).map_err(Error::Refuse)?;
        if line.bytes().any(|b| b == b'\r' || b == b'\n' || b == 0) {
            return Err(Error::Refuse(Refuse::NotARead));
        }
        let mut buf = Vec::with_capacity(line.len() + 1);
        buf.extend_from_slice(line.as_bytes());
        buf.push(b'\r');
        self.stream.write_all(&buf)?;
        let raw = read_prompt(&mut self.stream)?;
        let lines = split_reply(&raw);
        Ok(Reply { raw, lines })
    }
}

fn loopback(addr: &str) -> Result<SocketAddr, Error> {
    let mut iter = addr.to_socket_addrs()?;
    match iter.next() {
        Some(sa) if is_loopback(sa.ip()) && iter.next().is_none() => Ok(sa),
        Some(_) => Err(Error::NotLoopback),
        None => Err(Error::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "no address",
        ))),
    }
}

fn is_loopback(ip: IpAddr) -> bool {
    ip.is_loopback()
}

fn read_prompt(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = [0u8; 1024];
    loop {
        let n = stream.read(&mut buf)?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "connection closed before the prompt",
            ));
        }
        for &b in &buf[..n] {
            if b == 0 {
                continue;
            }
            out.push(b);
            if b == b'>' {
                return Ok(out);
            }
        }
    }
}

/// Classify a request without sending it.
///
/// Adapter lines are those whose first two non-space characters are `AT`.
/// A hexadecimal line is a read when its service identifier is `01`, `03`, `1A`, or `22`.
/// A trailing odd digit is the ELM response-count nibble and is not part of the service.
pub fn classify(line: &str) -> Result<Line, Refuse> {
    let compact: String = line.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    if compact.is_empty() {
        return Err(Refuse::Empty);
    }
    if compact.len() >= 2 && compact.as_bytes()[..2].eq_ignore_ascii_case(b"AT") {
        return Ok(Line::Adapter);
    }
    if !compact.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Refuse::NotARead);
    }
    let body = if !compact.len().is_multiple_of(2) {
        &compact[..compact.len() - 1]
    } else {
        compact.as_str()
    };
    if body.len() < 2 {
        return Err(Refuse::NotARead);
    }
    let Ok(service) = u8::from_str_radix(&body[..2], 16) else {
        return Err(Refuse::NotARead);
    };
    match service {
        0x01 | 0x03 | 0x1A | 0x22 => Ok(Line::Read(service)),
        0x27 => Err(Refuse::SecurityAccess),
        _ => Err(Refuse::NotARead),
    }
}

/// Split a reply on carriage return and linefeed, dropping blanks and the prompt.
#[must_use]
pub fn split_reply(raw: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(raw);
    text.split(['\r', '\n'])
        .map(|l| l.trim().trim_end_matches('>').trim())
        .filter(|l| !l.is_empty())
        .map(ToString::to_string)
        .collect()
}

/// Diagnostic payload from a reply, when the lines are one hex frame or a CAF multi-frame.
///
/// A leading echo of `request` is ignored. `None` is a status line (`OK`, `NO DATA`, a banner).
pub fn payload(request: &str, lines: &[String]) -> Option<Vec<u8>> {
    let echo = squash(request);
    let lines: Vec<&str> = lines
        .iter()
        .map(String::as_str)
        .filter(|l| squash(l) != echo)
        .collect();
    if lines.is_empty() {
        return None;
    }
    if lines[0].len() == 3 && lines[0].chars().all(|c| c.is_ascii_hexdigit()) && lines.len() > 1 {
        return assemble(&lines);
    }
    if lines.len() == 1 {
        return hex_bytes(lines[0]);
    }
    None
}

fn assemble(lines: &[&str]) -> Option<Vec<u8>> {
    let expect = usize::from_str_radix(lines[0], 16).ok()?;
    let mut data = Vec::with_capacity(expect);
    for (i, line) in lines[1..].iter().enumerate() {
        let (idx, rest) = line.split_once(':')?;
        if idx.len() != 1 {
            return None;
        }
        let n = usize::from(u8::from_str_radix(idx, 16).ok()?);
        if n != i % 16 {
            return None;
        }
        data.extend(hex_bytes(rest)?);
    }
    if data.len() == expect {
        Some(data)
    } else {
        None
    }
}

fn hex_bytes(text: &str) -> Option<Vec<u8>> {
    let hex: String = text.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    if hex.len() < 2 || !hex.len().is_multiple_of(2) || !hex.chars().all(|c| c.is_ascii_hexdigit())
    {
        return None;
    }
    let bytes = hex.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let mut i = 0;
    while i < bytes.len() {
        let h = std::str::from_utf8(&bytes[i..i + 2]).ok()?;
        out.push(u8::from_str_radix(h, 16).ok()?);
        i += 2;
    }
    Some(out)
}

fn squash(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect::<String>()
        .to_ascii_uppercase()
}

/// Hex bytes with a space between them.
#[must_use]
pub fn hex_spaced(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn security_access_and_writes_are_refused_before_any_socket() {
        assert_eq!(classify("2701"), Err(Refuse::SecurityAccess));
        assert_eq!(classify("27 01"), Err(Refuse::SecurityAccess));
        assert_eq!(classify("2E1234"), Err(Refuse::NotARead));
        assert_eq!(classify("3101"), Err(Refuse::NotARead));
        assert_eq!(classify(""), Err(Refuse::Empty));
        assert_eq!(classify("ATZ"), Ok(Line::Adapter));
        assert_eq!(classify("at sh 7E0"), Ok(Line::Adapter));
        assert_eq!(classify("0100"), Ok(Line::Read(0x01)));
        assert_eq!(classify("03"), Ok(Line::Read(0x03)));
        assert_eq!(classify("1A90"), Ok(Line::Read(0x1A)));
        assert_eq!(classify("22002F"), Ok(Line::Read(0x22)));
    }

    #[test]
    fn payload_reads_a_single_frame_and_a_length_prefixed_reply() {
        let single = vec!["62 00 2F 6C".to_string()];
        assert_eq!(
            payload("22002F", &single),
            Some(vec![0x62, 0x00, 0x2F, 0x6C])
        );
        let multi = vec![
            "22002F".to_string(),
            "008".to_string(),
            "0:010203040506".to_string(),
            "1:0708".to_string(),
        ];
        let got = payload("22002F", &multi).unwrap();
        assert_eq!(got, vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(payload("ATZ", &["OK".to_string()]), None);
        assert_eq!(payload("1A90", &["NO DATA".to_string()]), None);
    }
}
