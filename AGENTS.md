# Scope

`voltkit` is a read client. It talks ELM327-over-TCP to a loopback peer.

- Do not send `$27`, a seed, or a key. `classify` refuses service `27` before the socket write.
- Do not add a write service (`$2E`, `$2F`, `$31`, `$11`, `$14`, `$34`–`$37`, `$85`).
- Do not add a VIN literal. The check-digit test uses the worked example in the GM filings
  `gm-vin-2016` through `gm-vin-2019`. Do not copy a vehicle number into the tree.
- Do not add Bluetooth. The session and gauge commands stay on loopback and do not open a serial port. `voltkit profile` is the capture: it dials the TCP address the operator passes, and on Windows it opens the COM port they pass. `--set` and `--range` choose which reads, and the range span is at most 256. `voltkit wizard` asks which of those captures to run and then runs the same reads. In a terminal that question is a full-screen UI; a pipe keeps the line prompts. It still sends only reads.
- `ATSP6` means ELM protocol 6 (ISO 15765-4 CAN, 11-bit, 500 kbaud). Do not describe it as
  ISO 15765-2.
- Scan headers come from the 2026-11-14 trial. Do not invent a response id for `621`.

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass
before a change is done. `unsafe` is forbidden. No dependencies without a reason in the
commit message. `publish = false` stays. A release is a GitHub Release from a `v*.*.*` tag.
It is not a crates.io publish.
