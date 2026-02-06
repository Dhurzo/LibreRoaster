# Codebase Structure

**Analysis Date:** 2026-02-04

## Directory Layout

```
/home/juan/Repos/LibreRoaster/
├── src/                          # Main source code
│   ├── main.rs                   # Application entry point (binary)
│   ├── lib.rs                    # Library root (no_std)
│   ├── application/              # App orchestration & tasks
│   │   ├── mod.rs
│   │   ├── app_builder.rs        # Builder pattern for app construction
│   │   ├── service_container.rs  # Global singleton container
│   │   └── tasks.rs              # Embassy async task definitions
│   ├── control/                  # Core roasting logic
│   │   ├── mod.rs
│   │   ├── roaster_refactored.rs # Main RoasterControl struct
│   │   ├── handlers.rs           # Command handlers (4 implementations)
│   │   ├── command_handler.rs    # RoasterCommandHandler trait
│   │   ├── abstractions.rs       # PidController, OutputManager traits
│   │   ├── traits.rs             # Hardware traits (Thermometer, Heater, Fan)
│   │   └── pid.rs                # PID controller implementation
│   ├── hardware/                # Hardware drivers
│   │   ├── mod.rs
│   │   ├── max31856.rs           # Thermocouple sensor driver
│   │   ├── fan.rs                # PWM fan controller
│   │   ├── ssr.rs                # Solid State Relay driver
│   │   ├── shared_spi.rs         # Shared SPI bus wrapper
│   │   └── uart/                  # UART communication
│   │       ├── mod.rs
│   │       ├── driver.rs          # UART driver initialization
│   │       ├── tasks.rs           # Reader/writer tasks
│   │       └── buffer.rs          # Circular buffer implementation
│   ├── input/                    # Input parsing
│   │   ├── mod.rs
│   │   └── parser.rs             # Artisan command parsing
│   ├── output/                   # Output formatting
│   │   ├── mod.rs
│   │   ├── artisan.rs            # Artisan CSV protocol formatter
│   │   ├── manager.rs            # Output manager
│   │   ├── scheduler.rs          # Output scheduling
│   │   ├── traits.rs             # OutputFormatter trait
│   │   └── uart.rs               # UART printer
│   ├── config/                   # Configuration & types
│   │   ├── mod.rs
│   │   └── constants.rs           # All magic numbers & type definitions
│   └── error/                    # Error types
│       ├── mod.rs
│       └── app_error.rs          # Application errors
├── examples/                      # Example code
│   └── artisan_test.rs
├── docs/                         # Documentation
├── internalDoc/                  # Internal documentation
├── Cargo.toml                    # Project manifest
├── Cargo.lock                    # Dependency lock file
├── build.rs                      # Build script
├── rust-toolchain.toml           # Rust version specification
├── sdkconfig.defaults            # ESP-IDF SDK defaults
└── README.md
```

## Directory Purposes

### Source Root (`src/`)

**Purpose:** Contains all application code for the roaster firmware

**Contains:** 6 main modules, 1 binary entry point, 1 library root

**Key files:**
- `main.rs`: ESP32-C3 firmware entry point with `#![no_main]` and `#![no_std]`
- `lib.rs`: Library root exporting all modules for internal use

### Application Layer (`src/application/`)

**Purpose:** Application orchestration, dependency injection, task management

**Key files:**
- `app_builder.rs`: `AppBuilder` struct implementing builder pattern for constructing the application with all dependencies
- `service_container.rs`: `ServiceContainer` singleton providing access to `RoasterControl` and `ArtisanInput`
- `tasks.rs`: Embassy async task definitions (`control_loop_task`, `artisan_output_task`)

### Control Layer (`src/control/`)

**Purpose:** Core roasting logic, PID control, command processing, safety

**Key files:**
- `roaster_refactored.rs`: `RoasterControl` - main orchestrator struct owning state, handlers, and hardware
- `handlers.rs`: Four command handler implementations (`TemperatureCommandHandler`, `SafetyCommandHandler`, `ArtisanCommandHandler`, `SystemCommandHandler`)
- `traits.rs`: Hardware abstraction traits (`Thermometer`, `Heater`, `Fan`)
- `pid.rs`: `CoffeeRoasterPid` - PID controller implementation

