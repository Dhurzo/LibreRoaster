# Testing — LibreRoaster

**Last updated:** 2026-08-12

This document describes every test layer in the project, what each covers, its current status, and how to run it.

---

## Test Execution Context

LibreRoaster targets an ESP32-C3 (RISC-V, `no_std`) but the majority of tests run on the **host** (`x86_64-unknown-linux-gnu`) by stubbing hardware. The `test` feature enables the host-side Embassy time driver and replaces real hardware with mocks.

```bash
# Full host test suite
cargo test --target x86_64-unknown-linux-gnu --features test

# Embedded build verification (no tests, compiles only)
cargo build --release --target riscv32imc-unknown-none-elf --features embedded

# Hardware-in-the-loop (requires physical ESP32-C3)
python3 scripts/serial_integration_test.py --port /dev/ttyUSB0
```

**Current status:** host-side test suite is fully green — **693 tests pass** (`cargo test --target x86_64-unknown-linux-gnu --features test --lib --tests --no-fail-fast`), 0 failures (2026-08-12, tras la auditoría de compatibilidad Artisan A-TC4 + verificación light-roast A-TC4-D). The suite holds **487 unit + 255 integration test functions**; the difference vs. the 693 passing in this gate are the regression/feature-gated tests (see §2.7 and §6). Includes the safety hunt suites (`safety_repro_tests.rs`, `safety_injection_midroast_tests.rs`, `safety_invariant_harness.rs` — 1000 roasts aleatorios), the in-crate transport byte-drip tests (T-B1..T-B4), the Artisan golden-transcript replay suite (`artisan_transcript_replay.rs`), the pipeline soak (`pipeline_soak.rs`), the light-roast verification suite (A-TC4-D, see §2.2), and the extended proptests (parser/PID/actuador/RoastCurve/formatters).

> Note: the previous edition of this document hard-coded a count of "218 unit + 133 integration" plus "3 pre-existing doctest failures in `src/memory/strategy.rs`". The count drifted out of date and the "pre-existing failures" did not exist on `develop`. Both claims have been removed in favour of running the suite.

---

## 1. Unit Tests (`src/` inline `#[cfg(test)]` modules)

These live inside the library crate, co-located with the code they test. They verify individual functions, data structures, and module-level invariants in isolation.

| Module | Tests | Focus | Status |
|--------|-------|-------|--------|
| `src/input/parser.rs` | 112 | Artisan+ TC4 command parsing: CHAN, UNITS, FILT, OT1, OT2, UP, DOWN, STOP, READ, STATUS, PID subcommands, PROFILES, error handling, case insensitivity | ✅ All pass |
| `src/control/roaster_control.rs` | 61 | RoasterControl: command handling, safety transitions, charge detection, emergency latch, preheat | ✅ All pass |
| `src/output/artisan.rs` | 30 | TC4/Artisan response formatting: READ lines, STATUS lines, error responses, CSV output | ✅ All pass |
| `src/control/policies.rs` | 26 | Control policies: manual/PID arbitration, actuator guard conditions, heater/fan interlock | ✅ All pass |
| `src/config/constants.rs` | 25 | Configuration constants: pin assignments, timing values, protocol constants, feature-gate validation | ✅ All pass |
| `src/application/tasks.rs` | 25 | Task-level logic: control loop stages, command draining, rate limiting, telemetry emission | ✅ All pass |
| `src/control/controllers/sensor.rs` | 29 | SensorController: sampling, EMA filtering, fault debounce, rate-of-rise (two-tier hard/soft bands, A-TC4-D), over-temperature guard | ✅ All pass |
| `src/hardware/sensors/simulated.rs` | 18 | Simulated temperature curves: interpolation, waypoints, curve presets, noise, bounds | ✅ All pass |
| `src/safety/watchdog.rs` | 16 | Watchdog: software feeding, timeout accounting, reason tokens, hardware RTC watchdog | ✅ All pass |
| `src/control/pid.rs` | 15 | PID arithmetic: proportional/integral/derivative terms, integrator clamping, saturation, anti-windup | ✅ All pass |
| `src/application/app_builder.rs` | 13 | AppBuilder: peripheral wiring, service container construction, task spawn verification | ✅ All pass |
| `src/error/app_error.rs` | 10 | Error types: Display impl, error-source propagation, conversion traits | ✅ All pass |
| `src/application/stage_instrumentation.rs` | 10 | Stage reporter: stage sequence validation, timing capture | ✅ All pass |
| `src/control/handlers/*.rs` | 26 | Command handlers (safety 9, artisan 9, temperature 5, system 3): dispatch, manual actuation, PID, regression | ✅ All pass |
| `src/logging/traceability.rs` | 8 | TRACE event formatting: queue depth, stage names, guard state, watchdog state serialization | ✅ All pass |
| `src/logging/roast_logger.rs` | 7 | Roast logger: start/stop/dump cycle, CSV event logging | ✅ All pass |
| `src/hardware/heat_presence.rs` | 6 | Heat-source detection state machine | ✅ All pass |
| `src/hardware/transport_tasks.rs` | 4 | Transport event queue: byte-drip accumulation (T-B1), end-to-end dripped command (T-B2), overflow flush + `ERR buffer_overflow` (T-B3), two-transport byte-level interleave isolation (T-B4) | ✅ All pass |
| `src/input/multiplexer.rs` | 6 | Output channel routing: active channel selection, USB vs UART switching | ✅ All pass |
| `src/output/formatters/*.rs` | 16 | CSV (6), time (5), ROR (5): number normalization, NaN/Infinity handling, time formatting | ✅ All pass |
| `src/hardware/*.rs` (ssr, init, max31856, fan, mod) | 8 | SSR PWM, hardware init validation, MAX31856 register math, fan duty mapping | ✅ All pass |
| `src/memory/constants.rs`, `src/host_time_driver.rs` | 2 | Memory/allocation constants; host Embassy time driver | ✅ All pass |

