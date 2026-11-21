# voltkit

The user interface for one car. It speaks ELM327 command lines over TCP, each line ended by a
carriage return, and stops at the `>` prompt. The peer on the other end of that socket is
chosen here. This crate does not link the other tools, and it does not speak Bluetooth, SPS
HTTP, or the VCX frame port itself.

| Peer | What you run | What voltkit does today |
| --- | --- | --- |
| Lab | `fakeobd serve` on `127.0.0.1:35000`, with `profiles/volt-lab.profile` or a clone this tool wrote | `voltkit lab` starts `fakeobd serve --listen 127.0.0.1:35000` and dials it. The command does not pass `--profile`, a serial port, or a Bluetooth flag. `profile` and `wizard` still dial an address you pass. `--clone` prints a profile `fakeobd` can load. |
| Bench | `obd-bridge --name OBDLink` on Windows. It republishes a BLE ELM327, including an OBDLink MX that advertises the FFF0 serial service, as ELM327-over-TCP on `127.0.0.1:35000` | Dial that same address. voltkit does not open the radio. |
| SPS | `spskit` builds a VIT1 programming-data body from a saved module read. It does not post the body and it does not program the car | The programming job asks `parts`, `sps-extra`, `hardware`, and `identity`. Nothing here calls `spskit`, and there is no `fakesps` peer yet. |
| Dongle | `vcxtoolkit` packs a VCX serial frame and, on Windows, can open the USB serial port | Not wired. A registered J2534 DLL is not built in `vcxtoolkit` or here. The direct frame library is the path that exists. The fifteen `PassThru*` exports stay unbuilt until SAE J2534 is imported into primarysource. |

`gattmirror` is the Android-emulator HCI mirror. It is not this TCP peer.

`$27` is refused before it is written. The only diagnostic services the socket will carry are
`01`, `03`, `1A`, and `22`. A capture from a car is gitignored (`*.profile`, `profiles/`,
`captures/`, and a new file under `scans/`). The two tracked logs are the fakeobd loopback
capture.

The reset-and-format sequence is the one the 2026-11-14 trial sent, and each command is an
ELM327 datasheet command: `ATZ`, `ATL0`, `ATH0`, `ATS0`, `ATSP6`, `ATE0`, `ATCAF1`. `ATSP6` is
ELM protocol 6, ISO 15765-4 CAN with an 11-bit identifier at 500 kbaud. That protocol number
is not a statement about ISO 15765-2.

The scan is the trial's `$1A` identifiers (`90`, `DF`, `B0`, `99`, `CC`, `6F`, `6D`), mode `03`
on `7DF` with headers on, and the 29 `$22` data identifiers, each with the header that trial used.
Header `241` is the only one
the trial paired with a receive address (`ATCRA641`). `2889` is also asked at `621`, which has
no recorded response id. The raw single-wire frame from that trial is not sent.

A 17-character check against the Chevrolet Volt and Bolt EV pages of GM's 2016–2019 NHTSA
filings lives in `vin`. Those pages are `vin:gm-vin-2016` through `vin:gm-vin-2019` in
primarysource. The 2016 filing has a Volt page and no Bolt page. The check-digit test vector
is the worked example printed in those filings.

## Run

```bash
cargo run -- 127.0.0.1:35000
```

Point it at a loopback ELM327-over-TCP peer. Against fakeobd, that is `serve` with
`profiles/volt-lab.profile`. The dialogue is printed to stdout: `>` request, `<` reply line,
`=` diagnostic bytes when the reply is a hex payload.

```bash
cargo run -- lab
```

`voltkit lab` starts `fakeobd serve --listen 127.0.0.1:35000` and prints that same dialogue.
Set `VOLTKIT_LAB_PEER` to the binary when it is not `fakeobd` on `PATH`. The process is stopped
when the command exits. It does not open a serial port or a Bluetooth radio. `$27` is still
refused before it is written.

`voltkit profile` reads the identifiers the lab profile answers, plus the charger
and climate reads traced after it. Point it at the ELM adapter on the vehicle:
a TCP address, or on Windows a COM port at 115200 baud. A positive reply prints
a `did` or `lid` line whose hex is the data after the service and the identifier.
`$27` is still refused. The VCX frame port is not this protocol. The sets, the
clone file, the programming-read fields, and the diagrams of those three are in
[docs/capture.md](docs/capture.md).

`--set` dumps one group. `--range` walks a span of at most 256 identifiers on
one header. `--clone` prints a lab profile on stdout (the dialogue stays on
stderr) that `fakeobd` can load. `parts` is `$1A` `C0` through `CA`; a positive
line comments the decimal of the first four data bytes, which is the part-number
slice. There is no separate `fakevolt` or `obdkit` profile format: the clone is
the lab profile, and this client is the reader.

```bash
cargo run -- wizard
cargo run -- profile --list
cargo run -- profile 192.168.0.10:35000 --set charger --set climate --clone
cargo run -- profile COM5 --set parts --clone
cargo run -- profile COM5 --set cells --clone
```

`voltkit wizard` asks which capture to run, then the adapter address, then shows the `profile` command it will use. Nothing is sent until the answer is yes. In a terminal this is a full-screen UI. A pipe keeps the line prompts. The jobs are the lab profile, the programming read (`parts`, `sps-extra`, `hardware`, and `identity`), the nine-header presence probe, one screen, or a range. A profile can be printed here or written to a path outside this repository. When the screen captured it, the profile is printed after you quit.

The gauge command sends six of those `$22` reads and prints the scaled fields. It dials
only when `--host` is present, and only when that address is `127.0.0.1`.

```bash
cargo run -- gauges --host 127.0.0.1:35000
```

`22002F` uses the scan table's header `7E0`. `228334`, `2243AF`, `2241A3`, `2241A6`, and
`22434F` use header `7E4`. The printout is the first data byte of `002F` divided by 255,
the first data byte of `8334` times 100 divided by 255, the big-endian uint16 of `43AF`
times 100 divided by 65535, the uint16 of `41A3` divided by 10, the uint16 of `41A6`
divided by 64 in kilometres, and the first data byte of `434F` minus 40 in Celsius.

```bash
cargo test
```

The tests use an in-process TCP peer, or a stub binary `voltkit lab` launches. They do not open a
serial port, a Bluetooth device, or a non-loopback address, and they do not need a vehicle.

## Release

`Cargo.toml` sets `publish = false`. This crate is not published to crates.io.

Pushing a tag such as `v0.1.0` runs `.github/workflows/release.yml`. The tag matches `v*.*.*`. The workflow runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` on Ubuntu and Windows. It then attaches two binaries to the GitHub Release for that tag:

- `voltkit-x86_64-pc-windows-msvc.exe`, linked with the static CRT, so the machine does not need the Visual C++ redistributable
- `voltkit-x86_64-unknown-linux-gnu`

`SHA256SUMS` is attached beside them. A pull request, and any push, runs the same three checks in `.github/workflows/ci.yml` and does not publish a release. The repository has no `main` branch. The default branch is the one GitHub names.

License: MIT OR Apache-2.0.
