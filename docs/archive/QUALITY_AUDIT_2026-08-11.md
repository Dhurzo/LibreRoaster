# Rust Code-Quality Audit — LibreRoaster

> **Historical snapshot (2026-08-11, updated 2026-10-17).** Findings and the remediation plan are
> preserved as written. This is not the current status. Current verification is owned by
> [`DEVELOPMENT.md`](../DEVELOPMENT.md); current project state is summarized in
> [`../../CONTEXT.md`](../../CONTEXT.md).

**Date:** 2026-08-11 (snapshot; updated 2026-10-17 — see note)
**Scope:** all of `src/` (94 files, ~24,068 lines), `tests/`, `Cargo.toml`, Clippy configuration
**Method:** parallel multi-agent audit (8 dimensions) plus manual verification of key findings and
objective gates

> Note 2026-10-17: **historical snapshot** — the findings and remediation plan are preserved as
> written; the canonical source for current state is `CONTEXT.md` (Quality Gates). The suite at that
> date had **913 tests** (`579 lib + 334 integration`) with `--features test` (it was 735 on
> 2026-09-09 and 631 on 2026-08-11). `roaster_control.rs` remains split (v5.4: `controllers/` +
> `ssr_logic.rs`). Findings from this report already remediated: M-A5 (`OutputController` is a
> functional flag; the no-op stub is gone), M-X2/H-4 (`ERR rate_limited` has no emitters and
> `MAX_COMMANDS_PER_TICK` is gone — there is no per-tick command budget), M-A7 (dead
> `RoasterState` variants `Cooling`/`Fault`/`EmergencyStop` removed); H-5/M-R2 was partially
> mitigated (dump rows `String<256>` → `String<128>`, aggregate `DUMP_BUFFER_SIZE = 8192` with
> streaming ≤4 rows/tick — the staging queue sized to the ring, ~33 KB, still exists). The canonical
> gate is in `CONTEXT.md` (Quality Gates), not here.

---

## 1. Executive Summary

| Dimension | Verdict | Note |
|---|---|---|
| Compilation and gates | ✅ **Excellent** | `cargo fmt --check` clean; `cargo clippy --all-targets` **0 warnings** (with `unwrap_used`/`expect_used`/`panic` denied); riscv32 build warning-free |
| Physical safety (thermal/SSR/WDT) | ✅ **Strong** | Real defense in depth: 3 independent layers; no HIGH/CRITICAL findings |
| Error handling | ✅ **Very good** | `unwrap`/`expect`/`panic` outside production verified; 2 gaps in the output channel |
| Concurrency and async | ✅ **Solid** | Conservatively `SeqCst`, no await while holding a lock, bounded channels |
| Protocol/parser | ✅ **Robust** | No CRITICAL/HIGH; hostile input well covered (proptest fuzz) |
| Embedded performance | 🟡 **Acceptable** | Loop is limited by sleep, not CPU (210 ms serialized); 1 static-RAM problem (66 KB) |
| Architecture | 🟡 **Incomplete** | “A decomposed shell around an undecomposed core”: `RoasterControl` is still a god-module |
| Tests (735; 631 on 2026-08-11) | 🟡 **Strong with cracks** | Excellent depth on critical paths; 1 real tautology, 1 vacuous test, a cfg that excludes the strongest numeric tests |
| API documentation | 🔴 **Weak** | ~705 `pub` items, ~30% documented; `missing_docs` would fail on 300+ |

**Totals:** 0 CRITICAL · 10 HIGH · 26 MEDIUM · 22 LOW

**Top 3 highest-impact findings:**

1. **`tasks.rs:1140/1143` — output write failures silenced** (HIGH): the only protocol output path
   discards `Result` with `let _ =`; if Artisan disconnects, responses are lost with no retry, log,
   or counter.
2. **`roaster_control.rs:86-87` — 66 KB static dump queue** (HIGH): ~16% of ESP32-C3 SRAM for a
   rarely used diagnostic, with rows 6× oversized.
3. **Tests: tautology in the parser proptest** (`parser.rs:712`) — the expected command is compared
   with `matches!(Ok(_expected_command))`, a fresh binding that is **never compared**: 37 commands
   only prove “it parses,” never “it parses to the right value.”

---

## 2. Objective Metrics (verified by direct execution)

