# LibreRoaster Development Guide

**Last updated:** 2026-08-04

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
- **`async-lock-depth-metrics`** — enables async lock instrumentation used by concurrency tests
- **`regression`** — enables regression-specific mock support (pulls in `simulated-sensors`)

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

This is the correct default verification command when documentation or behavior changes touch protocol, control, or task orchestration.

### Regression numeric suite

The strongest numeric tests (`tests/sensor_conversion.rs` — 19-bit two's-complement LSB math, fixture→fault mapping) are gated behind the `regression` feature, so they are NOT part of the default `--features test` run. Run them explicitly (audit H-9, 2026-08-11):

```bash
cargo test --target x86_64-unknown-linux-gnu --features test --features regression --test sensor_conversion --no-fail-fast
```

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

Use `espflash monitor` to observe boot logs and live runtime behavior.

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

- `ARCHITECTURE.md` for the runtime model
- `PROTOCOL.md` for serial behavior
- `HARDWARE.md` for the electrical/pin model
- `ARTISAN_CONNECTION.md` for desktop-side setup
- `INSTRUMENTATION.md` for telemetry interpretation
