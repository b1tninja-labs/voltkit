//! Holds `127.0.0.1:35000` across the lab and bench integration tests.
//!
//! Those binaries run in parallel and both need that socket.

use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

/// A directory lock for `127.0.0.1:35000`. Drop it when the socket is free.
#[must_use = "dropping this releases 127.0.0.1:35000"]
pub struct PortHold {
    dir: PathBuf,
}

/// Wait until this process holds the lock for `127.0.0.1:35000`.
pub fn hold_35000() -> PortHold {
    let dir = std::env::temp_dir().join("voltkit-127-0-0-1-35000.lock");
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        if let Ok(meta) = fs::metadata(dir.join("stamp")) {
            if meta
                .modified()
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age > Duration::from_secs(180))
            {
                let _ = fs::remove_dir_all(&dir);
            }
        }
        match fs::create_dir(&dir) {
            Ok(()) => {
                let _ = fs::write(dir.join("stamp"), b"");
                return PortHold { dir };
            }
            Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
            Err(err) => panic!("could not lock 127.0.0.1:35000 ({err})"),
        }
    }
}

impl Drop for PortHold {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}
