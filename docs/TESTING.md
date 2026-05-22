# Testing — LibreRoaster

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

**Current status:** 218 unit tests + 133 integration tests (351 host-side) pass. 3 pre-existing doctest failures in `src/memory/strategy.rs` (documentation examples out of date with recent refactors). Embedded build compiles clean. An additional 35+ tests require the `regression` feature flag (fault injection, MAX31856 fixture replay) and ~32 are embedded-only (`target_arch = "riscv32"`).

---

## 1. Unit Tests (`src/` inline `#[cfg(test)]` modules)

These live inside the library crate, co-located with the code they test. They verify individual functions, data structures, and module-level invariants in isolation.

| Module | Tests | Focus | Status |
|--------|-------|-------|--------|
| `src/config/constants.rs` | ~5 | Configuration constants: pin assignments, timing values, protocol constants, feature flag validation | ✅ All pass |
| `src/memory/constants.rs` | ~2 | Memory layout and allocation constants | ✅ All pass |
| `src/hardware/fan.rs` | ~3 | Fan PWM control: duty cycle calculation, speed mapping | ✅ All pass |
| `src/hardware/ssr.rs` | ~3 | SSR PWM: duty cycle, heat detection integration | ✅ All pass |
| `src/hardware/init.rs` | ~2 | Hardware initialization sequence validation | ✅ All pass |
| `src/hardware/mod.rs` | ~1 | Embedded-hal error trait compatibility | ✅ All pass |
| `src/input/parser.rs` | ~30 | Artisan+ TC4 command parsing: CHAN, UNITS, FILT, OT1, OT2, UP, DOWN, STOP, READ, STATUS, PID subcommands, PROFILES, error handling, case insensitivity, edge cases | ✅ All pass |
| `src/input/init_state.rs` | ~5 | Boot-time state machine: valid command sequencing during initialization, rejection of premature operational commands | ✅ All pass |
| `src/input/multiplexer.rs` | ~10 | Output channel routing: active channel selection, USB vs UART switching, concurrent read/write isolation | ✅ All pass |
| `src/control/pid.rs` | ~10 | PID controller arithmetic: proportional/integral/derivative terms, integrator clamping, output saturation, anti-windup | ✅ All pass |
| `src/control/handlers/artisan.rs` | ~15 | Artisan manual command policy: heater/fan manual mode evaluation, guard conditions, clamp logic | ✅ All pass |
| `src/control/handlers/temperature.rs` | ~10 | Temperature command handler: PID enable/disable, setpoint management, cycle time, output limits | ✅ All pass |
| `src/control/handlers/safety.rs` | ~5 | Safety handler: emergency stop handling, fault condition propagation | ✅ All pass |
| `src/control/handlers/system.rs` | ~3 | System command handler: status reporting, regression commands | ✅ All pass |
| `src/output/artisan.rs` | ~15 | TC4/Artisan response formatting: READ lines, STATUS lines, error responses, CSV output | ✅ All pass |
| `src/output/formatters/csv.rs` | ~5 | CSV field formatting: number normalization, NaN/Infinity handling | ✅ All pass |
| `src/output/formatters/ror.rs` | ~7 | Rate-of-rise calculation: derivative filtering, history management, reset semantics | ✅ All pass |
| `src/output/formatters/time.rs` | ~4 | Time formatting: seconds, milliseconds, large values, carry-over | ✅ All pass |
| `src/application/tasks.rs` | ~5 | Task-level instrumentation: stage tracking, guard state transitions | ✅ All pass |
| `src/application/stage_instrumentation.rs` | ~5 | Stage reporter: stage sequence validation, timing capture | ✅ All pass |
| `src/application/service_container.rs` | ~5 | ServiceContainer: initialization, async sensor read error propagation | ✅ All pass |
| `src/logging/traceability.rs` | ~10 | TRACE event formatting: queue depth, stage names, guard state, watchdog state serialization | ✅ All pass |
| `src/logging/roast_logger.rs` | ~3 | Roast logger: start/stop/dump cycle, CSV event logging | ✅ All pass |
| `src/error/app_error.rs` | ~5 | Error types: Display impl, error source propagation, conversion traits | ✅ All pass |
| `src/hardware/max31856.rs` | ~10 | MAX31856 register math: raw temperature conversion (two's complement), fault register decoding, CRC | ✅ All pass |
| `src/hardware/sensors/simulated.rs` | ~15 | Simulated temperature curves: interpolation, waypoints, curve presets, monotonicity, overtemp bounds | ✅ All pass |

---

## 2. Integration Tests (`tests/`)

Host-side integration tests that wire multiple modules together with mocked hardware. They verify end-to-end flows, state machine transitions, protocol correctness, and error handling.

Integration test files in subdirectories (`tests/safety/`, `tests/timing/`, `tests/e2e/`, `tests/concurrency/`, `tests/hardware/`) are included into the top-level test binaries via `#[path]` mod declarations. Their tests are counted within the top-level file totals below.

| Category | Tests | Status |
|----------|-------|--------|
| All integration test files | 133 | ✅ All pass |

### 2.1 Protocol & Command Handling

| File | Tests | Focus | Status |
|------|-------|-------|--------|
| `tests/artisan_integration_test.rs` | 11 | Complete Artisan+ protocol flow: parse → dispatch → format → response. Tests READ, OT1, IO3, START, STOP, CHAN, UNITS responses | ✅ All pass |
| `tests/command_errors.rs` | 5 | Error propagation through command pipeline: invalid commands, parse errors, service container error mapping | ✅ All pass |
| `tests/command_idempotence.rs` | 4 | Idempotency of start/stop: double-start, double-stop, stop-without-start, state consistency after repeated calls | ✅ All pass |
| `tests/read_command_usb_test.rs` | 27 | READ command over USB CDC path: TC4 format (5-value, 8-value), Celsius/Fahrenheit, NaN handling, field ordering | ✅ All pass |

### 2.2 Roast Simulation

| File | Tests | Focus | Status |
|------|-------|-------|--------|
| `tests/artisan_roast_simulation.rs` | 8 | Full-host roast simulation: handshake → preheat → profile load → charge detection → active roast → PID control → stop → cooldown. Uses simulated sensor curves with full state machine | ✅ All pass |
| `tests/e2e/full_roast_cycle.rs` | 6 | End-to-end roast cycle: Start → Heat → Roast → Stop → Cool, including temperature curve following and state transitions (included via `#[path]` from the main test crate) | ✅ All pass |
| `tests/roast_scenarios/` | — | Phase-specific scenario helpers: `heating_phase.rs`, `roasting_phase.rs`, `cooling_phase.rs` — temperature curve injection and state simulation. These are utility modules (no tests), consumed by e2e/safety/timing tests via `crate::` path. | ✅ All pass |

### 2.3 Safety & Fault Injection

| File | Tests | Focus | Status |
|------|-------|-------|--------|
| `tests/safety/watchdog_integration.rs` | 5 | Watchdog integration: feed success/failure detection, timeout behavior, recovery after watchdog reset (included via `#[path]`) | ✅ All pass |
| `tests/safety/overtemperature_protection.rs` | 7 | Overtemperature shutdown: threshold triggering (260°C), fault condition propagation, heater cutoff verification, recovery via STOP (included via `#[path]`) | ✅ All pass |
| `tests/safety/ledc_guard.rs` | 3 | LEDC guard timeout: SSR cycle guard detection, timeout counting, guard state propagation to telemetry (included via `#[path]`) | ✅ All pass |
| `tests/fault_injection_scenarios.rs` | 14 | Matrix-based fault injection (requires `--features regression`): watchdog failures, guard timeouts, communication errors. Captures SystemStatus and formats STATUS response for each scenario | ✅ All pass |
| `tests/error_integration_tests.rs` | 5 | Cross-cutting error integration: error types through dispatch, safety handler error mapping, error recovery strategies | ✅ All pass |

### 2.4 Serial Communication

| File | Tests | Focus | Status |
|------|-------|-------|--------|
| `tests/mock_uart.rs` | 7 | Mock UART driver: send/receive, buffer management, concurrent access from multiple tasks | ✅ All pass |
| `tests/mock_uart_integration.rs` | 3 | UART integration with command processing: command parsing from UART bytes, response routing back through UART | ✅ All pass |
| `tests/mock_usb_driver.rs` | 12 | Mock USB CDC driver: read/write endpoints, buffer management, connection state simulation, concurrent access | ✅ All pass |
| `tests/transport_flood_test.rs` | 2 | Transport flood resistance: rapid command bursts through USB and UART paths, queue pressure handling, no message loss | ✅ All pass |
| `tests/multiplexer_tests.rs` | 14 | Output multiplexer: active channel selection, concurrent USB+UART output, channel switching during active output, timeout behavior | ✅ All pass |
| `tests/command_multiplexer_concurrency.rs` | 1 | Multiplexer command routing: concurrent command submission from multiple channels, ordering guarantees | ✅ All pass |

### 2.5 Sensor & Actuator Hardware

| File | Tests | Focus | Status |
|------|-------|-------|--------|
| `tests/sensor_conversion.rs` | 3 | Sensor temperature conversion: raw-to-celsius, fault register parsing, edge-case temperatures | ✅ All pass |
| `tests/concurrent_sensor_test.rs` | 1 | Concurrent sensor access: async sensor read under concurrent command processing | ✅ All pass |
| `tests/fan_serialization.rs` | 6 | Fan state serialization: fan speed set/get through status, fan OutputFormatter integration | ✅ All pass |
| `tests/ssr_monitor.rs` | 5 | SSR hardware monitor: heat source detection, availability state machine, fault detection | ✅ All pass |
| `tests/ssr_scheduler.rs` | 14 | SSR scheduler: duty cycle timing, zero-crossing alignment, guard window enforcement | ✅ All pass |
| `tests/roast_resilience_tests.rs` | 8 | Edge-case resilience: no-profile fallback, missing charge detection, invalid commands during roast, rapid start-stop cycles | ✅ All pass |

### 2.6 Performance & Concurrency

| File | Tests | Focus | Status |
|------|-------|-------|--------|
| `tests/timing/control_loop_timing.rs` | 2 | Control loop timing analysis: stage duration budgets (SensorRead ~160ms, ControlUpdate ~10ms), 100ms cycle feasibility validation (included via `#[path]`) | ✅ All pass |
| `tests/concurrency/dual_channel_stress.rs` | 2 | Dual USB+UART concurrency: simultaneous command injection from both channels, queue pressure handling (included via `#[path]`) | ✅ All pass |
| `tests/control_loop_stage.rs` | 2 | Stage reporter sequence validation: correct ordering and transition counting through all control loop stages | ✅ All pass |

### 2.7 Regression

| File | Tests | Focus | Status |
|------|-------|-------|--------|
| `tests/regression_status.rs` | 1 | Regression mode status reporting: formatting STATUS with regression active flag, compatibility with ArtisanFormatter | ✅ All pass |

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
| `tests/fixtures/max31856_sequences.rs` | MAX31856 register read sequences for fixture-based regression testing |
| `tests/hardware/mock_max31856.rs` | Mock MAX31856 SPI sensor for host-side thermocouple simulation (included via `#[path]`) |
| `tests/hardware/mock_ssr.rs` | Mock SSR/heater for host-side actuator testing (included via `#[path]`) |
| `tests/hardware/mock_fan.rs` | Mock fan for host-side actuator testing (included via `#[path]`) |

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
| E2E serial over real hardware | No automated CI for HIL tests (requires physical ESP32-C3) | Firmware changes may break serial protocol without detection until manual test |
| Long-duration stability | No soak test (>1 hour) with active PID | Memory leaks or timer drift may go undetected |
| Real MAX31856 with thermocouple | Mock sensors only; real sensor noise/glitch patterns untested | Sensor fault recovery paths may be exercised only in simulation |
| Concurrent UART + USB conflict | Dual-channel stress tests exist but don't test byte-level interleaving | Rare race conditions in byte-level framing may not be caught |
| Feature-gated tests in CI | ~35 tests require `--features regression` (fault injection, sensor conversion, regression snapshots) — never run in CI | Regression in these paths may go undetected until manual regression run |
| Embedded-only tests | ~32 tests require `target_arch = "riscv32"` — never run in CI | USB CDC, SSR monitor, and instrumentation paths untested in automated CI |
| Property-based testing | No `proptest` or `quickcheck` generators | Edge cases in PID math, sensor conversion, and protocol parsing may be missed |
| Zero tests for safety modules | `src/safety/watchdog.rs`, `src/safety/regression.rs` | Safety watchdog logic and regression detection have no direct unit coverage |
| Flash memory / persistence | No storage layer exists yet | N/A for current milestone |

## 7. CI Integration

| Aspect | Status |
|--------|--------|
| CI platform | GitHub Actions (`.github/workflows/ci.yml`) |
| Trigger | Push to `develop`/`main`, PR to `develop`/`main` |
| Jobs | 4: `fmt`, `clippy`, `test`, `embedded-build` |
| Host test command | `cargo test --features test --lib --tests --no-fail-fast` |
| Embedded build | `cargo build --release --target riscv32imc-unknown-none-elf --features embedded` |
| Regression tests | ❌ Not run (needs `--features regression`) |
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
cargo test --features regression --target x86_64-unknown-linux-gnu

# Quality baseline (fmt + clippy + test + embedded build)
scripts/quality-baseline.sh

# Regression checks (fault injection + max31856 fixtures)
scripts/run-regression-checks.sh

# Hardware-in-the-loop (requires ESP32-C3 on /dev/ttyUSB0)
python3 scripts/serial_integration_test.py --port /dev/ttyUSB0

# HIL test runner (flash + capture + validate + report)
python3 tests/hardware/hardware_test_runner.py --port /dev/ttyUSB0
```