| Metric | Value |
|---|---|
| `cargo fmt --all -- --check` | ✅ PASS |
| `cargo clippy --locked --all-targets` | ✅ 0 warnings (denies: `unwrap_used`, `expect_used`, `panic`, `fallible_impl_from`) |
| Host tests (`--features test`) | 735 passing (631 on 2026-08-11; see finding T-3: `regression` cfg excluded from the gate) |
| `unsafe` | 10 files; ~29 occurrences — verified: PAC register access (`RTC_CNTL`, `EFUSE`), `WDT` with correct unlock/relock, `static_cell` |
| `unwrap`/`expect`/`panic` in production code | **0** (all 116 occurrences are in `#[cfg(test)]` modules; only exception: intentional `unimplemented!()` in `conversion.rs:262`, gated to an unreachable configuration) |
| `#[allow]` | 33, all justified (test code or documented cases) |
| TODO/FIXME | 0 in `src/` |
| Files > 1,000 lines | 5 (`roaster_control.rs` 3229, `tasks.rs` 1921, `parser.rs` 1715, `artisan.rs` 1318, `simulated.rs` 956) |
| Atomics | `SeqCst` in watchdog/container (conservative but correct); `Relaxed` only in counters/metrics (acceptable) |
| Mutex-await | Verified: **no** `.lock().await` held across a second await; `with_roaster_async` closures are synchronous |

---

## 3. Strengths (verified correct)

These areas were audited and **confirmed well implemented**:

- **Thermal cutoff** (`sensor.rs:216-223`): correct `>=` comparison against
  `OVERTEMP_THRESHOLD: f32 = 260.0`, with `!bean_fault.has_fault()` so failed sensors do not trigger
  on corrupt data, and a latch at `sensor.rs:17` (no auto-rearm).
- **Fan-floor interlock** (`roaster_control.rs:365, 944`): `FAN_MIN_SAFETY_PCT = 20.0` forced when
  `ssr_output > 0.0`; the 2026-08-05 S6 bug is documented and fixed.
- **Stale-temperature guard** (`roaster_control.rs:835-852`): the PID holds with stale data instead
  of integrating against it; it also guards against running before the first read.
- **Watchdog** (`watchdog.rs`): 3× margin (feed ~330 ms vs 1,000 ms soft timeout) and 6.6× (RWDT HW
  2,206 ms, const-asserted in `constants.rs:181-184`); failed feed escalates to emergency after 2
  consecutive failures; saturation with `saturating_sub`; `NEVER_FED` sentinel.
- **Parser**: no panic/UB on hostile input (proptest fuzz `parse_never_panics`,
  `hostile_bytes_never_panic_and_never_unclamp`); numeric overflow → typed `Err`; NaN/Inf rejected
  with `is_finite()` on every float; exact arity; `update_control` performs no string formatting in
  steady state.
- **Temperature conversion**: lossy casts always pre-guarded with `clamp`/`min` (`fan.rs:53-58`,
  `ssr.rs:422-435`, `parser.rs:523`); every variable denominator has a divide-by-zero guard.
- **“Bug-anchored comments” culture**: every bug fix carries a `// Bug XX (date)` comment with the
  explanation — exceptional for keeping safety intact through refactors.
- **Honest test doubles** (`test_mocks.rs`): update state only on success, fail cleanly, expose
  `Arc<CsMutex>` for re-arming mid-test.
- **Ambitious Clippy config**: `unwrap`/`expect`/`panic` denies are genuinely effective (verified: 0
  in production).

---

## 4. Findings by Severity

### 4.1 CRITICAL
None.

### 4.2 HIGH (10)