**Totals:** 487 unit tests across 32 files in `src/`.

---

## 2. Integration Tests (`tests/`)

Host-side integration tests that wire multiple modules together with mocked hardware. They verify end-to-end flows, state machine transitions, protocol correctness, and error handling.

All integration tests live as top-level files directly in `tests/` with mocked hardware. They wire multiple modules together to verify end-to-end flows, state machine transitions, protocol correctness, and error handling.

| Category | Tests | Status |
|----------|-------|--------|
| All integration test files | 255 | ✅ All pass (693 pass in the default `--features test` gate; 33 more are regression-gated, §2.7) |

### 2.1 Protocol & Command Handling

| File | Tests | Focus | Status |
|------|-------|-------|--------|
| `tests/artisan_integration_test.rs` | 11 | Complete Artisan+ protocol flow: parse → dispatch → format → response. Tests READ, OT1, IO3, START, STOP, CHAN, UNITS responses | ✅ All pass |
| `tests/artisan_transcript_replay.rs` | 4 | Artisan golden-transcript replay (Audit A-TC4): connect handshake, software-PID slider session, firmware-PID session and light-roast slider session (A-TC4-D) replayed byte-by-byte through the production pipeline. Pins the wire contract Artisan depends on: `#1200`/`#OK` handshake acks, 5/8-field TC4 READ, 20-field STATUS, and the absence of the deprecated 4-field READ | ✅ All pass |
| `tests/command_errors.rs` | 5 | Error propagation through command pipeline: invalid commands, parse errors, service container error mapping | ✅ All pass |
| `tests/command_idempotence.rs` | 4 | Idempotency of start/stop: double-start, double-stop, stop-without-start, state consistency after repeated calls | ✅ All pass |
| `tests/read_command_usb_test.rs` | 15 | READ command over USB CDC path: TC4 format (5-value, 8-value), Celsius/Fahrenheit, NaN handling, field ordering | ✅ All pass |

### 2.2 Roast Simulation

| File | Tests | Focus | Status |
|------|-------|-------|--------|
| `tests/artisan_roast_simulation.rs` | 8 | Full-host roast simulation: handshake → preheat → profile load → charge detection → active roast → PID control → stop → cooldown. Uses simulated sensor curves with full state machine | ✅ All pass |
| `tests/critical_path_tests.rs` | 44 | End-to-end critical flows: roast lifecycle, emergency stops, safety interlocks, protocol round-trips | ✅ All pass |
| `tests/control_loop_integration.rs` | 16 | Control loop integration: stage ordering, command draining, telemetry emission under load | ✅ All pass |
| `tests/roast_resilience_tests.rs` | 25 | Edge-case resilience: no-profile fallback, missing charge detection, invalid commands during roast, rapid start-stop cycles, degenerate PROFILE/FANPROFILE shapes (unsorted/duplicated timestamps) at the control layer | ✅ All pass |
| `tests/full_roast_verification.rs` | 18 | Deterministic L1 full-roast simulation (real PID/safety math, synthetic 310 ms ticks): preheat, charge + `#CHARGE`, profile/fan-profile following, RoR through first crack, all 6 safety backstops, STOP/cooldown release, two consecutive roasts, and the **light-roast verification suite (A-TC4-D)**: software-PID and firmware-PID light-roast full flows (charge dip 200→95 °C, development to ~203 °C) plus 5 boundary tests — manual RoR-guard disarm, turnaround spike tolerance, hard-band runaway latch, sustained soft-band latch, slow-finish probe-stuck margin | ✅ All pass |

