# Capturing a car for the lab and for a programming read

`voltkit profile` speaks ELM327. It is the reader for the lab profile that `fakeobd` serves. The VCX frame port is a different tool: `vcx-probe` asks the dongle who it is, and the pass-through reply body is not recovered, so this command does not send vehicle bytes on that port.

The session and gauge commands stay on loopback. `profile` dials the TCP address the operator passes, or on Windows opens the COM port they pass, at 115200 unless `--baud` says otherwise. `$27` is refused before it is written. Write services are refused. The socket will carry services `01`, `03`, `1A`, and `22`. The capture sends `$1A`, `$22`, and one mode `03` on `7DF`.

A device id in the programming client is not a CAN header. K17's device id is 40, and Voltage module 40 is header `241`, but those two uses of 40 are different address maps. No file pairs an SPS device id with an 11-bit request header. `spsvcs.dll` has no static device-to-header rows. A header for one of those ids would come from a live `$1A` `$B0` response, which this command does not interpret.

```mermaid
flowchart TD
  adapter["ELM327 adapter"]
  all["profile --set all --clone"]
  lab["fakeobd profile"]
  parts["profile --set parts --set sps-extra"]
  saved["saved read for each controller the catalog names"]
  named["getProgrammingData names the blobs"]
  adapter --> all --> lab
  adapter --> parts --> saved --> named
```

## Commands

```text
voltkit profile --list
voltkit profile HOST:PORT [--set NAME]... [--range HEADER 22|1A START END]... [--clone]
voltkit profile COMn [--baud 115200] [--set NAME]... [--range HEADER 22|1A START END]... [--clone]
```

`--list` prints the set names and does not open a socket. With no `--set` and no `--range`, the command runs `all`. `--clone` writes the lab profile on stdout and the dialogue on stderr. Redirect stdout to a file outside the repository. A capture from a car contains that car's module bytes.

`--range` walks one 11-bit header. The service is `22` or `1A`. The span is at most 256 identifiers. `$1A` identifiers are one byte. Service `$27` is refused before a socket is opened. Headers `241`, `244`, and `24C` set the response filters the lab profile already records (`641`, `644`, `64C`). Any other header, including `621`, leaves the filter unset. That filter rule is for `--range`. The `presence` set sets the appendix response ids on its own headers.

## Sets

| Set | What it asks |
| --- | --- |
| `all` | The lab profile capture, including mode `03` on `7DF`. It does not include `parts`, `presence`, or `sps-extra`. |
| `scan` | The 2026-11-14 trial only, including mode `03` on `7DF`. |
| `hpcm` | Header `7E0`. |
| `powertrain` | Header `7E1`. |
| `becm` | Header `7E4`. |
| `charger` | The charger screen on `7E4`, including `$1A` `A4` and `75`. |
| `cells` | `7E7` identifiers `4181` through `4250`, plus `82A3` and `40D4`. |
| `climate` | `801F`, `82B2`, `817B`, `41B4`, `41B2`, `82B5`, `451E` on `241`. |
| `body` | The other `241` reads. |
| `cluster` | Header `244`. |
| `identity` | Every `$1A` read already in the lab capture. |
| `parts` | `$1A` `C0` through `CA` on `7E0`, `7E1`, `7E4`, `7E7`, and `241`. |
| `presence` | `$1A` `90` on `242`, `24B`, `251`, `255`, `257`, `258`, `7E2`, `7E5`, and `7E6`. |
| `sps-extra` | `$1A` `40` through `4A`, `60`, `62`, `64`, `66` through `6A`, `71`, `73`, `74`, and `76` on the `parts` headers. |

`cells` follows the lab table through `4240` and then the Voltage catalog tail `4241` through `4250`. That catalog calls `4202` through `4250` battery cell voltages and config, and it does not name a header. `7E7` is the cell ECU the lab profile already uses for `4181` through `4240`.

`presence` asks `$1A` `90` once on each of those headers. `242` receives `642`, `24B` receives `64B`, `251` receives `651`, `255` receives `655`, `257` receives `657`, and `258` receives `658`. `7E2`, `7E5`, and `7E6` set no `ATCRA`. The appendix says those physical headers use the default response eight above the request. `100`, `101`, `247`, `252`, `621`, and `7DF` are not in this set. Heap tokens `ATSH2410`, `ATSH7E00`, `ATSH7E40`, and `ATCRA6410` are not headers.

`sps-extra` uses the same five headers as `parts`: `7E0`, `7E1`, `7E4`, and `7E7` with no receive address, and `241` receiving `641`. Each listed identifier is asked once per header. `C0` stays in `parts`. `90`, `CB`, and `CC` are not in `sps-extra`.

## The clone

A positive `$22` reply becomes `did IIII HEX`, using the bytes after `62` and the identifier. A positive `$1A` reply becomes `lid II HEX`, using the bytes after `5A` and the identifier. A negative reply is counted and left out. The same identifier on the same header is kept once.

