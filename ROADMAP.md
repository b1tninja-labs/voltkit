# voltkit roadmap

`voltkit` is the user interface for one car. The peer is a TCP ELM327. The other tools stay
separate processes. Rows are `Status | Obligation | Anchor | Test`, the grammar `projecthub sync`
reads for Rust projects. This repository is registered there as tooling on 2026-11-21, public at
<https://github.com/b1tninja-labs/voltkit>, with `publish = false`. These rows are the matrix
that registration syncs.

The service-id framing, the `+0x40` positive-response offset, the ReadDataByIdentifier codec,
and the GMLAN `+0x400` addressing come from the sibling `udskit` crate (a path dependency), so
`voltkit` and `gmdiag` read one UDS/GMLAN model instead of each hand-rolling it. The refactor is
internal: the read-only scope, the scan set, and every obligation below are unchanged, and the
named tests still pin them.

Status:

- `[ok]` the named test exists and passed on 2026-11-21.
- `[open]` not built.
- `[no]` a deliberate non-goal until the named document is imported.

## The reader that exists

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [ok] | Speak ELM327 over TCP, and on Windows over a COM port. Refuse `$27` and the write services before a byte is written | `docs/capture.md`; `AGENTS.md` | `elm::security_access_and_writes_are_refused_before_any_socket`; `scan::the_session_is_the_init_then_reads_and_never_security_access` |
| [ok] | Ask the lab profile, the programming read, presence, one screen, or a range, and send nothing until the answer is yes | `docs/capture.md` | `wizard::the_lab_job_is_the_emulation_pass`; `wizard::a_screen_can_be_declined_and_security_access_is_refused` |
| [ok] | Write a fakeobd profile from the answers, and keep a capture from a car out of git | `README.md`; `.gitignore` | `capture::the_clone_is_one_ecu_per_answered_header` |
| [ok] | Name a US Chevrolet Volt (2016–2019) or Bolt EV (2017–2019) from the primarysource `vin` filings. DC fast charge is the 2019 card only. The 2016 filing has no Bolt page | `vin:gm-vin-2016§33`, `vin:gm-vin-2017§35`, `vin:gm-vin-2018§28`, `vin:gm-vin-2019§27`; Bolt `vin:gm-vin-2017§25`, `vin:gm-vin-2018§20`, `vin:gm-vin-2019§19` | `vin::identify_names_a_volt_built_from_the_2016_card`; `vin::a_string_built_from_the_bolt_card_names_the_year_and_the_series` |
| [ok] | Print six scaled `$22` reads from the scan table, and scale the other known formulas only for a reply already in hand. The command dials only `127.0.0.1`, and only when `--host` is set | `src/gauges.rs`; `README.md` | `gauges::gauge_lines_are_the_six_mapped_reads`; `gauges::scaled_fields_come_from_reply_bytes_without_a_socket`; `gauges::table_scales_come_from_reply_bytes_without_a_socket`; `elm::a_gauge_host_must_be_127_0_0_1`; `gauges_print_the_scaled_fields`; `gauges_do_not_connect_without_the_host_flag`; `a_gauge_host_other_than_127_0_0_1_is_refused` |

## Peers this interface starts

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [ok] | Offer the lab peer: start `fakeobd serve` on `127.0.0.1:35000` and dial it. fakeobd admits the read set and refuses `$27`. It has no serial or Bluetooth server | `fakeobd` `serve`; `crates/obd-policy` | `lab::serve_listens_on_127_0_0_1_35000`; `the_lab_command_starts_fakeobd_serve_on_127_0_0_1_35000_and_dials_it` |
| [ok] | Offer the bench peer: start `obd-bridge --name OBDLink` and dial `127.0.0.1:35000`. An OBDLink MX is that BLE ELM327 when it advertises the FFF0 serial service. voltkit does not open the radio | `obd-bridge` README | `bench::bridge_args_are_name_obdlink_and_the_socket_is_127_0_0_1_35000`; `bench::only_a_listening_line_for_this_socket_names_the_bench_peer`; `the_bench_command_starts_obd_bridge_named_obdlink_and_dials_127_0_0_1_35000`; `bench_does_not_dial_a_listener_already_bound_on_127_0_0_1_35000` |

## Peers the interface does not launch yet

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [open] | Hand the programming read to `spskit` `build_vit1` / `programming_data_request`. spskit does not post the body and does not program the car. No CLI wraps that function today | `spskit/vit1.py`; `spskit/docs/tool-boundary.md` | none |
| [open] | A fake SPS peer that answers `getProgrammingData` from a saved body. No `fakesps` directory exists. A shell is not created until a captured exchange is the contract | searched `D:\code` 2026-11-21 | none |
| [open] | Offer the dongle as the `vcxtoolkit` frame and, on Windows, its USB serial open. Do not send a vehicle write | `vcxtoolkit` README; `docs/tool-boundary.md` | none |
| [no] | Install or register a J2534 pass-through DLL. `vcxtoolkit` has no `cdylib`, no `PassThru*` export, and no `PassThruSupport.04.04` writer. SAE J2534 is not in primarysource | `vcxtoolkit/docs/tool-boundary.md` | none: the fifteen exports stay unbuilt |