### 2.3 Safety & Fault Injection

| File | Tests | Focus | Status |
|------|-------|-------|--------|
| `tests/fault_injection_scenarios.rs` | 4 | Overtemp/emergency-latch/STOP-recovery fault injection (requires `--features regression`). Note: watchdog-timeout and mid-roast hardware faults live in `tests/safety_injection_midroast_tests.rs` (see below) | ✅ All pass |
| `tests/safety_injection_midroast_tests.rs` | 6 | Mid-roast fault injection (requires `--features test`): heater-write failure (Bug B escalation), fan-write failure, sensor disconnect (F4.11 debounce → NaN → latched emergency), **software watchdog timeout** (watchdog.rs:78-81, formerly untested), interleaved USB/UART routing, SSR-not-available gating | ✅ All pass |
| `tests/error_integration_tests.rs` | 5 | Cross-cutting error integration: error types through dispatch, safety handler error mapping, error recovery strategies | ✅ All pass |

### 2.4 Serial Communication

| File | Tests | Focus | Status |
|------|-------|-------|--------|
| `tests/mock_uart.rs` | 7 | Mock UART driver: send/receive, buffer management, concurrent access from multiple tasks | ✅ All pass |
| `tests/mock_uart_integration.rs` | 3 | UART integration with command processing: command parsing from UART bytes, response routing back through UART | ✅ All pass |
| `tests/mock_usb_driver.rs` | 12 | Mock USB CDC driver: read/write endpoints, buffer management, connection state simulation, concurrent access | ✅ All pass |
| `tests/multiplexer_tests.rs` | 14 | Output multiplexer: active channel selection, concurrent USB+UART output, channel switching during active output, timeout behavior | ✅ All pass |
| `tests/command_multiplexer_concurrency.rs` | 1 | Multiplexer command routing: concurrent command submission from multiple channels, ordering guarantees | ✅ All pass |

### 2.5 Sensor & Actuator Hardware

| File | Tests | Focus | Status |
|------|-------|-------|--------|
| `tests/sensor_conversion.rs` | 16 | Sensor temperature conversion: raw-to-celsius, fault register parsing, edge-case temperatures (fixture rows require `--features regression`) | ✅ All pass |
| `tests/concurrent_sensor_test.rs` | 1 | Concurrent sensor access: async sensor read under concurrent command processing | ✅ All pass |
| `tests/fan_serialization.rs` | 6 | Fan state serialization: fan speed set/get through status, fan OutputFormatter integration | ✅ All pass |
| `tests/ssr_scheduler.rs` | 3 | SSR scheduler: duty cycle timing, cycle-guard window enforcement | ✅ All pass |

### 2.6 Performance & Concurrency

| File | Tests | Focus | Status |
|------|-------|-------|--------|
| `tests/control_loop_stage.rs` | 2 | Stage reporter sequence validation: correct ordering and transition counting through all control loop stages | ✅ All pass |
| `tests/pipeline_soak.rs` | 1 | Bounded pipeline soak (Audit A-TC4): ~700 mixed commands (valid Artisan traffic, garbage, latch/recovery cycles) through both transport entry points; queues stay bounded, wire stays well-formed, zero `ERR channel_full` drops | ✅ All pass |

### 2.7 Regression

| File | Tests | Focus | Status |
|------|-------|-------|--------|
| `tests/regression_status.rs` | 13 | Regression mode status reporting: formatting STATUS with regression active flag, snapshot fixture replay (requires `--features regression`) | ✅ All pass |

---

## 3. Python-based Test Scripts (`scripts/`)

### 3.1 Hardware-in-the-Loop (HIL)