| # | File | Finding | Fix |
|---|---|---|---|
| H-1 | `tasks.rs:1140-1143` | USB/UART write failures discarded with `let _ =`; a message already dequeued is lost forever. Reads have error counters with emergency escalation; writes have nothing | Saturating per-channel counter + `log::warn!`; expose in STATUS |
| H-2 | `roaster_control.rs:22-88` | 3,229-line god-module: `update_control` (:542-1051, ~500 lines) centralizes timeout, comms-idle, charge detection, RoR gating, stale-hold, PID throttle, fan selector, and probe-stuck — the brain was never decomposed | Extract `OrchestrationPolicy`/`RoastStateMachine`; target <1,500 lines |
| H-3 | `roaster_control.rs:145` | Broken encapsulation: public `status_mut()` + 8 controller accessors + public container fields; 10 call-sites mutate safety state from the task layer (`tasks.rs:572-593` writes the watchdog and calls `emergency_shutdown` directly) | Typed methods (`record_watchdog_failure`), private `status_mut()`, privatized accessors |
| H-4 | `tasks.rs:233` | `drain_commands` (orchestrator) embeds TC4 wire formatting (STATUS/READ CSV), `ERR rate_limited…` strings, and reimplements `send_handler_error`, which `ArtisanFormatter::format_err` already provides | Move response emission into `process_artisan_command`; `drain_commands` should only drain → dispatch → log |
| H-5 | `roaster_control.rs:86-87` | `dump_pending: Deque<String<256>, 257>` ≈ **66 KB static** (16% SRAM) for a dump that duplicates the ring buffer (33 KB); real rows are 33-40 B in 264 B slots | Streaming dump: `Option<String<8192>>` + cursor; or `String<96>` rows × 230 ≈ 22 KB |
| H-6 | `conversion.rs:396-399` + `tasks.rs:986,1078` | The 210 ms MAX31856 wait is pure serialized sleep: nothing overlaps (drain, control, WDT, telemetry run afterward); tick = 210 + 100 + overhead ≈ 330 ms | (a) Start conversion at tick start and wait only for the remainder; (b) continuous CMODE=1 with bench validation; tick → ~100-150 ms |
| H-7 | `tests` — `parser.rs:712` | **Tautology**: `assert!(matches!(result, Ok(_expected_command)))` — `_expected_command` is an unused fresh binding; 37 commands only prove “it parses,” never the payload | `assert_eq!(result, Ok(expected))` |
| H-8 | `critical_path_tests.rs:549-568` | Vacuous test: `if status.ssr_hardware_status != Available { assert_eq!(ssr_output, 0.0) }` with no `else`; the default stub is `Available`, so the assert **never runs** | Set `MockSsr.set_status(Error)` and assert unconditionally (as T6 does in `safety_injection_midroast_tests.rs:303`) |
| H-9 | `sensor_conversion.rs:9` | The strongest numeric tests (19-bit two's-complement math) run only with `--features regression`, **excluded from the standard `--features test` gate**; “631 tests pass” oversells it | Change the cfg to `feature = "test"` or add `regression` to the canonical gate |
| H-10 | `max31856.rs`, `shared_spi.rs`, `ledc_bus.rs`, `ssr.rs` | Hardware-only paths have no host coverage: SPI/fault register decode and duty mapping have zero host tests; `ssr_stub.rs` (the host production double) has 0 tests | Extract pure logic (decode, duty math) into host-tested functions; unit-test `percentage_to_ledc_duty` on x86_64 |

### 4.3 MEDIUM (26, summarized by area)

**Protocol/parser:**
- M-P1 `parser.rs:11-29,575` — profile FIFO (4 slots) fills **before** the multiplexer gate and
  enqueue; a discarded or inactive-transport PROFILE installs a stale profile in the next session
  (the only shared state without a gate).
- M-P2 `multiplexer.rs:79-88` — boot hijack: the first syntactically valid command from any cable
  claims the session (UART noise `READ\r` beats USB before Artisan connects).
- M-P3 `transport_tasks.rs:270-275` — valid commands on the inactive transport spam
  `ERR command_ignored_inactive_channel` to the active host (16-entry channel, drained 4 per 5 ms).
- M-P4 `usb_cdc/tasks.rs:44-59`, `uart/tasks.rs:156-178` — divergent legacy helpers: USB discards
  everything except the first command in the buffer; both re-activate the channel on parse error
  (regression P8 already fixed in the production path).
- M-P5 `main.rs:190` — USB CDC init failure is ignored and logged as “USB CDC initialized.”

**Architecture:**
- M-A1 `tasks.rs` — 1,921-line module mixes orchestration, I/O glue, safety escalation, telemetry,
  and instrumentation (functions fine, module wrong).
- M-A2 `service_container.rs:149` — real DI at construction, cosmetic at runtime: global singleton +
  public fields + channels as statics; tests serialized with a global `TEST_LOCK`.
- M-A3 `controllers/sensor.rs:22` — partial control→hardware boundary: actuators use traits, sensors
  use concrete types (`SensorConversionHub`, `SensorFault` in the public API).
- M-A4 `dispatch.rs:27` — three non-uniform handler interfaces (`RoasterCommandHandler`,
  `ManualCommandPolicy`, `SafetyPolicy`) with duplicated routing in
  `RoasterControl::process_command`.
- M-A5 `abstractions.rs:140-154` — `OutputController` is a no-op stub (`process_status` returns
  `Ok(())`); `OutputFormatter` (`traits.rs:13`) has **zero implementors**; `CsvFormatter` and
  `TimeFormatter` are dead (tests/reexports only).
- M-A6 ~705 public API items, ~30% documented; wildcard re-exports in 4 `mod.rs` files;
  `missing_docs` would fail on 300+.
- M-A7 `constants.rs:197` — dead variants: `RoasterState::Cooling`, `::Fault`, `::EmergencyStop`
  (0 references); 3 overlapping “failure-ish” variants.
- M-A8 `handlers/artisan.rs:164` — commit asymmetry: `SetFanManual` applies `apply_to_status` in
  the handler; `SetHeaterManual` does not (violates its own “single writer” contract); fan mutates
  status twice.

**Tests:**
- M-T1 `critical_path_tests.rs:127,172,430,1010` — OR asserts (`is_emergency_active() ||
  fault_condition`): any trip masks the specific backstop under test.
- M-T2 `critical_path_tests.rs:508-542` — missing the just-below negative:
  `OVERTEMP_THRESHOLD - 1.0` must NOT trigger.
- M-T3 `watchdog.rs:107,396` — feed's `bean_temp` is `_` (unused), but the test is named
  `feed_accepts_varying_temperatures` — a temperature-gating promise that does not exist.
- M-T4 `roast_resilience_tests.rs:92,99,12,77` — tests whose names oversell: they only parse
  strings, never transition state.
- M-T5 `critical_path_tests.rs:54,76` + `roaster_control.rs:2081` — reset tests never establish
  the precondition (never set `charge_detected=true`); `accessor_methods_return_references` has 0
  asserts.
- M-T6 `control_loop_integration.rs:268-304` — status snapshot before the final tick: heater and
  fan asserted at different instants.
- M-T7 `parser.rs:671-714` — `STAT`, `OT2 150`, lowercase, `PID;ON/OFF`, etc. lack dedicated
  semantic tests (a direct consequence of H-7).

**Performance:**
- M-R1 `traceability.rs:174-193` — TRACE formats `String<256>` + `write!` (soft-float for f32)
  **and discards** the result in release (`let _ = event`) every tick.
- M-R2 `tasks.rs:868-883` — #DUMP drain: up to 5 async-mutex acquisitions per tick, at least 1 per
  tick even when idle, plus a 264 B `row.clone()`.
- M-R3 `tasks.rs:404-414` — `debug!` with `.await`s in its arguments: 2 extra lock acquisitions per
  tick in instrumentation builds (distorts the timing it measures).
- M-R4 `tasks.rs:1163-1169` — `append_crlf` allocates a `Vec<u8, 1024>` per message for a payload of
  ≤258 B (up to 4 KB of stack churn per tick).
- M-R5 `constants.rs:775-822` — `SystemStatus` (~120-220 B, `Copy`) copied 3-4× per tick; the
  split `CoreRoastStatus`/`InstrumentationSnapshot` is annotated as a TODO in the struct itself.
- M-R6 Output: no drop counter for the output channel (`try_send` discarded at ~20 call-sites); the
  `ERR` lines that report drops can themselves be dropped under saturation.

**Miscellaneous:**
- M-X1 `roaster_control.rs` — SILENT: `get_status()` returns a 45-field struct by value (58
  call-sites) while `status_mut()` returns `&mut`; the deprecated `with_roaster` shim has no callers.
- M-X2 `uart/tasks.rs:62,71` — `ERR rate_limited` is unreachable dead code:
  `MAX_COMMANDS_PER_TICK == ARTISAN_CMD_CHANNEL_SIZE == 16`, so the channel cannot fill to 17.

### 4.4 LOW (22, selected)

- L-1 `constants.rs:238` — inverted dependency: config imports from application
  (`MAX_COMMANDS_PER_TICK = ARTISAN_CMD_CHANNEL_SIZE`).
- L-2 `temperature.rs:52` — timestamps truncated with `as_millis() as u32`: wrap after 49.7 days
  (harmless for roasts; document or use u64).
- L-3 `fan.rs:56` — `libm::floorf` soft-float per fan write on the hot path; inconsistent with SSR,
  which uses an `as u32` cast.
- L-4 `parser.rs:130-144` — `FILT` coerces silence to 0 and accepts any u8 (breaks the parser's
  “reject with noise” convention).
- L-5 `parser.rs:254-271` vs `367-400` — `PIDGAIN` and `PID;T` reject negatives at different layers
  with different ERR tokens.
- L-6 `watchdog.rs:263` — `is_alive()` treats `NEVER_FED` as alive (sentinel).
- L-7 `roaster_control.rs:3083-3123` — `FlakyFan` duplicates `MockFan`'s failure seam (2 doubles for
  the same concept).
