# LibreRoaster Development Guide

**Last updated:** 2026-10-26

This guide explains how to build, flash, test, and verify LibreRoaster without losing sight of the project’s two-runtime architecture: a real embedded firmware target and a host-side verification target.

## 1. Development model

LibreRoaster is not a generic desktop Rust project.

There are two distinct workflows:

- **embedded workflow**: build firmware for the ESP32-C3 and flash hardware
- **host workflow**: run parser/control/concurrency/integration tests on x86_64

Most documentation drift in this repository has come from treating those workflows as interchangeable. They are not.

## 2. Toolchain prerequisites

### Rust

The project targets Rust **1.88**.

### Target

Add the embedded target:

```bash
rustup target add riscv32imc-unknown-none-elf
```

### Flash tooling

Install `espflash` for device programming:

```bash
cargo install espflash
```

## 3. Cargo feature model

LibreRoaster uses Cargo features to separate embedded behavior from host-only helpers.

### Important features

- **`embedded`** — enables the firmware binary target
- **`std`** — enables standard-library-backed host behavior
- **`instrumentation`** — enables telemetry/instrumentation hooks (pulled in by `test`)
- **`test`** — enables host test support on top of `std` (equivalent to `std` + `instrumentation`)
- **`simulated-sensors`** — simulated thermocouple curves for hardware-free runs on device (also pulled in by `regression`)
- **`simulated-plant`** — F5 simulation build: implies `simulated-sensors` and attaches the closed-loop `ThermalPlant`, so the control loop's heater/fan output drives BT/ET. Use for PID/actuator checks, not for the time-based F1/F3/F4 traces.
- **`simulated-typical-roast`** — F3 simulation build: pulls in `simulated-sensors` and selects the guard-safe charge-dip curve for `START`/`Heating` tests without the RoR latch. Mutually exclusive with `simulated-web-medium-roast`.
- **`simulated-web-medium-roast`** — F4 simulation build: pulls in `simulated-sensors` and selects the 696 s public medium-roast curve (BT from Zenodo 21383046; ET = BT + 20 °C) for real-Artisan runs. Mutually exclusive with `simulated-typical-roast`.
- **`heat-sense`** — opt-in GPIO1 current-sense interpretation for builds with a validated stretched-pulse circuit (default: not interpreted; all other safety layers stay active). Cannot be combined with `simulated-sensors`; the build fails with `compile_error!`.
- **`async-lock-depth-metrics`** — enables async lock instrumentation used by concurrency tests
- **`regression`** — enables regression-specific mock support (pulls in `simulated-sensors`)

| Variant | Command |
|---------|---------|
| Race check | `cargo test --target x86_64-unknown-linux-gnu --features test --lib --tests --no-fail-fast -- --test-threads=1` |
| Code coverage | `cargo llvm-cov --target x86_64-unknown-linux-gnu --features "test,regression,simulated-sensors" --no-fail-fast --lcov --output-path target/coverage/lcov.info` (CI; `scripts/run-coverage.sh` still uses only `--features test`, so the conversion math and the L3 simulated pipeline show uncovered there) |
| Embedded build | `cargo build --release --target riscv32imc-unknown-none-elf --features embedded` (plus `embedded,heat-sense`, `embedded,regression`, `embedded,instrumentation`, `embedded,simulated-sensors`, `embedded,simulated-plant`, `--examples`, and the F3/F4/F5 simulation profiles, mirroring CI) |

CI runs 8 jobs (fmt, clippy, embedded-clippy, test, regression, coverage, embedded-build,
simulation-profiles). `simulation-profiles` builds F3, F4, and F5 and rejects incompatible feature
combinations without a Wokwi token; the weekly `Simulation` workflow (Mondays 06:00 UTC) runs the full Wokwi
manifest when `WOKWI_CLI_TOKEN` is configured. The recorded counts and dates for each battery are in the
verification snapshot at the end of §6.
Clippy runs both the default host `--all-targets` and a `--features test` pass because the `test`
feature gates real code (multiplexer, `usb_cdc` tasks).

### Why `test` matters

Host integration tests rely on the host-side Embassy time driver. That is why `cargo test` alone is not the authoritative command for the project’s integration coverage.

For x86_64 integration-style verification, use the explicit target and feature set.

## 4. Build workflows

### Embedded firmware build

```bash
cargo build --release --target riscv32imc-unknown-none-elf --features embedded
```

Use this when you want the real device artifact.

### Host library build

```bash
cargo check --target x86_64-unknown-linux-gnu --features test
```

Use this when validating test-oriented host code paths.

## 5. Flash workflow

### Standard flash

```bash
cargo espflash flash --release --target riscv32imc-unknown-none-elf --features embedded
```

### Flash and monitor

```bash
cargo espflash flash --release --target riscv32imc-unknown-none-elf --features embedded --monitor
```

### Monitor only

```bash
cargo espflash monitor --port /dev/ttyACM0 --speed 115200
```

