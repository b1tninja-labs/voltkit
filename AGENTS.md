# Scope

`voltkit` is a read client. It talks ELM327-over-TCP to a loopback peer.

- Do not send `$27`, a seed, or a key. `classify` refuses service `27` before the socket write.
- Do not add a write service (`$2E`, `$2F`, `$31`, `$11`, `$14`, `$34`–`$37`, `$85`).
- Do not add a VIN literal. The check-digit test uses the worked example in the GM filings
  `gm-vin-2016` through `gm-vin-2019`, read through `primarysource vin`. The Volt card is on
  every filing. The Bolt EV card starts in 2017. Do not copy a vehicle number into the tree.
- Do not add Bluetooth. A BLE OBDLink, including an MX that advertises the FFF0 serial service, is `obd-bridge` in front of the same TCP socket. The session and gauge commands stay on loopback and do not open a serial port. `voltkit lab` starts `fakeobd serve --listen 127.0.0.1:35000` and dials that address. `VOLTKIT_LAB_PEER` names the binary when it is not `fakeobd` on `PATH`. That command does not open a serial port or a Bluetooth radio. `voltkit bench` starts `obd-bridge --name OBDLink` and dials that same address. The obd-bridge README default for `--listen` is `127.0.0.1:35000`, so the command does not pass a listen flag. `VOLTKIT_BENCH_PEER` names the binary when it is not `obd-bridge` on `PATH`. That command does not open a serial port, a Bluetooth radio, or a vehicle. `voltkit profile` is the capture: it dials the TCP address the operator passes, and on Windows it opens the COM port they pass. `--set` and `--range` choose which reads, and the range span is at most 256. `voltkit wizard` asks which of those captures to run and then runs the same reads. In a terminal that question is a full-screen UI; a pipe keeps the line prompts. It still sends only reads.
- Do not take a crate dependency on `fakeobd`, `obd-bridge`, `spskit`, or `vcxtoolkit`. `voltkit lab` spawns the `fakeobd` binary and does not link it. `voltkit bench` spawns the `obd-bridge` binary and does not link it. The handoff otherwise is a TCP address, a profile file, or a saved module read. Do not add a `PassThru*` export or a `PassThruSupport.04.04` writer. SAE J2534 is not in primarysource.
- `ATSP6` means ELM protocol 6 (ISO 15765-4 CAN, 11-bit, 500 kbaud). Do not describe it as
  ISO 15765-2.
- Scan headers come from the 2026-11-14 trial. Do not invent a response id for `621`.

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass
before a change is done. `unsafe` is forbidden. No dependencies without a reason in the
commit message. `publish = false` stays. A release is a GitHub Release from a `v*.*.*` tag.
It is not a crates.io publish.