- L-8 `constants.rs:96` — 15 ms margin not earned in the 210 ms wait (datasheet: 195 ms worst case).
- L-9 `fan.rs` / `ssr.rs` — 4 SPI transactions in a critical section per tick with a fixed 160-cycle
  delay (correct; ~64 cycles recoverable).
- L-10 `logging/` — `warn!`/`error!` block on the same physical wire as the protocol (UART is
  Printf); add a one-shot warning discipline on hot paths.
- L-11 Naming: duplicated `artisan` module (handlers vs output); `abstractions.rs` mixes error +
  trait + stub; 2 duplicated IIR derivatives (`sensor.rs:230-268` vs `325-341`).
- L-12 Constants scattered across 4 modules with near-duplicates (`REPORT_BUFFER_SIZE` 64 vs
  `RESPONSE_BUFFER_SIZE` 512 vs `TRACE_EVENT_MAX_LEN`).

---

## 5. Concurrency and Async — Verified Verdict

Direct manual autopsy (grep + reading):

- **No lock-await deadlock**: `with_roaster_async` (`service_container.rs:211`) takes
  `.lock().await`, but the closure is **synchronous** — there is no await while holding the mutex.
  UART/USB mutexes (`driver.rs:137,153,187,200`) are short driver broadcasts.
- **Memory ordering**: `SeqCst` in watchdog and lock-depth (conservative, correct); `Relaxed` in
  `ledc_guard`/`queue_metrics`/`traceability` — all counters/metrics where it is appropriate.
