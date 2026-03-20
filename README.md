# LibreRoaster - OpenSource Coffee Bean Roaster Firmware ☕🔥

LibreRoaster is a open-source (hackable) coffee bean roaster designed for ESP32-C3 (firmware & hardware). Built with modern embedded Rust using Embassy async framework, featuring dual thermocouple monitoring, PWM heater/fan control, and Artisan+ compatibility via USB or UART communication.


## Project Status

**Current version:** v5.1 (2026‑03‑12)
**Milestone:** v5.1 in progress – Documentation update and minor improvements.
**Next:** v5.2 – TBD.

### Recent Changes

- Added STATUS command with 18‑field CSV for automation telemetry
- Added REG command for over‑temperature regression testing
- Enhanced watchdog instrumentation and safety logging
- Implemented quality baseline scripts (quality‑baseline.sh)
- Refactored unsafe code and improved memory strategy
- Updated all internal documentation (ARCHITECTURE.md, PROTOCOL.md, etc.)

## Project Philosophy

The project aims to enable anyone with intermediate technical skills to build their own affordable coffee roaster. Due to the cost-focused approach, certain components are chosen over more expensive alternatives - this is evident in the (future) hardware section where even recycled components are utilized.

The project is adaptable to both more expensive and more budget-friendly components. The design has also been kept simple, which means the roaster is dependent on ARTISAN+ and does not function in "standalone" mode without ARTISAN+ (a standalone version with a different controller could be considered if there is community interest).

## Core Value

Artisan can read temperatures and control heater/fan during a roast session via serial connection.

## Features

| Feature | Description |
|---------|-------------|
| **Dual-Channel Artisan Communication** | Connect via USB CDC (native) or UART0 (GPIO20/21) at 115200 baud |
| **Dual Thermocouple Monitoring** | MAX31856-based ET (Environment) and BT (Bean) temperature sensing |
| **PWM Output Control** | SSR heater and fan control via LEDC PWM (0-100%) |
| **Rate of Rise (ROR)** | Automatic BT rate-of-change calculation for roasting metrics |
| **Command Multiplexer** | Routes Artisan commands between USB and UART with 60s timeout |
| **Initialization Handshake** | Supports CHAN→UNITS→FILT sequence with `#` acknowledgment |

### Async Architecture

LibreRoaster uses the **Embassy** async framework for concurrent task execution:

| Component | Description |
|-----------|-------------|
| **Embassy Executor** | Task scheduler for ESP32-C3 RISC-V |
| **Async Sensors** | Non-blocking MAX31856 temperature reads using embassy-time timers |
| **Async UART/USB** | Non-blocking serial communication via embassy traits |
| **Async Mutex** | Thread-safe access to shared RoasterControl |
| **Channel Communication** | Inter-task command passing via embassy-sync channels |

The async architecture enables:
- Concurrent USB and UART command processing
- Non-blocking sensor reads (no busy-wait loops)
- Predictable timing with embassy-time delays
- Safe concurrent access to shared state

### Supported Artisan Commands

| Command | Description |
|---------|-------------|
| `READ` | Request telemetry (ET, BT, heater%, fan%) |
| `REG` | Regression-runner trigger that ramps heater/fan to 100%, keeps the watchdog fed, and emits SAFETY OT-REGRESSION records so automation can detect and monitor over-temperature regression cycles |
| `STATUS/STAT` | Automation telemetry snapshot returning 18 CSV fields including ET, BT, Heater, Fan, WatchdogOK, WatchdogFailures, LastWatchdogReason, LEDCGuardTimeouts, RegressionActive, PID state (PV, MV, IntegratorValue, DerivativeValue), flags (SaturationFlag, IntegratorClampFlag, DerivativeAvailableFlag), and latency metrics (CommandLatency, MaxCommandLatency). See INSTRUMENTATION_README.MD for complete field definitions. (alias `STAT`) while surfacing watchdog guard/regression telemetry without touching `READ` |
| `OT1 [0-100]` | Set heater power percentage |
| `OT2 [0-100]` | Set fan speed percentage (auto-cuts heater if out of range) |
| `IO3 [0-100]` | Set fan speed percentage |
| `UP` | Increase heater by 5% |
| `DOWN` | Decrease heater by 5% |
| `START` | Begin roasting, enable continuous output |
| `STOP` | Emergency stop, disable outputs |
| `CHAN [rate]` | Set communication rate (legacy) |
| `UNITS [C/F]` | Set temperature units (Celsius/Fahrenheit) |
| `FILT [value]` | Set filter value (legacy) |

