# Safety case — hazards, guards, evidence

Each row: hazard → guard in firmware → automated evidence (test) → bench evidence still required.

| # | Hazard | Guard | Automated evidence | Bench |
|---|---|---|---|---|
| 1 | Bean/drum over-temperature | BT ≥ 260 °C, ET ≥ 300 °C → latch | `overtemp_bt_triggers_emergency`, `overtemp_et_triggers_emergency`, `t1_*` | Measure real ET peak |
| 2 | Probe open/short/NaN | NaN PV → latch; probe-stuck (two-stage manual, PID 120 s) | `nan_pv_triggers_emergency_during_roast`, `h8_shorted_probe_latches`, `r2_frozen_hot_bt_with_moving_et_latches` | Unplug a TC mid-roast |
| 3 | Stale sensor data | 1 s validity → latch | `s3_stale_sensor_mid_roast_trips` | — |
| 4 | Host lost while heating | 15 s comms-idle → latch | `comms_idle_timeout_triggers_emergency_during_roast`, `comms_idle_protects_manual_mode_when_heater_energized` | Close Artisan mid-roast |
| 5 | Forgotten roast | 30-min roast / 60-min uncharged PID;ON / 90-min manual budgets | `max_roast_time_triggers_emergency_shutdown`, `h2_manual_session_cap_fires`, `t3_*`, `e1_*` | — |
| 6 | Runaway heating | RoR guard 45/60 °C·min⁻¹ (firmware PID) | `ror_guard_armed_in_pure_firmware_pid` | — |
| 7 | Heater/fan write failure | 3 retries → latch, fan 100 % | `heater_write_failure_mid_roast_escalates_to_latched_emergency`, `fan_write_failure_mid_roast_escalates_to_latched_emergency` | — |
| 8 | Firmware hang | RWDT fed only by the control tick, ~2.2 s system reset | `safety_thresholds_are_sane` (margin assert) | Scope GPIO10 on forced hang |
| 9 | Panic with heater on | `custom_pre_backtrace` drives GPIO10 LOW | build of all binaries | Scope GPIO10 on forced panic |
| 10 | Boot glitch energises SSR | mandatory 10 kΩ pull-down on GPIO10 | — | Scope GPIO10 at power-up |
| 11 | Unsafe re-energise after a trip | latch; PID;ON never clears it | `pid_on_rejected_while_latched_and_keeps_latch`, `h11_pid_on_while_latched_is_rejected` | — |
| 12 | Setpoint above cutoff | PID target capped 10 °C under the channel cutoff | `t2_*` | — |
| 13 | RoR-follow drives a runaway target | setpoint clamped to BT ± 3 °C, RoR ≤ 30 °C/min, BT channel only, stops on DROP/SV/latch | `setpoint_always_within_lead_of_bt` (proptest), `e3_*` | First RoR-follow roast attended |
| 14 | Autotune overheats | manual mode only, aborts on any command/latch/BT ≥ 230 °C, duty ≤ base+step | `e4_*`, `too_hot_aborts_mid_test` | First TUNE attended |
| 15 | Old/new feature interaction | cross-feature fuzz invariants (128 random sequences per run) | `fuzz_new_commands_keep_safety_invariants` | — |
