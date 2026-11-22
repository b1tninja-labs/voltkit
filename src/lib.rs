//! Read client for the Voltage app's Wi-Fi session: ELM327 command lines over TCP.
//!
//! Each line is terminated by a carriage return. The session is the reset-and-format
//! sequence from the ELM327 datasheet, then the `$1A` identifiers, mode `03` on `7DF`,
//! and the `$22` data identifiers from the 2026-11-14 Voltage trial. `profile` reads the lab profile's
//! identifiers from the adapter address the operator passes. `wizard` detects the adapters present
//! (`detect`), lets the operator select one instead of typing a COM port, asks which capture to run,
//! then runs it and auto-writes the profile. `lab` starts `fakeobd serve` on `127.0.0.1:35000` and
//! dials that address (or, with the `lab` feature, serves that bench in-process). `bench` starts
//! `obd-bridge --name OBDLink` and dials that same address; the OBDLink MX is reached only through
//! that bridge, so voltkit never opens the radio itself. `gauges` reads six of those `$22`
//! identifiers and prints the scaled fields. SecurityAccess (`$27`) and write
//! services are refused before any byte is written.

#![forbid(unsafe_code)]

pub mod bench;
mod capture;
pub mod detect;
mod elm;
mod gauges;
pub mod lab;
mod scan;
mod vin;
pub mod wizard;

pub use capture::{capture_set, clone_profile, range_rows, select_rows, Read, SETS};
pub use detect::{devices, Device, Link};

pub use elm::{hex_spaced, payload, split_reply, Error, Line, Refuse, Reply, Session};
pub use gauges::{exchange_gauges, gauge_session_lines};
pub use scan::{
    lines_for, profile_bytes, profile_lines, profile_note, profile_table, scan_lines, scan_table,
    session_lines, ScanRow, INIT,
};
pub use vin::{identify, us_bolt, us_volt, BoltSeries, Model, Shape, ShapeError, UsBolt, UsVolt};