The file starts with `s3 5000`. Each answered header that has a recorded response id becomes one `ecu` line. The names, response ids, functional flag, and latency are the lab profile's values, so a clone sits on the same bus shape. The order is `7E0`, `7E4`, `244`, `241`, `24C`, `7E1`, `7E7`. Mode `01` and freeze frames are not in the capture. Mode `03` on `7DF` is. A positive reply starts with `43`, and the clone comments the raw hex. The file has no `dtc` line. A negative reply is left out.

A header with no recorded response id is a comment. `621` is that case for a `did` or `lid` line, and mode `03` on `7DF` is that case for the raw hex. The comment is that line, then a line that says the header has no recorded response id.

For `$1A` `C0` through `CA`, when at least four data bytes came back, the line comments the decimal of those four bytes. That is the part-number slice: the identifier echo is not included, and the next four bytes are an unsigned integer, first byte most significant. In `spskit`, `format0_decimal` takes the same four bytes from a longer buffer whose byte 2 is the service and whose start byte 1 is response byte 4. A `$7F` in that buffer writes no number. A short payload is not a part number.

`fakeobd serve --profile FILE` parses this file. A sample clone was loaded on `127.0.0.1:35991` and read back: `224532` returned `did 4532 0001`, `1AC1` returned `lid C1` with decimal 42, and `224250` returned `did 4250 BBE0`. That server was stopped. The sample used synthetic bytes, not a car.

There is no second profile format for a project named fakevolt or obdkit. The clone is the lab profile, and this client is the reader.

## Voltage emulation

`voltkit profile COMn --set all --clone` is the emulation pass. It covers the 2026-11-14 trial and the lab profile: powertrain on `7E1`, HPCM2 on `7E0`, BECM and the charger screen on `7E4`, climate and the other body reads on `241`, the cluster on `244`, the `$1A` placeholders on `24C`, the cell table on `7E7`, and mode `03` on `7DF`. The command also has `presence` and `sps-extra`. Those two sets stay outside `all`, as `parts` does.

These reads are still outside that pass:

- The trial's module-scan row names mode `03` on `7E0`, `7E1`, `7E4`, and `7E7` in one shared phase, without a separate filter for each. Those four stay outside this pass. The hub audit counted one allowed `03` and did not keep the request bytes.
- No trial log records an OBD mode `01` PID. The request atlas says the app sends no mode `01` request. The hub audit counted two allowed lines whose service byte was `01`, and those request bytes were not kept. `01FFFFFFFF000000` on `621` is a single-wire wake, not a PID. The command does not send a mode `01` PID.
- Of the 115 catalog `$22` identifiers, 113 are already asked by `profile_table` or by `volt-lab.profile`. The two with no header in either file are `5247` and `5248`. All 70 catalog `42xx` identifiers sit inside the `7E7` sweep `4181` through `4250`.
- The Voltage appendix pairs these request headers with response ids: `241`/`641`, `242`/`642`, `244`/`644`, `24B`/`64B`, `24C`/`64C`, `251`/`651`, `255`/`655`, `257`/`657`, `258`/`658`. Physical headers `7E0` through `7E7`, skipping `7E3`, use the default response eight above the request, and the appendix names no separate `ATCRA` for them. Headers with no paired response id are `100`, `101`, `247`, `252`, `621`, and `7DF`. `247` and `252` have filters `547` and `552` only. Heap strings `ATSH2410`, `ATSH7E00`, `ATSH7E40`, and `ATCRA6410` are a real header with the next digit glued on. They are not extra addresses.
- This command sets response filters for `241`/`641`, `244`/`644`, and `24C`/`64C`, and `presence` adds `242`/`642`, `24B`/`64B`, `251`/`651`, `255`/`655`, `257`/`657`, and `258`/`658`. It asks `7E0`, `7E1`, `7E2`, `7E4`, `7E5`, `7E6`, `7E7`, `7DF`, and `621`. `7E2`, `7E5`, and `7E6` leave the filter unset. It does not ask `100`, `101`, `247`, or `252`.
- The single-wire wake frames from the trial are not module data, and they are not sent.

Service `$27` and the write services stay out. The simulator refuses them. The app continued after that refusal, so emulation of that moment is the refusal, not a captured seed.

The checked-in `fakeobd` profile is on another branch and was not edited from here. It still lacks the seven climate reads on `241` and the charger identifiers `4532`, `4530`, `452F`, `A4`, and `75`. A clone from the car is what fills those. `all` already asks for them.

## Programming read

The programming client is `spskit`. `vit1.py` turns a saved module read into the VIT1 block on `getProgrammingData`. This crate does not post that request, and it does not download a calibration. A posted K17 request with no VIT1 came back `VIT_NO_VALID_CMB`. The catalog's suggested part numbers are not the module read. The calibration file is named from the module's own part numbers.

The traced catalog names 25 controller codes. Method 128, the block-same-cal list, is 23 rows and 21 codes, and that list includes K16. The short Normal list, method 1, is K114B, K124, K36, and K74. One missing part identifier and the builder will not attach the read.