- **Watchdog feed**: from the control loop once per tick (330 ms) against a 1,000 ms soft timeout
  and 2,206 ms HW timeout; loop stalls are exposed to the HW WDT (no feed from another task that
  could mask a hang).
- **Safety priority**: `EmergencyStop` and `ArtisanEmergencyStop` live in `handlers/safety.rs:84-92`
  with a latch (`source: Some("emergency_shutdown")`) and bypass the command-channel rate limit.
- **Send/Sync**: `unsafe` verified — PAC register access with correct WDT unlock/relock;
  `static_cell` for init; no mutable `static mut`.

**Conclusion: solid concurrency design.** Minor notes: silent channel drops (M-R6), `is_alive()`
sentinel (L-6), global `TEST_LOCK` serializing tests (M-A2).

## 6. Physical Safety — Verified Verdict

| Layer | Status |
|---|---|
| 260 °C thermal cutoff (`sensor.rs:216`) | ✅ Correct: `>=`, per-channel fault guard, latch |
| Stale-temperature hold (`roaster_control.rs:835-852`) | ✅ PID holds, does not integrate stale data |
| 20 % fan-floor interlock (`roaster_control.rs:365,944`) | ✅ Applied in policy and emergency |
| Fan-fail = unsafe (`roaster_control.rs:1365`) | ✅ Emergency if the fan does not reach 100 % after retries |
| SSR: OFF on errors, 14-bit duty clamp | ✅ Pre-guarded |
| RWDT HW: 2,206 ms, correct unlock/relock | ✅ 6.6× margin |
| Command limits | ✅ Clamped setpoints; NaN/Inf rejected in the parser |
| Startup: heater OFF by default | ✅ (initialization starts with no active state) |

**No path was found that lets a wrong or malicious command set the heater to 100 % without passing
through limits.**

---

## 7. Prioritized Remediation Plan

### Phase 1 — Fast, low risk (1-2 days)
1. **[H-7] Tautological proptest**: `matches!` → `assert_eq!` in `parser.rs:712` — unlocks real
   semantics for 37 commands.