Automation-focused readers should consult [internalDoc/INSTRUMENTATION_README.MD](internalDoc/INSTRUMENTATION_README.MD) immediately after this table for the STATUS/STAT column definitions, payload expectations, and the way REG logs SAFETY OT-REGRESSION so instrumentation crews can react safely.

## Quick Start

### 1. Flash the Firmware

See [FLASH_GUIDE.md](internalDoc/FLASH_GUIDE.md) for detailed flashing instructions.

**Summary:**
1. Connect ESP32-C3 via USB
2. Use espflash GUI or CLI to flash `libreroaster.bin`
3. Verify successful flash

### 2. Connect to Artisan

See [ARTISAN_CONNECTION.md](internalDoc/ARTISAN_CONNECTION.md) for detailed connection instructions.

**Summary:**
1. Identify the USB port (ttyACM on Linux, /dev/cu.usbmodem-* on macOS, COM on Windows)
2. Configure Artisan: port + 115200 baud + Arduino/RPi mode
3. Verify connection with READ command

## Hardware Requirements

| Component | Description |
|-----------|-------------|
| ESP32-C3 | RISC-V development board |
| 2x MAX31856 | Ther
mocouple amplifier boards |
| 2x Type-K Thermocouples | Bean Temp and Environment Temp |
| SSR | Solid State Relay for heater control |
| Fan | Variable speed fan (PWM controlled) |

### HIL Validation and Golden Outputs

HW-03 requires every manifest-driven HIL scenario to produce artifact-backed evidence. This HIL validation workflow is the authoritative path for capturing golden outputs, so follow `tests/hardware/HIL-PLAYBOOK.md` to:

- Select a scenario from `tests/hardware/scenario_manifest.json` and run `tests/hardware/validation_runner.py` using the `--manifest`, `--scenario`, and `--runs-dir` flags so telemetry and metadata land under `tests/hardware/runs/`.
- Run `tests/hardware/analysis.py` with `--thresholds tests/hardware/thresholds.json` and `--template tests/hardware/report_template.md` to generate a report that embeds scenario metadata, golden artifact links, PASS/FAIL badges, and run metadata for auditors.
- Bundle the CSV, metadata JSON, markdown report, and manifest entry into a tarball so auditors can verify the path from scenario manifest → telemetry → golden artifact.

Approved golden CSVs live in `tests/hardware/goldens/` and must include the `metadata.retention_days` value specified in the manifest so artifact retention matches HW-03 expectations.

## Pinout

| GPIO | Function | Description | Note |
|------|----------|-------------|------|
| 3 | MAX31856 #1 CS | Environment Temperature (ET) | SPI bus shared |
| 4 | MAX31856 #2 CS | Bean Temperature (BT) | SPI bus shared |
| 5 | SPI MOSI | Master Out Slave In | Shared between MAX31856 chips |
| 6 | SPI MISO | Master In Slave Out | Shared |
| 7 | SPI SCLK | Serial Clock | Shared |
| 9 | Fan PWM | Fan speed control (25kHz) | Strapping pin – safe with pull‑up (see internalDoc/HARDWARE.md) |
| 10 | SSR PWM | Heater control (1Hz) | |
| 1 | Heat Detection | SSR feedback input | Pull‑up enabled |
| 20 | UART TX | Artisan communication (to Artisan) | |
| 21 | UART RX | Artisan communication (from Artisan) | |

For detailed hardware wiring and strapping pin information, see [HARDWARE.md](internalDoc/HARDWARE.md).

## Artisan Connection

LibreRoaster supports two connection methods to Artisan:

| Method | Description |
|--------|-------------|
| **USB CDC** | Native ESP32-C3 USB (recommended) — no adapter needed |
| **UART0** | GPIO20/21 at 115200 baud — requires USB-to-UART adapter |

