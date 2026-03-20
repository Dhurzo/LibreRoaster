# Development Guide - LibreRoaster

> Last Updated: 2026-03-11 (v5.1)

This guide provides complete instructions for building, flashing, testing, and debugging LibreRoaster firmware.

## Prerequisites

Before developing LibreRoaster, ensure you have:

### Hardware

- ESP32-C3 development board (USB-C recommended)
- USB cable (data-capable)
- Computer with USB port
- (Optional) MAX31856 thermocouple boards for testing with real sensors

### Software

1. **Rust Toolchain** (v1.88):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup update stable
   ```

2. **ESP32-C3 Target**:
   ```bash
   rustup target add riscv32imc-unknown-none-elf
   ```

3. **espflash Tool** (for flashing firmware):
   ```bash
   cargo install espflash
   ```

## Build

### Building Firmware

LibreRoaster is built as an embedded binary for the ESP32-C3 RISC-V processor. The build requires specifying the target explicitly:

```bash
# Build for ESP32-C3 embedded target
cargo build --release --target riscv32imc-unknown-none-elf --features embedded
```

**Output location:** `target/riscv32imc-unknown-none-elf/release/libreroaster.bin`

**Audit:** The audited `cargo build --release --target riscv32imc-unknown-none-elf --features embedded` run is documented in [95-build-verification.md](../95-build-verification.md), proving the embedded ELF (`target/riscv32imc-unknown-none-elf/release/libreroaster`) and `.bin` (`target/riscv32imc-unknown-none-elf/release/libreroaster.bin`) artifacts produced during BUILD-01. Rerun the same command to reproduce those binaries and consult the audit artifact paths for traceability.

> **Note:** The `--target riscv32imc-unknown-none-elf` flag is required because LibreRoaster is an embedded application (no stdlib), not a host application.

### Build Modes

```bash
# Build in release mode (optimized, smaller binary)
cargo build --release

# Build in debug mode (faster compilation, larger binary)
cargo build

# Clean build artifacts
cargo clean
```

### Development Features

LibreRoaster provides Cargo features to enable optional functionality:

| Feature | Purpose | Command Example |
|---------|---------|-----------------|
| `std` | Enable standard library (for host tests) | `cargo test --features std ...` |
| `test` | Enable test helpers | `cargo test --features test ...` |
| `async-lock-depth-metrics` | Enable async mutex depth instrumentation for concurrency testing | `cargo test --features async-lock-depth-metrics ...` |
| `embedded` | Enable embedded binary build | `cargo build --features embedded ...` |

#### Combining Features

Multiple features can be combined:

```bash
# Run host tests with async lock metrics
cargo test --features "std,test,async-lock-depth-metrics" --target x86_64-unknown-linux-gnu
```

## Flash

### Connection Steps

#### 1. Connect to ESP32-C3

1. Connect your ESP32-C3 board to your computer via USB
2. The board should power on (LED indicator)
3. Note: USB port:
   - Linux: `/dev/ttyUSB0` or `/dev/ttyACM0`
   - macOS: `/dev/cu.usbserial-*` or `/dev/cu.usbmodem-*`
   - Windows: `COM3`, `COM4`, etc.

#### 2. List Available Ports

```bash
# Using espflash
espflash list

# Or using cargo
cargo espflash list
```

### Flashing Methods

#### Method 1: espflash CLI (Recommended)

```bash
# Flash with default settings (automatic port detection)
espflash flash target/riscv32imc-unknown-none-elf/release/libreroaster.bin

# Specify port manually
espflash flash --port /dev/ttyACM0 target/riscv32imc-unknown-none-elf/release/libreroaster.bin

# Flash and open serial monitor
espflash flash --monitor target/riscv32imc-unknown-none-elf/release/libreroaster.bin
```

#### Method 2: cargo espflash

```bash
# Build and flash in release mode
cargo espflash flash --release

# Flash and monitor output
cargo espflash flash --release --monitor

# Monitor only (without flashing)
cargo espflash monitor --speed 115200
```

### Troubleshooting

#### Device Not Found

- Try a different USB cable (some cables are power-only)
- Try a different USB port (preferably USB 2.0)
- Check if the device is detected: `ls /dev/ttyACM*` (Linux)

#### Permission Denied (Linux)

```bash
# Add user to dialout group
sudo usermod -a -G dialout $USER

# Then log out and log back in
```

#### Flash Write Errors

- Disconnect and reconnect the board
- Hold BOOT button while flashing
- Try a different USB port or cable
- Ensure the board is not in a strange boot mode
- Try `--bootloader` flag with your flashing command

## Test

LibreRoaster includes a comprehensive test suite. Tests can run on the host (x86_64) for development without requiring ESP32-C3 hardware.

### Basic Test Commands

```bash
# Run all tests
cargo test

# Run specific test by name
cargo test test_name