| Script | Focus | Status |
|--------|-------|--------|
| `scripts/serial_integration_test.py` | Full roast session over serial to a real ESP32-C3: handshake, temperature polling, PID control, manual actuation, STATUS diagnostics, STOP. Requires `--features "embedded,simulated-sensors"` | ⚠️ Run manually on hardware |
| `tests/hardware/artisan_roast_hil.py` | Artisan HIL simulation: drive a full roast through serial with temperature verification | ⚠️ Run manually on hardware |
| `tests/hardware/read_command_hil.py` | READ command HIL test: verify READ response fields and formatting on real hardware | ⚠️ Run manually on hardware |
| `tests/hardware/hardware_test_runner.py` | Test runner framework for hardware tests: scenario loading, threshold validation, report generation | ⚠️ Run manually on hardware |
| `tests/hardware/validation_runner.py` | Validation runner: executes all HIL scenarios and produces a summary report against `scenario_manifest.json` | ⚠️ Run manually on hardware |

### 3.2 Traceability & Quality

| Script | Focus | Status |
|--------|-------|--------|
| `scripts/test_traceability_matrix.py` | Unit tests for the traceability matrix system: trace line parsing, summary aggregation, queue depth formatting | ✅ Pass |
| `scripts/quality-baseline.sh` | Quality gate runner: fmt → clippy → test → embedded build | N/A (shell script) |
| `scripts/run-regression-checks.sh` | Regression test runner: runs fault injection scenarios with regression feature | N/A (shell script) |

---

## 4. Hardware Mock Infrastructure

| File | Purpose |
|------|---------|
| `src/common/mod.rs` | In-source call-tracking stubs: `StubHeater`, `StubFan`, `StubThermometer` with operation history |
| `src/hardware/test_mocks.rs` | In-source error-injection mocks: `MockThermometer`, `MockSsr`, `MockFan` |
| `tests/common/mod.rs` | Shared test utilities: `build_test_control()`, `StubHeater`, `StubFan` for constructing a RoasterControl with mocked actuators |

---

## 5. Quality Fixtures

| File | Purpose |
|------|---------|
| `tests/quality/fixtures/clippy-tier1-fail.jsonl` | Clippy violation fixture for quality tooling tests |
| `tests/quality/fixtures/clippy-mixed-failures.jsonl` | Mixed-severity clippy violation fixture |
| `tests/hardware/scenario_manifest.json` | HIL scenario manifest: defines test scenarios with thresholds |
| `tests/hardware/hardware_thresholds.json` | Temperature/actuator thresholds for hardware test validation |

---

## 6. Known Test Gaps

| Area | Gap | Impact |
|------|-----|--------|
| E2E serial over real hardware | No automated CI for HIL tests (requires physical ESP32-C3). A desktop-Artisan HIL session checklist is staged in `TEST-PLAN.md` (V2) | Firmware changes may break serial protocol without detection until manual test |
| Wire format vs. real Artisan | Partially covered since A-TC4 (2026-08-12): `tests/artisan_transcript_replay.rs` replays byte transcripts of real Artisan sessions and pins the wire contract; the remaining gap is a live desktop-Artisan HIL session (telemetry interleave tolerance) | Low (host-covered); HIL confirmation pending hardware |
| Long-duration stability | Protocol-layer soak covered by `tests/pipeline_soak.rs`; a **thermal** soak (>1 hour with active PID and sensor ticks) is still untested | Memory leaks or timer drift under thermal load may go undetected |
| Real MAX31856 with thermocouple | Mock sensors only; real sensor noise/glitch patterns untested | Sensor fault recovery paths may be exercised only in simulation |
| Concurrent UART + USB conflict | Covered since A-TC4: byte-level interleave of two transports in flight is tested in-crate (`transport_tasks.rs` T-B4); command-level interleave by `safety_injection_midroast_tests.rs` T5; routing policy (first-valid-command-wins) by `multiplexer_tests.rs` | None for framing; routing behaviour is by design |
| Feature-gated tests | 33 tests require `--features regression` (sensor_conversion 16, regression_status 13, fault_injection 4) — covered by the dedicated CI `regression` job | Low: covered in CI |
| Embedded-only tests | ~32 tests require `target_arch = "riscv32"` — never run in CI | USB CDC, SSR monitor, and instrumentation paths untested in automated CI |
| Property-based testing | Proptest exists and is green: `src/input/parser.rs` (hostile bytes/NUL), `src/control/pid.rs`, `src/control/controllers/actuator.rs`, `src/hardware/sensors/simulated.rs`, `src/output/artisan.rs` (hostile SystemStatus). No `#[ignore]` tests exist anywhere in the repo. No cargo-fuzz yet | Edge cases in PID math, sensor conversion, and protocol parsing are now covered; fuzzing over the wire format remains future work |
| Regression-runner unit coverage | `src/safety/regression.rs` has no direct unit tests; its behavior is exercised via `tests/regression_status.rs` and `tests/fault_injection_scenarios.rs` | Low (covered by integration) |
| Flash memory / persistence | No storage layer exists yet | N/A for current milestone |