Keep the raw `$1A` payload. The decimal comment is the part slice, and the hex is what a later step needs when the hardware field is a string rather than a decimal. A negative reply is not a zero.

| Controller | Method | Part identifiers | Hardware | Also |
| --- | --- | --- | --- | --- |
| K17 | 11228 | `C1`–`CA` | `CC`, the string from the read | `$90` |
| K33 | 11231 | `C1`–`CA` | `CC`, four-byte decimal | `$90` |
| K20 | 11229 | `C1`–`CA` | `CC`, decimal | `$90`, and the CVN flag |
| K9 | 410 | `C1`–`CA`, then `41`–`4A` | `CC`, decimal | `$90` |
| A22 | 446 | `C1` | `CB`, decimal | |
| A11 | 432 | `C1`, `C3`–`CA`, `73` | `CB`, decimal | |
| B218R | 11243 | `C1`–`CA` | `CB`, decimal | |
| B218L | 11239 | `C1`–`CA` | `CB`, decimal | |
| B233B | 388 | `C1`–`C2` | `CB`, decimal | |
| K43 | 417 | `C1`–`C3` | `CB`, decimal | |
| K84 | 427 | `C1`–`C3` | `CB`, decimal | |
| T3 | 434 | `C1`, `CA` | `CB`, decimal | |
| K182 | 423 | `C1`–`C2` | `CB`, decimal | |
| K109 | 428 | `C1`–`C2` | `CB`, decimal | |
| K177 | 416 | `C1`–`C2` | `CC`, decimal | |
| K107A | 531 | `C1`–`C3` | `CB`, decimal | CVN flag |
| K107B | 532 | `C1`–`C3` | `CC`, decimal | CVN flag |
| K118 | 530 | `C1`–`C2` | none | |
| P16 | 11242 | `C1`–`CA`, plus `40`, `42`, `44`, `67`, `69`, `71`, `74`, `76` | `CB`, decimal | `$90` |
| K73 | 601 | none | `CB`, decimal | `$90` |
| K124 | 396 | `C1`–`CA`, plus `60`, `62`, `64`, `66`, `68`, `6A` | `CB`, decimal | |
| K74 | 579 | `C2`–`CA`, `71` | `C1`, decimal, the same width as a part | |
| K114B | 529 | `C1`–`C2` | none | `$90` |
| K36 | 421 | `C1`–`C3` | `C1`, the same read as the first part | `$90` |

K17's hardware length is 0, so `ssecuhn` stays the string from `$CC`. The other hardware cells in the table are the four-byte decimal. `$90` is collected only on the rows that say so.

K16 is in the catalog, device id 205, request method 583, and it is not in the table. Its part list is the string `P;C0,C1,C2` with length 0, and the builder does not split it. `C0` appears in that string and in no pairable rule. K73 request methods 520 and 566 copy three bytes from `$81` and do not turn that copy into `ssecuhn`. Method 601 is the K73 row above.

`voltkit profile COMn --set parts --set sps-extra --clone` asks `$1A` `C0` through `CA`, then `40` through `4A`, `60`, `62`, `64`, `66`, `67`, `68`, `69`, `6A`, `71`, `73`, `74`, and `76`, on `7E0`, `7E1`, `7E4`, `7E7`, and `241`. `sps-extra` does not repeat `C0`. `identity` already asks `90` and `CC` on some of those headers. `90`, `CB`, and `CC` are not in `sps-extra`. No SPS device id has a recovered CAN header, so the read cannot be aimed at a controller by its device id.

That saved read is what lets `getProgrammingData` name each blob, its size, its download id, and the server list. Fetching the file is the following SPS call. Putting a blob on a module is a later write, and it is outside this capture.

## What a live `$1A` `$B0` answer would decode

`5247` and `5248` stay out. They are names in the Voltage catalog, and they were not found as `$22` reads in `c6.v` or the callers next to it. No header is recorded for either one.

`spsvcs.dll` fills its CAN matrix only from a live `$1A` `$B0` response. A positive reply is at least `0x0C` bytes, with byte `0x0A` equal to `5A` and byte `0x0B` equal to `B0`. `V` is the big-endian dword in bytes 5 through 8. The device id is byte `0x0C`. The request id is derived, not stored as its own byte:

- When `V` is `641` through `65F`, the request id is the low byte of `V` with `0x200` set, and the response id is `V`. A decoded row would print a request in `241` through `25F` and a response in `641` through `65F` with the same low byte. The device byte is independent of that low byte. The image does not say which device byte arrives with which `V`.
- When byte 0 is `4` and `V` is `7E8` through `7EF`, the request id is `V` minus 8, so `7E0` through `7E7`, and the response id is `V`.

Any other `V` does not fill the ids. Device 40 has no static row. Expanding either range into one row per id would invent devices the DLL does not contain. This command does not send `$B0` and does not decode that reply.