# Run with output (see print statements)
cargo test -- --nocapture
```

### Host Integration Tests

These tests run on the host (x86_64) without embedded hardware. They validate concurrent behavior, command routing, and Artisan protocol compatibility.

| Test | Command | Purpose |
|------|---------|---------|
| **Command Multiplexer Concurrency** | `cargo test --target x86_64-unknown-linux-gnu --test command_multiplexer_concurrency` | Validates concurrent USB+UART command routing without queue saturation |
| **Concurrent Sensor Read** | `cargo test --features async-lock-depth-metrics --target x86_64-unknown-linux-gnu --test concurrent_sensor_test` | Proves async mutex handles concurrent sensor reads without race conditions |
| **Mock UART Integration** | `cargo test --target x86_64-unknown-linux-gnu --test mock_uart_integration` | Tests UART communication protocol with mock hardware |
| **Artisan Integration** | `cargo test --target x86_64-unknown-linux-gnu --test artisan_integration_test` | Validates Artisan command/response protocol compliance |

> **Note:** Host tests do not require ESP32-C3 hardware. They run on your development machine using the `std` feature.

### Concurrency Regression Test

- Run the host-side multiplexer stress test:
  ```bash
  cargo test --target x86_64-unknown-linux-gnu --test command_multiplexer_concurrency
  ```
- The test spawns `queue_processor_task`/`usb_queue_processor_task`, fires concurrent USB+UART commands, and drives `ServiceContainer::roaster_async_sensor_read()` via a `ThreadPool` so the real queue processor is exercised.
- Instrumentation lives in `libreroaster::application::queue_metrics` (`QueueProcessorMetrics`), and `queue_processor_metrics_snapshot()` returns:
  - `queue_depth`: most recent occupancy of the command queue.
  - `max_depth`: highest occupancy observed while the test ran.
  - `backlog_events`: each time the queue depth hit or exceeded `QUEUE_DEPTH_BACKLOG_THRESHOLD` (currently 24, which is 3/4 of the queue) both producers contributed to the same metric.
- Operators should verify the snapshot after a run: `max_depth` stays below 24 and `backlog_events == 0`. Any backlog event signals the queue saw saturation and is a prompt to revisit command burst pacing or queue handling.
- To dive deeper, run the test with `-- --nocapture` and instrument `queue_processor_metrics_snapshot()` in your debugger or additional test helpers to inspect per-run values.

### Concurrent Sensor Read Instrumentation (ASYNC-06)

- Execute the host-side concurrent sensor read proof with async lock metrics enabled:
  ```bash
  cargo test --features async-lock-depth-metrics --target x86_64-unknown-linux-gnu --test concurrent_sensor_test
  ```
- The harness boots `ServiceContainer`, populates both async and sync `RoasterControl` instances, then uses a `ThreadPool` to spawn ten `ServiceContainer::roaster_async_sensor_read()` futures and `join_all` to batch so every `Result<(), ContainerError>` is asserted.
- Internally `ServiceContainer` instruments the embassy mutex with test-only helpers `async_lock_depth_max_for_tests()` and `reset_async_lock_metrics_for_tests()` so the test can confirm `max_async_lock_depth` never exceeds `1` (no parallel holders) and that counters reset to zero before/after each run.
- Passing runs prove ASYNC-06 for the milestone audit because the host harness proves the async mutex survives concurrent sensor reads without dropped locks or multiple holders, and the README's command plus telemetry proves we can rerun coverage on demand.

## Debug

### Serial Monitor

```bash
# Using espflash
espflash monitor --speed 115200

# Or using cargo
cargo espflash monitor --speed 115200
```

### Common Issues

#### 1. Flash Write Errors

- Check USB connection
- Try different USB port
- Ensure ESP32-C3 is properly connected

#### 2. Build Errors

- Update Rust toolchain: `rustup update stable`
- Clear build artifacts: `cargo clean`
- Check internet connection for dependency downloads

#### 3. Sensor Timeout Errors

- Check SPI wiring (SCLK, MOSI, MISO, CS pins)
- Verify thermocouple connections to MAX31856
- Ensure pull-up resistors are installed on CS lines

#### 4. Watchdog Timeouts

- If watchdog fires unexpectedly, check control loop timing
- Verify `WATCHDOG_FEED_INTERVAL_MS` (100ms) is being met
- Check for blocking operations in the control loop

#### 5. UART Communication Issues

- Verify TX/RX pin assignments (GPIO20/21)
- Check baud rate is 115200
- Ensure no cross-talk with other peripherals

## Quality Baseline

The project includes automated quality gates and regression checks:

### Running Quality Checks

```bash
# Run the quality baseline (format, clippy, tests)
scripts/quality-baseline.sh
```

This script:
1. Runs `cargo fmt` to ensure code is formatted
2. Runs `cargo clippy` with project-specific policy settings
3. Runs `cargo test` to verify all tests pass
4. Exits with non-zero status if any check fails

### Running Regression Checks

```bash
# Run the full regression suite
scripts/run-regression-checks.sh
```

This script runs all host integration tests to verify no regressions were introduced.

### Quality Policy

The project uses a module-criticality ratcheting quality policy defined in `.cargo/config.toml`:

- **Tier 1** (safety/control/protocol modules): Stricter linting, no warnings allowed
- **Tier 2** (other modules): Standard linting, some warnings allowed

For details, see `.cargo/config.toml` and `scripts/quality-baseline.sh`.

---

## Additional Documentation

For more information, see:

- [HARDWARE.md](HARDWARE.md) - Hardware specifications, pinout, and wiring
- [ARCHITECTURE.md](ARCHITECTURE.md) - System architecture and task structure
- [PROTOCOL.md](PROTOCOL.md) - Artisan protocol reference
- [INSTRUMENTATION_README.MD](INSTRUMENTATION_README.MD) - Watchdog and regression telemetry
- [README.md](../README.md) - Project overview and quick start guide