2. **[H-8] Vacuous test** `ssr_not_detected_forces_zero_output_in_manual_mode`: set stub state and
   assert unconditionally.
3. **[M-T3/L-6] Watchdog**: rename the test or implement temperature gating; decide the
   `NEVER_FED` sentinel.
4. **[M-R1] TRACE**: `#[cfg]`-gate the body of the 5 `traceability.rs` functions — zero risk,
   removes ~800 B of churn per tick.
5. **[M-R4] `append_crlf`**: `Vec<u8, 300>` or double-write in the drivers.
6. **[M-X2] Dead `ERR rate_limited`**: lower `MAX_COMMANDS_PER_TICK` or remove the branch.
7. **[L-4/L-5] FILT + PIDGAIN**: parser validation consistent with the rest.
8. **[M-A5] Dead code**: remove the `OutputController` stub, `OutputFormatter` (0 implementors),
   dead `CsvFormatter`/`TimeFormatter`, the `with_roaster` shim, and dead `RoasterState` variants
   (M-A7).
9. **[M-T2] Thermal negative**: test that `OVERTEMP_THRESHOLD - 1.0` does not trigger.

### Phase 2 — Medium (3-5 days)
10. **[H-1/M-R6] Output visibility**: saturating counters for write failures and channel drops,
    exposed in STATUS; rate-limited `warn!`.
11. **[M-P5] USB init**: boot failure → halt path (`run_init_or_panic`), never log “initialized” on
    failure.
12. **[H-5] Streaming dump**: remove the 66 KB deque (or at least use `String<96>` rows); frees
    ~44-66 KB of SRAM.
13. **[M-A6] API shrink + docs**: `#![warn(missing_docs)]` + `#![warn(unreachable_pub)]`, document
    ~40 entry points, prune wildcard re-exports.
14. **[M-T1/M-T4/M-T5/M-T6] Harden test asserts**: independent backstops, real preconditions,
    post-tick snapshot.
15. **[H-9] Numeric test gate**: add `regression` to the canonical quality command in `CONTEXT.md`.
16. **[M-P4] Legacy USB/UART helpers**: align with the production path or remove; re-point
    integration tests.
17. **[M-R3] `debug!` with awaits**: snapshot temperatures before the macro.

### Phase 3 — Structural (1-2 weeks, with the 735-test suite as a safety net)
18. **[H-2] Extract `OrchestrationPolicy`/`RoastStateMachine`** from `update_control` and the
    Artisan handlers; `RoasterControl` → thin facade (<1,500 lines). The regression suite protects
    the extraction — byte-identical semantics.
19. **[H-4/M-A4] Unify handler interfaces** into one router inside `CommandDispatcher`; command
    responses inside the process flow.
20. **[H-3] Encapsulation**: private `status_mut()`, typed methods for watchdog/latency/guard,
    privatized container fields with accessors.
21. **[M-A3] Sensor boundary**: `SensorHub`/`ThermometerPair` trait in `control/traits.rs`, hardware
    errors mapped behind the trait.
22. **[M-P1/M-P2/M-P3] Multiplexer**: gate the profile FIFO, active USB at boot or a grace period,
    rate-limited inactive-channel notification.
23. **[H-6] Tick**: early conversion start + overlap with the timer (or continuous mode with bench
    validation) — the only change that makes “100 ms” real.
24. **[M-A1/M-R5] Split `SystemStatus`** (already annotated in `constants.rs:772-774`) + re-home
    `tasks.rs`.

---

## 8. Final Verdict

**LibreRoaster is embedded code of above-average quality for the industry**: impeccable gates (fmt
+ clippy 0 warnings with strict denies), zero `unwrap` in production, zero TODO/FIXME, real defense
in depth in the safety layers, a parser hardened against hostile input with fuzzing, an exceptional
bug-comment culture, and a 735-test suite (formerly 631) that has caught real bugs.

The debt is **structural, not behavioral**: a central god-module that concentrates all control
policy, a public API 3× larger than necessary with 30% documentation, two observability gaps in the
output channel, 66 KB of static RAM for a dump, a serialized 210 ms wait that defines the real tick
(~330 ms), and some tests that oversell what they verify. None of the findings can cause
over-temperature, an uncontrolled heater, or hardware loss — the emergency and watchdog layers are
the final backstop and are verified.