## 7. CI Integration

| Aspect | Status |
|--------|--------|
| CI platform | GitHub Actions (`.github/workflows/ci.yml`) |
| Trigger | Push to `develop`/`main`, PR to `develop`/`main` |
| Jobs | 7: `fmt`, `clippy`, `embedded-clippy`, `test` (+ doctests), `regression`, `coverage`, `embedded-build` |
| Host test command | `cargo test --target x86_64-unknown-linux-gnu --features test --lib --tests --no-fail-fast` + `cargo test --target x86_64-unknown-linux-gnu --features test --doc` |
| Clippy command | `cargo clippy --locked --all-targets -- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic` (host) plus the same against `--target riscv32imc-unknown-none-elf --features embedded` (embedded job) |
| Embedded build | `cargo build --release --target riscv32imc-unknown-none-elf --features embedded` (plus `embedded,regression` and `embedded,instrumentation` variants) |
| Regression tests | ✅ Run — `cargo test --features "test,regression" --target x86_64-unknown-linux-gnu --no-fail-fast` |
| Code coverage | ✅ Run — `cargo llvm-cov --target x86_64-unknown-linux-gnu --features "test,regression,simulated-sensors" --no-fail-fast --lcov --output-path target/coverage/lcov.info` (Audit A-TC4: regression + simulated-sensors included so the conversion math and the L3 pipeline are instrumented) |
| HIL tests | ❌ Not run (needs physical ESP32-C3) |
| Embedded-only tests | ❌ Not run (needs `riscv32` target) |

---

## 8. Running Tests

```bash
# Fast — unit tests only (no hardware)
cargo test --lib --target x86_64-unknown-linux-gnu --features test

# Full — unit + integration (no hardware)
cargo test --target x86_64-unknown-linux-gnu --features test

# Regression — fault injection scenarios (requires regression feature)
cargo test --features "test,regression" --target x86_64-unknown-linux-gnu --no-fail-fast

# Quality baseline (fmt + clippy + host tests)
scripts/quality-baseline.sh

# Regression checks (fault injection + max31856 fixtures)
scripts/run-regression-checks.sh

# Hardware-in-the-loop (requires ESP32-C3 on /dev/ttyUSB0)
python3 scripts/serial_integration_test.py --port /dev/ttyUSB0

# HIL test runner (flash + capture + validate + report)
python3 tests/hardware/hardware_test_runner.py --port /dev/ttyUSB0
```

---

## 9. Interactive Hardware Verification

When you need to verify the firmware on real hardware without running a full HIL script, you can flash with simulated sensors and send commands interactively via serial.

### 9.1 Setup: flash + monitor

Flash with simulated sensors (no real thermocouples needed) and open a serial monitor:

```bash
# Terminal A: flash without monitor, then open picocom
cargo espflash flash --release --target riscv32imc-unknown-none-elf \
  --features "embedded,simulated-sensors"

picocom /dev/ttyACM0 -b 115200
```

Using `picocom` (or `screen`) instead of `espflash --monitor` lets you send commands from a second terminal without "Device or resource busy" errors.

### 9.2 Why firmware is silent after boot

The firmware boots successfully and displays log messages, but emits **no telemetry** because continuous output is **disabled by default** (`OutputController::continuous_enabled = false` in `src/control/abstractions.rs`). Telemetry is only emitted every ~100ms when continuous output is enabled.

Continuous output is enabled by these commands:
- `START` — begins roast, enables PID and continuous telemetry
- `OT1 <pct>` — manual heater control (0-100)
- `IO3 <pct>` — manual fan control (0-100)
- `UP` / `DOWN` — incremental heater adjustment

It is disabled by:
- `STOP` — emergency stop, disables PID and continuous output

Commands like `READ` and `STATUS` return a **single response** regardless of continuous output state.

### 9.3 Sending commands from a second terminal

With picocom running in Terminal A, use Terminal B to send commands:

```bash
# Single-shot readings (work regardless of continuous output state)
echo "READ"   > /dev/ttyACM0     # TC4-format reading (5 or 8 fields)
echo "STATUS" > /dev/ttyACM0     # 20-field diagnostic line

# Enable continuous output and view simulated curves
echo "SETTARGET 200" > /dev/ttyACM0  # Set PID target to 200°C
echo "START"         > /dev/ttyACM0  # Begin roast, enable continuous telemetry

# Alternative: manual control also enables continuous output
echo "OT1 50"  > /dev/ttyACM0   # Heater at 50%, enables continuous telemetry
echo "IO3 75"  > /dev/ttyACM0   # Fan at 75%, enables continuous telemetry

# Stop
echo "STOP"    > /dev/ttyACM0   # Disable PID and continuous output
```

After `START` or `OT1`, Terminal A (picocom) will show `#`-prefixed telemetry lines (timestamp in s since boot, at the real loop cadence of ~310-330 ms):

```
#120.0,180.5,150.3,3.2,0.0
#120.2,181.0,150.7,3.4,0.0
#120.4,181.4,151.2,3.1,0.0
```

Fields: `#<time_s>,ET,BT,ROR,Gas`.

### 9.4 Command protocol details

| Command | Args | Effect |
|---------|------|--------|
| `READ` | none | Returns single TC4 response (`AMB,ET,BT,...`) |
| `STATUS` / `STAT` | none | Returns 20-field diagnostic line |
| `CHAN;1200` | polling rate | Artisan's channel-map handshake: the rate is recorded in `chan_poll_rate_hz` and acknowledged with `#<rate>`. It does NOT select a transport (USB/UART routing is owned by the command multiplexer). |
| `UNITS;C` / `UNITS;F` | temp scale | Set Celsius or Fahrenheit |
| `SETTARGET 200` | target °C | Set PID target temperature |
| `START` | **no args** | Begin roast, enable PID and continuous output |
| `STOP` | none | Emergency stop, disable PID and output |
| `OT1 75` | 0-100 | Manual heater at given percentage |
| `IO3 75` | 0-100 | Manual fan at given percentage |
| `UP` / `DOWN` | none | Incremental heater adjustment |
| `PREHEAT 180` | target °C | Set preheat target |

**Important:** `START` takes no arguments. To set a target temperature and start, send `SETTARGET 200` first, then `START`. The parser (`src/input/parser.rs`) requires `parts.len() == 1` for the `START` command — sending `START 200` results in `UnknownCommand`.

### 9.5 What to observe

When continuous telemetry is active with simulated sensors, verify:

1. **Temperature evolution** — BT rises through the simulated medium roast curve (25 → 225°C over ~10 min), ET stays higher (25 → 250°C)
2. **No fault conditions** — simulated sensors never fault, so no `ERR` lines from stale temperature or overtemp
3. **PID terms** — if PID is enabled (`START` was sent), `SV` field appears in 8-field `READ` response
4. **Heater output** — SSR remains at 0% unless GPIO1 is externally pulled low (heat source detection pin)

### 9.6 Host-side roast simulation (no hardware)

To see a full roast lifecycle output without hardware:

```bash
# The test has println! diagnostics — use --nocapture to see them
cargo test --test artisan_roast_simulation --features test -- --nocapture
```

This prints each phase: handshake → preheat → profile load → charge → active roast → stop → cooldown, with READ responses at each step.

### 9.7 Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `ERR invalid_value empty_command` | Empty line sent (newline without content) | Normal when pressing Enter — harmless |
| `ERR invalid_value unknown_command` | Command not recognized | Check command syntax (e.g., `START` not `START 200`) |
| `ERR safety_fault <reason>` | An internal safety trap (overtemp, stale sensor, probe-stuck, …) armed the emergency latch | Inspect the reason; recover with `PID;OFF` (or `START`/`PREHEAT`) — the heater is off and the fan at 100 % while latched |
| `ERR probe_stuck_warning` | Manual / software-PID mode: BT flat (≤ 1 °C) for 120 s with the heater on (Audit A-TC4-C) | Informational — the roast keeps running (a slow finish can hold BT flat at low duty). If it persists to 300 s, the detector latches with `ERR safety_fault Probe stuck` |
| No telemetry after `START` | Continuous output was already off before START, or START failed silently | Send `READ` first to verify the device is responsive, then `READ` again to check whether PID is active (8-field vs 5-field response) |
| `cr1_readback_mismatch` on boot | Real MAX31856 connected but not responding | Use `--features "embedded,simulated-sensors"` to skip MAX31856 init |
| `/dev/ttyACM0: Dispositivo o recurso ocupado` | espflash monitor or another terminal holds the port | Kill the existing monitor, use picocom/screen instead, or type commands directly into the espflash monitor window |