**Detailed guide:** See [ARTISAN_CONNECTION.md](internalDoc/ARTISAN_CONNECTION.md) for complete connection instructions including wiring diagrams, port identification, and troubleshooting.

**Key settings:**
- Baud rate: 115200
- Mode: Arduino/RPi
- Commands work immediately (handshake disabled)

## Protocol

### READ Response Format

4-value CSV: ET,BT,HEATER,FAN

| Field | Type | Unit | Description |
|-------|------|------|-------------|
| ET | Decimal | °C | Exhaust Temperature |
| BT | Decimal | °C | Bean Temperature |
| HEATER | Decimal | % | Heater PWM percentage |
| FAN | Decimal | % | Fan PWM percentage |

Example: `185.3,201.4,45,80`

### Initialization

Artisan sends commands without handshake or Artisan sends handshake sequence (CHAN, UNITS, FILT). LibreRoaster responds with `#` acknowledgment. 

## License

Apache 2.0

## Project Structure

```
├── src/
│   ├── main.rs              # Main application entry point
│   ├── lib.rs               # Library interface
│   ├── application/         # Application architecture
│   │   ├── mod.rs          # Application module exports
│   │   ├── app_builder.rs  # Service container and dependency injection
│   │   ├── service_container.rs # Service management
│   │   └── tasks.rs        # Application tasks
│   ├── hardware/           # Hardware abstraction layer
│   │   ├── mod.rs         # Hardware module exports
│   │   ├── max31856.rs    # MAX31856 thermocouple driver
│   │   ├── ssr.rs         # SSR control with LEDC PWM and heat detection
│   │   ├── fan.rs         # Fan control with LEDC PWM
│   │   ├── shared_spi.rs  # Shared SPI bus implementation
│   │   └── uart.rs        # UART communication
│   ├── control/            # Roaster control logic
│   │   ├── mod.rs         # Control module exports
│   │   ├── roaster_refactored.rs # State machine and command processing
│   │   └── handlers.rs     # Control handlers
│   ├── input/              # Input processing
│   │   ├── mod.rs         # Input module exports
│   │   └── parser.rs      # Artisan command parsing
│   ├── output/             # Output and formatting
│   │   ├── mod.rs         # Output module exports
│   │   ├── artisan.rs     # Artisan protocol formatter
│   │   └── uart.rs        # UART output
│   ├── config/             # Configuration
│   │   └── constants.rs    # Hardware constants and pin assignments
│   └── error/              # Error handling
│       └── app_error.rs    # Custom error types
├── examples/
│   └── artisan_test.rs     # Artisan protocol example
├── .cargo/
│   └── config.toml         # Cargo target configuration
├── Cargo.toml               # Project dependencies
├── build.rs                # Build script
├── rust-toolchain.toml     # Rust toolchain specification
└── README.md               # This file
```

## Development

### Prerequisites

Before building LibreRoaster, ensure you have:

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

### Build Commands

```bash
# Build in release mode
cargo build --release

# Build in debug mode
cargo build

# Clean build artifacts
cargo clean
```

**Note:** For detailed flashing, testing, and debugging instructions, see [DEVELOPMENT.md](internalDoc/DEVELOPMENT.md).

### Quality Baseline and Regression Testing

The project includes automated quality gates and regression checks:

```bash
# Run the quality baseline (format, clippy, tests)
scripts/quality-baseline.sh

# Run the full regression suite
scripts/run-regression-checks.sh
```

These scripts are the authoritative source for quality policy and regression validation. They are used in CI and should be run before submitting changes.
#### Building for ESP32-C3

LibreRoaster is built as an embedded binary for the ESP32-C3 RISC-V processor. The build requires specifying the target explicitly:

```bash
# Build for ESP32-C3 embedded target
cargo build --release --target riscv32imc-unknown-none-elf --features embedded
```

**Output location:** `target/riscv32imc-unknown-none-elf/release/libreroaster.bin`

> **Note:** The `--target riscv32imc-unknown-none-elf` flag is required because LibreRoaster is an embedded application (no stdlib), not a host application.

#### Build and Flash Workflow

Complete end-to-end workflow to build and flash firmware to ESP32-C3:

```bash
# 1. Build firmware with embedded features
cargo build --release --target riscv32imc-unknown-none-elf --features embedded

# 2. Verify binary was produced (optional but recommended)
ls -lh target/riscv32imc-unknown-none-elf/release/libreroaster.bin

# 3. Flash to ESP32-C3
cargo espflash flash --release

# 4. Flash and monitor serial output
cargo espflash flash --release --monitor
```

**Workflow steps:**
1. **Build** - Compile the firmware with `--features embedded` to enable the binary target
2. **Verify** - Confirm the `.bin` file exists before attempting to flash
3. **Flash** - Write the binary to ESP32-C3 using espflash
4. **Monitor** - Optionally view serial output to verify successful boot

For detailed flashing instructions and troubleshooting, see [DEVELOPMENT.md](internalDoc/DEVELOPMENT.md).

### Test Commands

LibreRoaster includes a comprehensive test suite. Tests can run on the host (x86_64) for development without requiring ESP32-C3 hardware.

#### Basic Test Commands

```bash
# Run all tests
cargo test

# Run specific test by name
cargo test test_name

# Run with output (see print statements)
cargo test -- --nocapture
```
**Quality and Regression:** For a complete quality and regression check, run the scripts described in [Quality Baseline and Regression Testing](#quality-baseline-and-regression-testing).


#### Host Integration Tests

These tests run on the host (x86_64) without embedded hardware. They validate concurrent behavior, command routing, and Artisan protocol compatibility.

| Test | Command | Purpose |
|------|---------|---------|
| **Command Multiplexer Concurrency** | `cargo test --target x86_64-unknown-linux-gnu --test command_multiplexer_concurrency` | Validates concurrent USB+UART command routing without queue saturation |
| **Concurrent Sensor Read** | `cargo test --features async-lock-depth-metrics --target x86_64-unknown-linux-gnu --test concurrent_sensor_test` | Proves async mutex handles concurrent sensor reads without race conditions |
| **Mock UART Integration** | `cargo test --target x86_64-unknown-linux-gnu --test mock_uart_integration` | Tests UART communication protocol with mock hardware |
| **Artisan Integration** | `cargo test --target x86_64-unknown-linux-gnu --test artisan_integration_test` | Validates Artisan command/response protocol compliance |

> **Note:** Host tests do not require ESP32-C3 hardware. They run on your development machine using the `std` feature.

### Concurrency Regression Test

- Run the new host-side multiplexer stress test:
  ```bash
  cargo test --target x86_64-unknown-linux-gnu --test command_multiplexer_concurrency
  ```
- The test spawns `queue_processor_task`/`usb_queue_processor_task`, fires concurrent USB+UART commands, and drives `ServiceContainer::roaster_async_sensor_read()` via a `ThreadPool` so the real queue processor is exercised.
- Instrumentation lives in `libreroaster::application::queue_metrics` (`QueueProcessorMetrics`), and `queue_processor_metrics_snapshot()` returns:
  - `queue_depth`: the most recent occupancy of the command queue.
  - `max_depth`: the highest occupancy observed while the test ran.
  - `backlog_events`: each time the queue depth hit or exceeded `QUEUE_DEPTH_BACKLOG_THRESHOLD` (currently 24, which is 3/4 of the queue) both producers contributed to the same metric.
- Operators should verify the snapshot after a run: `max_depth` stays below 24 and `backlog_events == 0`. Any backlog event signals the queue saw saturation and is a prompt to revisit command burst pacing or queue handling.
- To dive deeper, run the test with `-- --nocapture` and instrument `queue_processor_metrics_snapshot()` in your debugger or additional test helpers to inspect the per-run values.

### Concurrent sensor read instrumentation (ASYNC-06)

- Execute the host-side concurrent sensor read proof with async lock metrics enabled:
  ```bash
  cargo test --features async-lock-depth-metrics --target x86_64-unknown-linux-gnu --test concurrent_sensor_test
  ```
- The harness boots `ServiceContainer`, populates both async and sync `RoasterControl` instances, then uses a `ThreadPool` to spawn ten `ServiceContainer::roaster_async_sensor_read()` futures and `join_all` the batch so every `Result<(), ContainerError>` is asserted.
- Internally `ServiceContainer` instruments the embassy mutex with test-only helpers `async_lock_depth_max_for_tests()` and `reset_async_lock_metrics_for_tests()` so the test can confirm `max_async_lock_depth` never exceeds `1` (no parallel holders) and that the counters reset to zero before/after each run.
- Passing runs prove ASYNC-06 for the milestone audit because the host harness proves the async mutex survives concurrent sensor reads without dropped locks or multiple holders, and the README's command plus telemetry proves we can rerun the coverage on demand.

### Development Features

LibreRoaster provides Cargo features to enable optional functionality:

| Feature | Purpose | Command Example |
|---------|---------|-----------------|
| `std` | Enable standard library (for host tests) | `cargo test --features std ...` |
| `test` | Enable test helpers | `cargo test --features test ...` |
| `async-lock-depth-metrics` | Enable async mutex depth instrumentation for concurrency testing | `cargo test --features async-lock-depth-metrics ...` |
| `embedded` | Enable embedded binary build | `cargo build --features embedded ...` |

#### Using async-lock-depth-metrics

The `async-lock-depth-metrics` feature instruments the Embassy async mutex to track lock depth during concurrent operations:

- **What it does:** Instruments the embassy mutex to track maximum concurrent holders
- **When to use:** Running `concurrent_sensor_test` to verify no race conditions
- **How to interpret results:** `max_async_lock_depth` should never exceed `1` (indicating no parallel holders)

```bash
# Run concurrent sensor test with lock metrics
cargo test --features async-lock-depth-metrics --target x86_64-unknown-linux-gnu --test concurrent_sensor_test
```

#### Combining Features

Multiple features can be combined:

```bash
# Run host tests with async lock metrics
cargo test --features "std,test,async-lock-depth-metrics" --target x86_64-unknown-linux-gnu
```

### Flash Commands

```bash
# List available ports
cargo espflash list

# Flash firmware
cargo espflash flash --release

# Flash and monitor
cargo espflash flash --release --monitor

# Monitor only
cargo espflash monitor
```

## Debugging

### Serial Monitor

```bash
cargo espflash monitor --speed 115200
```

### Common Issues

1. **Flash Write Errors**: 
   - Check USB connection
   - Try different USB port
   - Ensure ESP32-C3 is properly connected

2. **Build Errors**:
   - Update Rust toolchain: `rustup update stable`
   - Clear build artifacts: `cargo clean`
   - Check internet connection for dependency downloads

## Safety Features

LibreRoaster implements multiple safety mechanisms:

| Feature | Threshold | Action |
|---------|-----------|--------|
| **Over-temperature** | 260°C | Emergency shutdown, cut heater, max fan |
| **Sensor Timeout** | 1 second | Fault condition, disable heater |
| **Heat Detection** | SSR feedback | Verify heater is actually turning on |
| **Fault Conditions** | Any fault | Emergency shutdown, max fan for cooling |

### Safety Guarantees

- **Automatic Emergency Shutdown**: If temperature exceeds 260°C or sensor times out, the system automatically cuts power to the heater and runs the fan at 100% for cooling
- **Heat Source Verification**: System monitors SSR feedback to verify the heater element is actually drawing power
- **Temperature Validity**: Sensor readings older than 1 second are marked invalid to prevent stale data from causing issues
- **Fault Tracking**: All fault conditions are logged and trigger safe shutdown state
- **Manual Emergency**: STOP command immediately cuts heater and sets fan to 100%

## ⚠️ Safety Warning

**This project involves serious safety risks.**

LibreRoaster works with:

- ⚡ **High voltages**
- 🔥 **Very high temperatures**

Improper handling can result in **severe injury, fire, or death**.

### Please follow these precautions:

- Only work on the hardware if you have **proper electrical knowledge**.
- Always disconnect power before modifying or servicing the device.
- Use appropriate **thermal insulation and heat-resistant materials**.
- **Never leave the roaster unattended while operating.**
- Keep a **fire extinguisher nearby at all times** when using the roaster.
- Operate the roaster in a **well-ventilated and fire-safe area**.

> ⚠️ You build and use this project **at your own risk**.  
> The authors and contributors are **not responsible** for any damage, injury, or loss.

---

## 📜 License

This project is open source under APACHE-2 LICENCE.  
See the `LICENSE` file for more information.
