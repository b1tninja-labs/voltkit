//! Read client for the Voltage app's Wi-Fi session: ELM327 command lines over TCP.
//!
//! Each line is terminated by a carriage return. The session is the reset-and-format
//! sequence from the ELM327 datasheet, then the `$1A` identifiers and `$22` data
//! identifiers from the 2026-11-14 Voltage trial. `profile` reads the lab profile's
//! identifiers from the adapter address the operator passes. `gauges` reads six of those `$22`
//! identifiers and prints the scaled fields. SecurityAccess (`$27`) and write
//! services are refused before any byte is written. There is no Bluetooth path.

#![forbid(unsafe_code)]

mod capture;
mod elm;
mod gauges;
mod scan;
mod vin;

pub use capture::{capture_set, clone_profile, range_rows, Read, SETS};

pub use elm::{hex_spaced, payload, split_reply, Error, Line, Refuse, Reply, Session};
pub use gauges::{exchange_gauges, gauge_session_lines};
pub use scan::{
    lines_for, profile_bytes, profile_lines, profile_note, profile_table, scan_lines, scan_table,
    session_lines, ScanRow, INIT,
};
pub use vin::{us_volt, Shape, ShapeError, UsVolt};