Port names vary by platform, but the configuration model is the same.

## 6. Test and verification workflows

### Fast library tests

```bash
cargo test --lib --target x86_64-unknown-linux-gnu --features test
```

> The `--features test` is **required**: without it, host tests fail at link time (`undefined symbol: _embassy_time_now`) because the host Embassy time driver only exists behind the `test` feature.

This is useful for quick inner-loop development, but it does not cover the full host integration surface.

### Host integration baseline

```bash
cargo test --target x86_64-unknown-linux-gnu --features test
```

This is the correct default verification command when documentation or behavior changes touch protocol, control, or task orchestration. See the verification snapshot at the end of this section before quoting counts.

### Regression numeric suite

The strongest numeric tests (`tests/sensor_conversion.rs` — 19-bit two's-complement LSB math, fixture→fault mapping) are gated behind the `regression` feature, so they are NOT part of the default `--features test` run. Run them explicitly:

```bash
cargo test --target x86_64-unknown-linux-gnu --features test --features regression --test sensor_conversion --no-fail-fast
```

### Simulated-sensors pipeline (L3)

Real control-loop ticks over the synthetic `simulated-sensors` temperature curves (wall clock), including the full pipeline test `control_loop_tick_simulated_sensors_full_pipeline`:

```bash
cargo test --target x86_64-unknown-linux-gnu --features test --features simulated-sensors --lib control_loop_tick_simulated
```

### Pinout simulation without hardware (Wokwi)

Builds the firmware and prepares the Wokwi project in `sim/wokwi/` for a visual pin check without hardware. F1 uses `simulated-sensors` (UART, status LED, SSR/fan PWM); F2 uses `heat-sense` plus the custom MAX31856 chip (real SPI and GPIO1 heat-sense); F3 uses `simulated-typical-roast` (F1 plus a charge-dip curve that stays below the RoR guard, for a solid-green `START`/`Heating` test); F4 uses `simulated-web-medium-roast` (F1 plus the 696 s public medium-roast trace for end-to-end Artisan recording); F5 uses `simulated-plant` (closed-loop `ThermalPlant` driven by heater/fan output):

```bash
./scripts/sim-wokwi-prepare.sh
FEATURES=embedded,heat-sense ./scripts/sim-wokwi-prepare.sh
FEATURES=embedded,simulated-typical-roast ./scripts/sim-wokwi-prepare.sh
FEATURES=embedded,simulated-web-medium-roast ./scripts/sim-wokwi-prepare.sh
FEATURES=embedded,simulated-plant ./scripts/sim-wokwi-prepare.sh
```

Open `sim/wokwi/` in VS Code and run `Wokwi: Start Simulator`; the serial monitor at 115200 baud is the Artisan UART. See `SIMULATION.md` for the pin checklist, chip controls, and the experimental esp-emulator status. For automated serial checks with `wokwi-cli`, use the manifest-driven runner:

```bash
./scripts/sim-wokwi-check.sh --list
./scripts/sim-wokwi-check.sh --scenario f1-smoke --scenario f4-short
./scripts/sim-wokwi-check.sh --all
```

The runner builds each feature set once, verifies `sim/wokwi/build-meta.json`, and writes logs/results under `target/wokwi/`. See `SIMULATION.md` §3.4.1 for the scenario matrix and §3.4.2 for the `SIM` command family. To verify a bridge trace outside Artisan, run `./scripts/verify-artisan-trace.py --trace <file> --expect <regex>`. `--build-only` prepares the images without a Wokwi token. To drive the simulation with the real Artisan application, use the RFC2217-to-PTY bridge in `SIMULATION.md` §3.6.

### Artisan wire-contract tests

Golden-transcript replay pins the wire contract against real Artisan session bytes; the soak stress-tests the full pipeline with a random command stream:

```bash
cargo test --target x86_64-unknown-linux-gnu --features test --test artisan_transcript_replay
cargo test --target x86_64-unknown-linux-gnu --features test --test pipeline_soak
```

Run these whenever a change touches the parser, formatters, transport tasks, or command handlers.

### Concurrency instrumentation run

```bash
cargo test --target x86_64-unknown-linux-gnu --features "test,async-lock-depth-metrics" --test concurrent_sensor_test
```

Use this when you need evidence about async lock depth and sensor-read concurrency behavior.

### Targeted integration examples

```bash
cargo test --target x86_64-unknown-linux-gnu --features test --test artisan_integration_test
cargo test --target x86_64-unknown-linux-gnu --features test --test command_multiplexer_concurrency
cargo test --target x86_64-unknown-linux-gnu --features test --test mock_uart_integration
```

### Verification snapshot (last recorded)

This is the single source for volatile test counts. Counts change with features and test additions;
rerun the commands above before citing them as current.

| Battery | Result | Date |
|---|---|---|
| Host baseline (`--features test --lib --tests`) | 918 passed / 1 ignored (584 lib + 334 integration) | 2026-10-26 |
| Heat-sense state machine (`--features test,heat-sense --lib hardware::ssr_logic`) | 20 tests; library total 602 | 2026-10-26 |
| Simulated plant (`--features test,simulated-plant --lib --tests`) | 960 passed / 1 ignored (620 lib + 340 integration); 6/1 doctests | 2026-10-26 |
| Doctests (`--features test --doc`) | 6 passed / 1 ignored | 2026-10-26 |
| Regression (`--features test,regression --lib --tests`) | 993 passed / 1 ignored (620 lib + 373 integration) | 2026-10-26 |
| Regression CI form (includes doctests) | 999 passed / 2 ignored | 2026-10-26 |

## 7. Quality gates

The repository already contains the canonical quality scripts:

```bash
scripts/quality-baseline.sh
scripts/run-regression-checks.sh
```

Use them when you need a broader repository-level check instead of a narrow local command.

The project also enforces important Clippy constraints at the manifest level, including denying `unwrap`, `expect`, and `panic` usage in production code paths. That policy is worth remembering when reviewing changes because some existing source still reflects areas where enforcement, cfg-gating, or cleanup needs attention.

## 8. Debugging workflow

### Serial monitoring

Use `espflash monitor` to observe panics and warning/error logs and live runtime behavior. Logs go to
UART0; native USB carries the Artisan protocol. Boot `info!` lines are filtered by the default
`Warn` level, so rebuild with the `instrumentation` feature (and use a UART adapter) when you need
the startup log sequence.

The firmware emits useful startup logs during hardware init, watchdog init, and task startup. Those logs are often the fastest way to distinguish:

- hardware initialization failure,
- transport failure,
- protocol mismatch,
- repeated watchdog resets.

### Diagnostic protocol use

When the device boots and the serial path is available, the quickest interactive checks are:

1. `READ`
2. `STATUS`
3. `UNITS;C` or `UNITS;F`
4. `OT1`, `OT2`, and `STOP` in a safe environment

`STATUS` is especially important because it exposes runtime health rather than only roast values.

## 9. Hardware-in-the-loop validation

The repository includes a HIL workflow under `tests/hardware/`.

That workflow is the right place for:

- artifact-backed golden-output capture,
- threshold-based telemetry analysis,
- repeatable scenario execution,
- audit-ready run packaging.

If a change affects actuation timing, telemetry layout, or hardware behavior, HIL validation is more authoritative than host-only tests.

### Capturing HIL example output: unplug native USB first

The HIL examples (`examples/hil_*.rs`, `examples/gpio_roast_test.rs`) print
via `esp-println` with its default `auto` printer, which routes **each line**
to USB-Serial-JTAG as soon as the host enumerates the native USB port (SOF
flag, sticky once set) and to UART0 otherwise. The examples never initialise
USB, so anything printed after host enumeration lands in an unread FIFO and
is silently lost — this looks exactly like the firmware hanging mid-suite
(observed 2026-09-14 with `hil_c1`: output always stopped after test 02
while the suite kept running to 5/5 unseen).

Reliable procedure for short HIL suites: physically unplug the native-USB
cable and capture only UART0 (`/dev/ttyUSB0`). With no USB host present no
SOF ever arrives and 100 % of the output stays on UART. (The production
firmware is unaffected: it initialises USB CDC for real, so its `auto`
routing has a live reader on the USB side.)

## 10. Common failure modes developers should expect

### Build succeeds, flash fails

Usually a cable, port, permissions, or boot-mode issue.

### Host tests fail to link

Usually means the `test` feature was omitted from an x86_64 integration-style run.

### Temperatures appear wrong in Artisan

Usually a `UNITS` mismatch, not a thermocouple failure. Check `STATUS` field 19.

### Control loop behaves oddly under load

Check command latency fields, watchdog health, and guard timeout counters before assuming a PID bug.

## 11. Recommended change workflow

For non-trivial firmware changes, this is the safest sequence:

1. read `ARCHITECTURE.md` and `PROTOCOL.md`,
2. make the code change,
3. run targeted host tests,
4. run the host integration baseline with `--features test`,
5. run broader quality scripts if the change crosses subsystems,
6. run HIL validation when hardware behavior or telemetry contracts changed,
7. update the technical docs if behavior changed.

## 12. Related documents

- [`ARCHITECTURE.md`](ARCHITECTURE.md) for the runtime model.
- [`PROTOCOL.md`](PROTOCOL.md) for serial behavior.
- [`HARDWARE.md`](HARDWARE.md) for the electrical and pin model.
- [`SIMULATION.md`](SIMULATION.md) for the no-hardware Wokwi/esp-emu workflows.
- [`ARTISAN_CONNECTION.md`](ARTISAN_CONNECTION.md) for desktop-side setup.
- [`INSTRUMENTATION.md`](INSTRUMENTATION.md) for telemetry interpretation.
