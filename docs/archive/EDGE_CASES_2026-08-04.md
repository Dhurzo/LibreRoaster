# LibreRoaster — Detected Edge Cases (E2E Audit)

> **Historical snapshot (2026-08-04, status note updated 2026-10-17).** Findings are preserved as
> written. This is not the current status. Current verification is owned by
> [`../DEVELOPMENT.md`](../DEVELOPMENT.md); current project state is summarized in
> [`../../CONTEXT.md`](../../CONTEXT.md).

**Date:** 2026-08-04 (snapshot; current-status note updated 2026-10-17)
**Method:** strict E2E verification audit from three perspectives: data flow, contract compliance,
and edge cases/failures.
**Overall result:** ✅ No execution blockers. Status on 2026-10-17: **913/913 host tests pass**
(`cargo test --target x86_64-unknown-linux-gnu --features test --lib --tests`, 579 lib + 334
integration; it was 735/735 on 2026-09-09 and 646/646 on 2026-08-04), strict Clippy clean, embedded
build `riscv32imc-unknown-none-elf` OK. **`CONTEXT.md` is the canonical source for current state**;
this document is a historical snapshot and its narrative is preserved as written. Findings closed
since this audit include EC-A6 (`ERR rate_limited` no longer exists: no emitters and no
`MAX_COMMANDS_PER_TICK`), EC-34 (no per-tick command budget), and the dump now uses an aggregate
`DUMP_BUFFER_SIZE = 8192` with streaming ≤4 rows/tick (`String<128>` rows).
**Status:** findings marked (RESOLVED) are closed in the working tree; (OPEN) findings are pending
defensive improvements. `transport_tasks.rs` references are pre-F5.3 (current readers:
`usb_cdc/tasks.rs`, `uart/tasks.rs`); `roaster_control.rs` was split in v5.4
(`controllers/sensor|actuator|safety|dispatch.rs` + `ssr_logic.rs`).

---

## 1. Executive Summary

The system works end to end in a real scenario (Artisan + USB CDC / UART, manual/PID/profile
control, telemetry, multi-layer safety). This list documents the edge cases analyzed, verified, or
detected during the audit, with file:line evidence and cross-references to the documentation. None
break E2E execution; open findings are minor defensive risks already mitigated somewhere in the
design.

---

## 2. VERIFIED Edge Cases (behave as expected)

