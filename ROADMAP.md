# voltkit roadmap

`voltkit` is the user interface for one car. The peer is a TCP ELM327, a Windows COM port, or an
OBDLink MX opened natively over the host Bluetooth radio. `vcxtoolkit` (the VCX-NANO doctor),
`dangle` (the native host-radio BLE library), and `fakeobd` (the in-process bench) are optional,
feature-gated library links; `dangle`'s TCP-bridge binary (the optional `voltkit bench` path) and
`spskit` stay separate processes. Rows are `Status | Obligation | Anchor | Test`, the grammar `projecthub sync`
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

## Adapter detection and selection

The wizard detects the adapters present and lets the operator select one, instead of typing a COM
port, then auto-writes the profile outside the repository. The peer dependencies are optional Cargo
features: `vcx` (the VCX-NANO doctor) and `ble` (native host-radio BLE via `dangle`) on by default
but both Windows-gated, and `lab` (the in-process `fakeobd` bench) off by default. Cargo still
resolves every path-dep manifest, so a build needs the siblings present even where it does not
compile them; the default Linux binary links only `udskit`.

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [ok] | Scan the host Bluetooth radio natively (`dangle::scan_advertisers`) and offer only the serial-service (MX) advertisers as BLE devices, each carrying its address. voltkit opens the radio in-process; it spawns no bridge | `src/detect.rs`; `dangle` README | `detect::a_serial_advertiser_becomes_a_native_ble_device`; `detect::a_ble_target_round_trips_through_its_address` |
| [ok] | Turn a chosen adapter into a capture target (TCP, Windows COM, or a native `ble:<address>` dongle), and keep the gated VCX-NANO identify-only with no target | `src/detect.rs`; `AGENTS.md` | `detect::a_link_becomes_a_capture_target_except_the_gated_vcx`; `detect::the_menu_order_is_vcx_then_serial_then_ble_then_remembered` |
| [ok] | Name the capture file from the job and the time, outside the repository, with no VIN in the name | `src/detect.rs`; `.gitignore` | `detect::the_capture_file_is_named_from_the_job_and_the_time`; `detect::timestamp_matches_known_dates` |
| [ok] | Pick a capturable device or inform on a VCX-NANO, with manual entry as a fallback, in the full-screen UI | `src/tui/detect.rs` | `detect::a_capturable_device_is_picked_and_a_vcx_only_informs`; `detect::manual_and_rescan_sit_after_the_devices` |
| [ok] | Build without the `vcx` feature (or off Windows): the VCX probe is empty and the wizard falls back gracefully | `Cargo.toml`; `src/detect.rs` | `detect::without_the_vcx_feature_the_probe_is_empty` |
| [ok] | Run the wizard as two phases: an ELM adapter is picked for the Phase 1 module clone, a VCX-NANO is routed to Phase 2 | `src/tui/detect.rs`; `src/tui/stage.rs` | `detect::phase_1_picks_an_elm_adapter_and_phase_2_routes_a_vcx`; `stage::enter_runs_then_returns_and_esc_goes_back` |
| [ok] | Phase 2: stage a gated spskit-compatible SPS collect skeleton from voltkit's own parts/hardware/VIN DIDs, every read value null, marked gated. Send no vehicle read | `src/stage.rs`; `spskit/spskit/vit1.py`; `AGENTS.md` | `stage::the_reads_are_vin_parts_and_hardware_per_programming_header`; `stage::the_skeleton_is_gated_json_with_empty_values`; `stage::the_skeleton_name_is_the_serial_and_the_time_without_a_vin` |

## Peers this interface starts

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [ok] | Offer the lab peer: start `fakeobd serve` on `127.0.0.1:35000` and dial it. fakeobd admits the read set and refuses `$27`. It has no serial or Bluetooth server | `fakeobd` `serve`; `crates/obd-policy` | `lab::serve_listens_on_127_0_0_1_35000`; `the_lab_command_starts_fakeobd_serve_on_127_0_0_1_35000_and_dials_it` |
| [ok] | Offer the optional bench peer: start the `dangle --name OBDLink` TCP bridge and dial `127.0.0.1:35000`. This is the bring-the-bridge path beside the wizard's native BLE; the wizard itself opens the radio in-process | `dangle` README | `bench::bridge_args_are_name_obdlink_and_the_socket_is_127_0_0_1_35000`; `bench::only_a_listening_line_for_this_socket_names_the_bench_peer`; `the_bench_command_starts_dangle_named_obdlink_and_dials_127_0_0_1_35000`; `bench_does_not_dial_a_listener_already_bound_on_127_0_0_1_35000` |

## Peers the interface does not launch yet

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [open] | Hand the programming read to `spskit` `build_vit1` / `programming_data_request`. spskit does not post the body and does not program the car. No CLI wraps that function today | `spskit/vit1.py`; `spskit/docs/tool-boundary.md` | none |
| [open] | A fake SPS peer that answers `getProgrammingData` from a saved body. No `fakesps` directory exists. A shell is not created until a captured exchange is the contract | searched `D:\code` 2026-11-21 | none |
| [ok] | Identify a VCX-NANO in the wizard with vcxtoolkit's `open_usb` + GetInfo doctor (model, firmware), retried, on Windows with the `vcx` feature, and stage the gated SPS skeleton from it. Do not send a vehicle read — the live `collect` stays gated in vcxtoolkit | `vcxtoolkit` README; `docs/tool-boundary.md`; `src/detect.rs`; `src/stage.rs` | `detect::a_com_name_yields_its_number` (the port the doctor probes); `stage::the_skeleton_is_gated_json_with_empty_values`; hardware GetInfo stays `#[ignore]` in `vcxtoolkit` |
| [open] | Run the live SPS collect through the VCX-NANO (the gated read), not only stage it. The `VCX_PRO_Recv` byte layout is not recovered, so the wizard stages the skeleton and holds the read for owner sign-off | `vcxtoolkit` README; `src/stage.rs` | none |
| [ok] | Native host-OS BLE inside voltkit, dropping the bridge hop: scan and connect to the dongle over the host radio in-process (no spawned process, no TCP socket), driving the ELM327 session over the returned byte pipe. Linked through the `dangle` BLE library (the `ble` feature) | `dangle` README; `src/elm.rs`; `src/detect.rs` | `detect::a_serial_advertiser_becomes_a_native_ble_device`; `detect::a_ble_target_round_trips_through_its_address` |
| [no] | Install or register a J2534 pass-through DLL. `vcxtoolkit` has no `cdylib`, no `PassThru*` export, and no `PassThruSupport.04.04` writer. SAE J2534 is not in primarysource, so it stays a greyed, unbuilt option | `vcxtoolkit/docs/tool-boundary.md` | none: the fifteen exports stay unbuilt |