### Hardware Layer (`src/hardware/`)

**Purpose:** ESP32-C3 hardware drivers for sensors, actuators, communication

**Key files:**
- `max31856.rs`: MAX31856 thermocouple amplifier driver for bean/environment temperature
- `fan.rs`: `FanController` and `SimpleLedcFan` - PWM fan control via LEDC
- `ssr.rs`: `SsrControlSimple` - Solid State Relay control with heat detection
- `uart/`: UART communication subsystem (see subdirectory below)

**Subdirectory: `src/hardware/uart/`**
- `driver.rs`: UART initialization and driver struct
- `tasks.rs`: `uart_reader_task`, `uart_writer_task` - async serial communication
- `buffer.rs`: `CircularBuffer` for serial data handling

### Input Layer (`src/input/`)

**Purpose:** Parse incoming Artisan+ protocol commands

**Key files:**
- `parser.rs`: `parse_artisan_command()` - string to `ArtisanCommand` enum parser

### Output Layer (`src/output/`)

**Purpose:** Format and send status data to Artisan software

**Key files:**
- `artisan.rs`: `ArtisanFormatter` and `MutableArtisanFormatter` - Artisan CSV protocol formatting
- `manager.rs`: `OutputManager` - processes and sends system status
- `traits.rs`: `OutputFormatter` trait for abstraction

### Config Layer (`src/config/`)

**Purpose:** Constants, type definitions, state structures

**Key files:**
- `constants.rs`: GPIO pins, PWM frequencies, temperature thresholds, state enums, command enums

### Error Layer (`src/error/`)

**Purpose:** Centralized error types

**Key files:**
- `app_error.rs`: `AppError` enum and error handling

## Key File Locations

### Entry Points

- **Firmware entry:** `src/main.rs` - ESP32-C3 binary with `#[esp_rtos::main]` async main
- **Library root:** `src/lib.rs` - no_std library exporting all modules

### Configuration

- **All constants:** `src/config/constants.rs` - GPIO pins, thresholds, enums
- **Cargo manifest:** `Cargo.toml` - dependencies, features, profiles
- **Rust toolchain:** `rust-toolchain.toml` - specifies Rust 1.88

### Core Logic

- **Main orchestrator:** `src/control/roaster_refactored.rs` - `RoasterControl` struct
- **Command routing:** `src/control/handlers.rs` - handler implementations
- **Hardware traits:** `src/control/traits.rs` - `Thermometer`, `Heater`, `Fan` traits

### Hardware Drivers

- **Temperature sensors:** `src/hardware/max31856.rs`
- **Fan control:** `src/hardware/fan.rs`
- **SSR control:** `src/hardware/ssr.rs`
- **UART:** `src/hardware/uart/driver.rs` and `src/hardware/uart/tasks.rs`

### Async Tasks

- **Task definitions:** `src/application/tasks.rs`
- **UART tasks:** `src/hardware/uart/tasks.rs`

## Naming Conventions

### Files

- **Module files:** `mod.rs` in each module directory
- **Main implementation:** Descriptive lowercase with underscores: `roaster_refactored.rs`, `artisan_test.rs`
- **Tests:** Inline in source files with `#[cfg(test)]` modules or separate files with `_tests.rs` suffix (e.g., `abstractions_tests.rs`)

### Structs

- **Main controllers:** `PascalCase` ending with purpose: `RoasterControl`, `FanController`, `ArtisanFormatter`
- **Handlers:** `PascalCase` ending with `Handler`: `TemperatureCommandHandler`, `SafetyCommandHandler`
- **Drivers:** Descriptive: `Max31856`, `SsrControlSimple`, `SimpleLedcFan`
- **Formatters:** Descriptive: `ArtisanFormatter`, `MutableArtisanFormatter`

### Traits

- **Hardware abstractions:** Noun: `Thermometer`, `Heater`, `Fan`
- **Control abstractions:** Noun ending with `Controller` or `Manager`: `PidController`, `OutputManager`
- **Handler abstractions:** Noun ending with `Handler`: `RoasterCommandHandler`
- **Output abstractions:** Noun: `OutputFormatter`

### Enums