| # | Edge case | Result | Evidence |
|---|---|---|---|
| EC-01 | Partial command with a full queue (overflow) | ✅ Queue is cleared and `ERR buffer_overflow` is emitted at the next terminator; commands are not corrupted | `src/hardware/transport_tasks.rs:132-152` (Bug #2) |
| EC-02 | CRLF terminators (stray LF after CR) | ✅ The `while has_terminator` loop consumes the LF without wasting turns; one command per arrival, not one per two | `src/hardware/transport_tasks.rs:354-382` (Bug B11) |
| EC-03 | Command > 256 bytes (long PROFILE) | ✅ Explicit `CommandTooLong`, no silent truncation (previously `String<128>` cut profiles at ~170 B) | `src/input/parser.rs:79-98` (Bug B8) |
| EC-04 | NaN/Inf in operator inputs | ✅ Rejected in the parser for OT2 (M6), PIDGAIN/PID;T/PID;LIMIT (B21, PROTO-1), SV/SETTARGET/PREHEAT/PROFILE (B9); NaN in PV → emergency | `src/input/parser.rs:250-252, 364-369, 416-418, 470-472`; `src/control/roaster_control.rs:666-669` |
| EC-05 | `PID;CHAN;3\|4` | ✅ Rejected with `ERR out_of_range` (only 1=ET, 2=BT; previously executed as BT silently) | `src/input/parser.rs:372-389` (Bug P12) |
| EC-06 | Artisan handshake (UNITS/FILT) | ✅ `#OK` with the `#` prefix (an ArduinoTC4 driver requirement); bare `OK` previously broke initialization | `src/control/roaster_control.rs:1345-1378`; `src/output/artisan.rs:312-316` (Bug P-TC4, RESOLVED) |
| EC-07 | Delimiters `OT1,75` / `IO3=50` / `OT1;75` | ✅ All 4 delimiters (space, `;`, `,`, `=`) accepted only for actuators; does not corrupt FILT/PROFILE/PID,ON | `src/input/parser.rs:154-179` (Bug P-TC4, RESOLVED) |
| EC-08 | OT2 out of range (e.g. 150, -5, 50.5) | ✅ ±0.5 rounding, 0-100 clamp, `ERR OT2_CLAMPED fan=<n> heater_unchanged`; heater/PID untouched | `src/input/parser.rs:460-485` (Bug L10); `src/control/roaster_control.rs:1193-1217` |
| EC-09 | STOP with the heater on | ✅ Latch armed (`Error` state), heater 0, fan 100 % persistent (cooldown latch); while faulted, READ/STATUS/STOP/EmergencyStop/START/PREHEAT/CHAN/UNITS/FILT/STREAM are accepted (`roaster_control.rs:1232-1251`) | `src/control/roaster_control.rs:1226-1259, 991-1006` |
| EC-10 | STOP → recovery | ✅ `PID;OFF` and `START`/`PREHEAT` clear the latch (previously only a path with no protocol producer, a permanent brick) | `src/control/roaster_control.rs:1035-1038, 1112-1114, 1499-1501` (P3/V2-1) |
| EC-11 | Duplicate START during an active roast | ✅ Ignored; original `profile_start_time` untouched (does not restart the roast clock) | `src/control/roaster_control.rs:1089-1094` |
| EC-12 | PREHEAT → START (B14 handoff) | ✅ Clean transition; `profile_start_time` set; time backstops active | `src/control/roaster_control.rs:1096-1170` |
| EC-13 | START from Idle with PID already enabled (PID;SV) | ✅ Takes the full handoff (the previous `is_streaming()` gate swallowed START) | `src/control/roaster_control.rs:1089-1090` (V2-4) |
| EC-14 | Artisan disconnect with the heater on | ✅ `COMMS_IDLE_TIMEOUT_MS` (15 s) with the physical gate `heater_energized \|\| roast_active` → emergency (previously only roast state; manual mode unprotected) | `src/control/roaster_control.rs:498-522` (V2-16c) |
| EC-15 | Manual session without START (OT1 only) | ✅ Own time budget (H2): `heat_session_start` (M3) + 5400 s manual cap (`MAX_MANUAL_HEAT_SESSION_SECS`); START-anchored roast keeps 1800 s + comms-idle | `src/control/roaster_control.rs:645-660` |
| EC-16 | Momentary `OT1 0` between commands | ✅ MAX_ROAST_TIME budget is not restarted: 60 s heater-off debounce | `src/control/roaster_control.rs:535-550` (R7) |
| EC-17 | Long preheating (> 30 min, large drum) | ✅ Excluded from MAX_ROAST_TIME (P6); comms-idle still covers it | `src/control/roaster_control.rs:567-569` |
| EC-18 | High RoR during PREHEAT (empty drum) | ✅ Guard disarmed in Preheating (V2-16a); armed only in Heating/Stable or Idle+PID+heater (P4) | `src/control/roaster_control.rs:699-737` |
| EC-19 | `PID;CHAN;1` (ET as PV) | ✅ BT-only RoR guard (M4/P1); Heating↔Stable transitions use the active PV (L11/F); no flip-flop | `src/control/roaster_control.rs:699-737, 1682-1719` |
| EC-20 | Stuck probe (TC short reads a valid ~0 °C without a fault bit) | ✅ Probe-stuck detector armed at any duty > 0 (S1); two stages in manual/software-PID mode (A-TC4-C): wire warning at 120 s (`ERR probe_stuck_warning`), latch at 300 s; in firmware-PID, latch at 120 s outside ±5 °C of target; disarmed near the target (legitimate regulation) | `src/control/roaster_control.rs:1010-1097` (P5/S1/A-TC4-C) |
| EC-21 | BT = 0.0 during cooldown (status default / MAX31856 POR) | ✅ Does not release the cooldown latch; requires real BT > 0 and < 60 °C (R8) | `src/control/roaster_control.rs:590-604` |
| EC-22 | Fan never configured (no OT2/FANPROFILE) with heater on | ✅ `FAN_MIN_SAFETY_PCT = 20 %` applied (previously heater up to 100 % with 0 airflow) | `src/control/roaster_control.rs:853-876` (Bug A) |
| EC-23 | Heater/fan write failure in the control loop | ✅ Escalated to a full `emergency_shutdown` (previously warn-only with the SSR in an unknown state) | `src/control/roaster_control.rs:812-827, 885-888` (Bug B) |
| EC-24 | `ssr_cycle_busy` on OT1 (100 ms guard) | ✅ Hardware-first: software state is committed only after an accepted write; the next tick retries | `src/control/roaster_control.rs:282-312` (M10/Bug C) |
| EC-25 | Sensor never read + START (PID against PV=0) | ✅ Treated as stale → PID holds the last applied output (previously ramped to 100 % without feedback) | `src/control/roaster_control.rs:762-774` (NEW-5/R2) |
| EC-26 | Command on the inactive channel (UART while USB is active) | ✅ Explicit `ERR command_ignored_inactive_channel` (previously silent discard, STOP lost) | `src/hardware/transport_tasks.rs:272-277` (Bug D) |
| EC-27 | Read error on the inactive transport | ✅ Does not count toward the emergency threshold (active channel only; previously a dead UART could abort a USB roast) | `src/hardware/transport_tasks.rs:162-170, 459-479` (P7) |
| EC-28 | Garbage in the boot window (channel None) | ✅ Parse errors do NOT activate the multiplexer from None (previously UART could hijack the channel) | `src/hardware/transport_tasks.rs:283-337` (P8) |
| EC-29 | USB host disappears (Artisan dies, cable unplugged) | ✅ USB write with 50+20 ms timeout (A2); line discarded, next tick sends fresh data; UART stays operational | `src/hardware/usb_cdc/driver.rs:59-90` |
| EC-30 | Duplicate READ response | ✅ Only `drain_commands` emits; `handle_read_status` only validates the format (historical Bug #3) | `src/application/tasks.rs:324-339`; `src/control/roaster_control.rs:1292-1313` |
| EC-31 | Light-roast turnaround triggers a false RoR trip (0.6 °C/s ~3 s after charge) | ✅ Two-band RoR guard (A-TC4-D): soft band 0.75–1.0 °C/s requires 12 sustained ticks (~3.7 s); hard band > 1.0 °C/s keeps the 3-tick latch. Values are provisional pending HIL | `src/control/controllers/sensor.rs` (`tiered_ror_trip`); `src/config/constants.rs` (`MAX_BT_RATE_OF_RISE_HARD`, `ROR_SOFT_DEBOUNCE_LIMIT`) |
| EC-32 | Second `#DUMP` mid-drain | ✅ `dump_pending.clear()` at the start; dumps never splice together | `src/control/roaster_control.rs:1468-1493` (V2-7) |
| EC-33 | Output channel full during #DUMP | ✅ Re-push to the front of the deque; no row is lost (FIFO preserved) | `src/application/tasks.rs:849-864` (V2-7); `src/control/roaster_control.rs:1454-1466` |
| EC-34 | Artisan startup burst (10-15 lines) | ✅ Channel 16 (previously 8): burst fits within one tick window; `MAX_COMMANDS_PER_TICK == ARTISAN_CMD_CHANNEL_SIZE == 16` with emergency bypass | `src/application/service_container.rs:41-49` (E1); `src/application/tasks.rs:247-266` |

---

## 3. OPEN Findings (defensive improvements, non-blocking)

### EC-A1 — Uncommitted P-TC4 bug diff ⚠️ ~~(process blocker)~~ → **RESOLVED (2026-08-05)**
- ~~**File**: working tree, `develop` branch (last commit 475581b)~~
- **Status**: the handshake fix (`#OK` for UNITS/FILT, `,`/`=`/`DCFAN` delimiters) is committed in
  `47e4942` (“fix(protocol): Artisan ArduinoTC4 handshake compatibility”). Verified in Phase 0 of
  BUG-CATCH-PLAN.md: HEAD = `52b6bdf` contains the fix, and the HEAD build responds `#OK`. No diff
  remains.

---

## 3b. Security-Audit Findings (BUG-CATCH-PLAN.md, 2026-08-05)

Result of phases 0–4 (baseline, static repros, mid-roast injection, proptests, invariant harness
N=1000). Each finding had a reproduction test (red pre-fix), and **all fixable findings were fixed
on 2026-08-05** (green post-fix). Severities follow the plan triage: BLOCKER / SAFETY-CRITICAL /
OPERATIONAL / LATENT.

| # | Severity | Finding | Reproduction | Status |
|---|---|---|---|---|
| **S1** | SAFETY-CRITICAL | Manual mode unsupervised with a dead but finite probe (shorted TC reads a valid 0.0 °C, no fault bit): overtemp/RoR/staleness do not trigger; probe-stuck was armed only at heater ≥50 %; comms-idle neutralized by Artisan's READ polling. OT1 <50 % with a dead probe = heater blind until MAX_ROAST_TIME (30 min). Harness I6 exposed it in **165/1000 random roasts** | `dead_probe_manual_roast_now_trips_probe_stuck` (verifies post-fix emergency); harness I6: **0/1000** | ✅ **RESOLVED**: probe-stuck detector armed at any duty > 0 (`roaster_control.rs`) |
| **S2** | Design (record decision) | START/PREHEAT/OFF are whitelisted during `fault_condition` and clear the emergency latch (serial re-energization possible after a real overtemp). Documented firmware decision: trust the operator (Bug P3) | `s2_serial_start_clears_latched_emergency_and_reenergizes` | **Keep** (firmware design decision) |
| **S3** | OPERATIONAL | `PID;CT` accepted an unbounded u32: `PID;CT 4294967295` freezes the PID throttle → heater holds the last output without regulation | `s3_pid_cycle_time_huge_freezes_regulation` | ✅ **RESOLVED**: 10..=60,000 ms bound in `parser.rs` |
| **S4** | OPERATIONAL | Internal-trap emergencies silently absorbed total fan failure: `force_fan_100` failed, but the Err was only `EmergencyShutdown` | `s4_internal_trap_absorbs_fan_failure` + in-crate test `emergency_shutdown_fan_total_failure_keeps_fan_output_honest` | ✅ **RESOLVED**: `Err(HardwareError(emergency_fan_failed))` (`actuator.rs`) |
| **S5** | LATENT | `apply_guarded_heater(NaN)` accepted NaN → `ssr_output = NaN` → comms-idle and MAX_ROAST_TIME inert (`NaN > 0.0 == false`) | `s5_nan_input_poisons_ssr_output_and_disarms_backstops` (un-ignored, GREEN) | ✅ **RESOLVED**: `!desired.is_finite()` guard → Err (`actuator.rs`) |
| **S6** | OPERATIONAL (low) | `OT2 0` with heater on: the command path wrote fan 0 % without the `FAN_MIN_SAFETY_PCT` floor (~330 ms + fade) | `s6_ot2_zero_bypasses_fan_floor_until_next_tick` (expects immediate floor, GREEN) | ✅ **RESOLVED**: floor applied in `apply_policy_outcome` (`roaster_control.rs`) |
| **S7** | Telemetry (minor) | `ssr_output = 0.0` unconditionally during emergency even if the SSR might be stuck on | T1 `heater_write_failure_mid_roast_escalates_to_latched_emergency` (expects `ssr_output > 0` + Error status) | ✅ **RESOLVED**: `ssr_output` is zeroed only if `force_heater_off` succeeds (`actuator.rs`) |
| **S8** | OPERATIONAL (EC-A2) | UART TX without timeout (asymmetry with USB): a hung `flush()` would freeze all output | — (riscv32-only driver) | ✅ **RESOLVED**: 50+50 ms timeout in `UartTxDriver::write_bytes` (`uart/driver.rs`) |
| **S9** | LATENT (LOW) | SW watchdog sentinel: `LAST_FEED_MS == 0` meant both “never fed” and a real timestamp → a feed at t=0 disabled the timeout forever | `software_watchdog_times_out_after_missed_feeds` | ✅ **RESOLVED**: `NEVER_FED = u64::MAX` sentinel (`watchdog.rs`) |
| **S10** | LATENT | `normalize_read_value` only mapped non-finite values to 0.0; a huge finite value truncated the READ buffer mid-number → corrupt token (“-”) | `src/output/artisan.rs::format_read_never_panics_with_hostile_status` (un-ignored, GREEN) | ✅ **RESOLVED**: ±1000 clamp in `normalize_read_value` (`artisan.rs`) |

**Bug-hunt verdict**: 735 host tests green (0 failures, 0 ignored; 674 on 2026-08-05), strict Clippy
clean, embedded build OK. No BLOCKER open. **All 10 fixable findings (S1, S3–S10) are RESOLVED with a
green reproduction test**; S2 remains as a design decision. Post-fix re-verification (1000-roast
harness + full suite) introduced no new bugs.

### EC-A2 — UART TX without timeout (asymmetry with USB) — ✅ RESOLVED (S8, 2026-08-05)
- **Evidence**: `src/hardware/uart/driver.rs:49-58` (`write_bytes` without `with_timeout`) vs
  `src/hardware/usb_cdc/driver.rs:70-89` (50+20 ms timeout, Bug A2).
- **Risk**: low in practice — the UART TX FIFO drains in hardware at 115200 baud without depending
  on the host. A driver failure that hung `flush()` would freeze `dual_output_task` and all output.
- **Applied fix**: `with_timeout(50 ms)` on write + `with_timeout(50 ms)` on flush inside
  `UartTxDriver::write_bytes` → `UartError::TransmissionError` on time (symmetric with USB).

### EC-A3 — Silent output-channel drops under load
- **Evidence**: `src/application/service_container.rs:48-49` (16-slot channel); all producers use
  `try_send` (`src/application/tasks.rs:316, 333, 263`; `src/control/roaster_control.rs:1616-1627`);
  drain 4 msgs/5 ms (`tasks.rs:1090-1121`, Bug E2).
- **Risk**: during a `#DUMP` (256 rows, ~20 s) with a host that does not read, later SAFETY/ERR
  lines can be dropped. Wide margin in normal operation (producers ≈1-4 lines/tick of ~310 ms vs
  drain capacity ≈800 lines/s).
- **Suggested solution**: priority slot for SAFETY/ERR or age-based drop.

### EC-A4 — Doc-code discrepancy: telemetry cadence
- **Evidence**: `docs/PROTOCOL.md:157-160` says “once per control tick”; the implementation emits
  with a 1 Hz wall-clock gate (`src/config/constants.rs:143` `DEFAULT_OUTPUT_INTERVAL_MS=1000`;
  `src/application/tasks.rs:768-774`, Bug M1).
- **Impact**: none (the code is correct; ARCHITECTURE.md §9 already says 1000 ms). Update
  PROTOCOL.md.

### EC-A5 — READ response latency (performance note)
- **Evidence**: `src/application/tasks.rs:233-360` (drain once per tick); real tick ≈310-330 ms
  (`src/config/constants.rs:233` `CONTROL_LOOP_TICK_MS`).
- **Impact**: a READ response can take up to ~310 ms. Within Artisan's timeouts, but the weakest E2E
  point under aggressive timeout settings.

### EC-A6 — `ERR rate_limited` spam under extreme bursts
- **Evidence**: `src/application/tasks.rs:254-266` — commands exceeding the limit are consumed from
  the channel with `continue`, emitting one ERR per surplus command (up to 8).
- **Impact**: wire noise, no loss of functionality. The emergency bypass (STOP) is correct.

---

## 4. Edge Cases Documented in the Repo and Verified in Code

| Doc ref | Finding | Verification |
|---|---|---|
| PROTOCOL.md §11 | `UNITS` does not persist across power cycles | ✅ `TemperatureSettings` is RAM-only (`src/config/constants.rs:655-706`) |
| PROTOCOL.md §11 | `FILT` coerces malformed input to 0 | ✅ `src/input/parser.rs:118-132` (documented, accepted risk) |
| PROTOCOL.md §6 | Bare `OFF` is not parsed (only `PID;OFF`) | ✅ `parse_pid_subcommand` (`src/input/parser.rs:311-423`); bare `OFF` → `unknown_command` |
| PROTOCOL.md §10 | `handler_failed <token>:<source>` with valid tokens | ✅ `src/application/tasks.rs:1057-1073` |
| ARCHITECTURE.md §9 | Real cadence ≈310-330 ms; SW watchdog 1000 ms | ✅ `src/config/constants.rs:224-233`; feed 1/tick ≈3 feeds/window |

---

## 5. Audit Methodology and Execution

1. **Data flow**: full trace from input → parsing → multiplexer → channel → handlers → formatting →
   output transport, with attention to `Option`/`None` states, types, and implicit assumptions.
2. **Contracts**: every command/parameter/response checked against `docs/PROTOCOL.md` (last updated
   2026-08-04).
3. **Edge cases**: simulated failure conditions, concurrency (async mutex, `ssr_cycle_busy`),
   latency (310 ms tick), exceptions (NaN/Inf), and intermediate states (latch, cooldown).
4. **Quality gates executed**:
   - `cargo fmt --all -- --check` → OK
   - `cargo clippy --locked --all-targets -- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic` → OK
   - `cargo test --target x86_64-unknown-linux-gnu --features test --lib --tests --no-fail-fast` → **735 passed, 0 failed (646 on 2026-08-04)**
   - `cargo build --release --target riscv32imc-unknown-none-elf --features embedded` → OK
