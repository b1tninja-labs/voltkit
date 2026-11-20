# voltkit

A read client for the Voltage app's Wi-Fi session. It speaks ELM327 command lines over TCP,
each line ended by a carriage return, and stops at the `>` prompt.

The reset-and-format sequence is the one the 2026-11-14 trial sent, and each command is an
ELM327 datasheet command: `ATZ`, `ATL0`, `ATH0`, `ATS0`, `ATSP6`, `ATE0`, `ATCAF1`. `ATSP6` is
ELM protocol 6, ISO 15765-4 CAN with an 11-bit identifier at 500 kbaud. That protocol number
is not a statement about ISO 15765-2.

The scan is the trial's `$1A` identifiers (`90`, `DF`, `B0`, `99`, `CC`, `6F`, `6D`) and the
29 `$22` data identifiers, each with the header that trial used. Header `241` is the only one
the trial paired with a receive address (`ATCRA641`). `2889` is also asked at `621`, which has
no recorded response id. The raw single-wire frame from that trial is not sent.

`$27` is refused before it is written. The only diagnostic services the socket will carry are
`01`, `03`, `1A`, and `22`. There is no Bluetooth path and no write service.

A 17-character check against the Chevrolet Volt pages of GM's 2016–2019 NHTSA filings lives in
`vin`. The check-digit test vector is the worked example printed in those filings.

## Run

```bash
cargo run -- 127.0.0.1:35000
```

Point it at a loopback ELM327-over-TCP peer. Against fakeobd, that is `serve` with
`profiles/volt-lab.profile`. The dialogue is printed to stdout: `>` request, `<` reply line,
`=` diagnostic bytes when the reply is a hex payload.

`voltkit profile` reads the identifiers the lab profile answers, plus the charger
and climate reads traced after it. Point it at the ELM adapter on the vehicle:
a TCP address, or on Windows a COM port at 115200 baud. It prints each exchange
and, for a positive reply, a `did` or `lid` line whose hex is the data after the
service and the identifier. `$27` is still refused. The VCX frame port is not
this protocol.

```bash
cargo run -- profile 192.168.0.10:35000
cargo run -- profile COM5 --baud 115200
```

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

The tests use an in-process TCP peer. They do not open a serial port, a Bluetooth device, or a
non-loopback address.

License: MIT OR Apache-2.0.