- **State/mode:** `PascalCase` ending with `State` or `Status`: `RoasterState`, `SsrHardwareStatus`
- **Commands:** `PascalCase` ending with `Command`: `ArtisanCommand`, `RoasterCommand`
- **Errors:** `PascalCase` ending with `Error` or `ErrorType`: `RoasterError`, `InputError`

### Functions

- **Methods:** `snake_case`: `read_sensors()`, `set_power()`, `enable_pid()`
- **Constructor-like:** `new()`, `with_*()` (builder pattern)
- **Initialization:** `init_*()`, `initialize_*()`
- **Async tasks:** `*_task` suffix: `control_loop_task()`, `uart_reader_task()`

### Constants

- **Pins:** `UPPER_SNAKE_CASE`: `SPI_SCLK_PIN`, `SSR_CONTROL_PIN`, `FAN_PWM_PIN`
- **Frequencies:** `UPPER_SNAKE_CASE`: `FAN_PWM_FREQUENCY_HZ`, `SSR_PWM_FREQUENCY_HZ`
- **Thresholds:** `UPPER_SNAKE_CASE`: `OVERTEMP_THRESHOLD`, `MAX_SAFE_TEMP`
- **Timeouts:** `UPPER_SNAKE_CASE`: `TEMP_VALIDITY_TIMEOUT_MS`, `PID_SAMPLE_TIME_MS`
- **Offsets:** `UPPER_SNAKE_CASE`: `BT_THERMOCOUPLE_OFFSET`, `ET_THERMOCOUPLE_OFFSET`

### Modules

- **Layer modules:** Lowercase plural: `application`, `control`, `hardware`, `input`, `output`, `config`, `error`
- **Submodules:** Descriptive lowercase: `uart`, `parser`, `manager`, `scheduler`

## Where to Add New Code

### New Feature (e.g., Add Web Server)

1. **Primary code:** Create `src/network/` or extend `src/hardware/` for network hardware
2. **New module:** Add module declaration in `src/lib.rs`
3. **Integration:** Wire into `AppBuilder` in `src/application/app_builder.rs`

### New Hardware Driver

1. **Implementation:** Create in `src/hardware/` (e.g., `src/hardware/new_sensor.rs`)
2. **Module export:** Add to `src/hardware/mod.rs`
3. **Trait implementation:** Implement appropriate trait from `src/control/traits.rs`
4. **Registration:** Wire through `AppBuilder` builder methods

### New Command Handler

1. **Implementation:** Add to `src/control/handlers.rs` (new struct implementing `RoasterCommandHandler`)
2. **Registration:** Add to `RoasterControl::new()` in `src/control/roaster_refactored.rs`
3. **Routing:** Update `RoasterControl::process_command()` to include handler

### New Output Formatter

1. **Implementation:** Create in `src/output/` (e.g., `src/output/json.rs`)
2. **Trait:** Implement `OutputFormatter` from `src/output/traits.rs`
3. **Builder:** Add to `AppBuilder::with_formatter()` in `src/application/app_builder.rs`

### Utilities/Helpers

1. **Location:** `src/` root or appropriate module
2. **Pattern:** Create `utils.rs` in module or `src/utils/` if many utilities

### Tests

1. **Unit tests:** Inline `#[cfg(test)]` module in source file
2. **Integration tests:** Create `src/module/*_tests.rs` or `tests/` directory
3. **Example tests:** Add to `examples/` and run with `cargo test --example`

## Special Directories

### `examples/`

**Purpose:** Example code demonstrating library usage

**Contents:** `artisan_test.rs` - test example for Artisan protocol

**Generated:** No, manually created

**Committed:** Yes

### `docs/`

**Purpose:** Project documentation

**Generated:** No, manually created

**Committed:** Yes

### `internalDoc/`

**Purpose:** Internal development notes

**Generated:** No

**Committed:** Yes

### `src/` (main source)

**Purpose:** All application firmware code

**Generated:** No

**Committed:** Yes

### Hidden Files

- `.cargo/`: Cargo configuration (toolchains, build targets)
- `.embuild/`: ESP-IDF build system (generated, not committed)
- `.git/`: Git repository (generated by git)

---

*Structure analysis: 2026-02-04*
