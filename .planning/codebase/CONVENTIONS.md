# Coding Conventions

**Analysis Date:** 2026-02-04

## Language & Edition

**Primary Language:**
- Rust 2021 Edition (specified in `Cargo.toml`)

**Target:**
- riscv32imc-unknown-none-elf (ESP32-C3 embedded target)
- Rust stable toolchain with `rust-src` component

## Naming Patterns

### Files

**Modules:**
- Single-word lowercase: `src/control/`, `src/output/`, `src/hardware/`
- Module files named after module: `src/control/mod.rs`

**Source Files:**
- Snake_case for files: `max31856.rs`, `pid_controller.rs`
- Feature-specific: `abstractions_tests.rs` (test co-located with module)

**Constants:**
- SCREAMING_SNAKE_CASE for constants in `src/config/constants.rs`:
  ```rust
  pub const FAN_PWM_FREQUENCY_HZ: u32 = 25000;
  pub const DEFAULT_TARGET_TEMP: f32 = 225.0;
  pub const PID_SAMPLE_TIME_MS: u32 = 100;
  ```

### Types

**Structs:**
- PascalCase: `CoffeeRoasterPid`, `ArtisanFormatter`, `SystemStatus`
- Descriptive names: `ServiceContainer`, `TemperatureCommandHandler`

**Enums:**
- PascalCase: `RoasterError`, `OutputError`, `Max31856Error`, `PidError`
- Variants: PascalCase with descriptive names:
  ```rust
  pub enum RoasterError {
      TemperatureOutOfRange,
      SensorFault,
      InvalidState,
      PidError,
      HardwareError,
      EmergencyShutdown,
  }
  ```

**Traits:**
- PascalCase: `PidController`, `OutputManager`, `OutputFormatter`
- Descriptive purpose: `RoasterCommandHandler`, `SerialOutput`

### Functions & Methods

**Functions:**
- Snake_case: `new()`, `set_target()`, `compute_output()`, `enable_continuous_output()`

**Methods:**
- Getter-style: `get_target()`, `get_manual_heater()`, `is_enabled()`
- Action verbs: `handle_command()`, `process_status()`, `reset()`
- Async methods: `read_command()`, `should_print()`, `send_response()`

### Variables

**Local Variables:**
- Snake_case: `target_temp`, `bean_temp`, `fan_speed`
- Descriptive names for clarity: `heat_detection_pin`, `manual_heater_value`

**Fields in Structs:**
- Snake_case: `pub state`, `pub bean_temp`, `pub ssr_output`
- Prefixed with `_` for unused parameters: `fn compute_output(&mut self, current_temp: f32, _current_time: u32)`

## Code Style

### Formatting

**Indentation:** Standard Rust indentation (4 spaces typical)

**Line Length:** Not explicitly configured - follow standard Rust practices

**Blank Lines:**
- Two blank lines between module-level items
- One blank line between function definitions
- No blank lines within function bodies (except logical separation)

### Imports & Module Structure

**Crate Imports:**
```rust
// Standard library (for no_std, this is empty)
extern crate alloc;

// External dependencies
use embassy_time::{Duration, Timer};
use log::{info, warn};
use esp_hal::gpio::{Input, Level, Output};

// Internal
use libreroaster::application::AppBuilder;
use libreroaster::hardware::ssr::SsrControlSimple;
```

**Module Organization:**
- Barrel exports in `mod.rs` files
- Separate modules by domain: `control/`, `output/`, `hardware/`, `input/`, `error/`, `application/`, `config/`

### Attributes

**Conditional Compilation:**
```rust
#![cfg(not(test))]
#![no_std]
#![no_main]

#[cfg(test)]
extern crate std;
```

**Allow/Deny Attributes:**
```rust
#[deny(clippy::mem_forget)]
#[deny(clippy::large_stack_frames)]
#[allow(clippy::large_stack_frames, reason = "...")]
#[allow(static_mut_refs)]
```

## Error Handling

### Error Type Pattern

**Module-Specific Errors (New Pattern):**
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum AppError {
    Temperature { message: heapless::String<256>, source: TemperatureError },
    Control { source: ControlError },
    Hardware { source: HardwareError },
    Communication { source: CommunicationError },
    Initialization { source: InitError },
    Safety { severity: SafetyLevel },
    Configuration { source: ConfigError },
}
```

**Legacy Errors:**
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum RoasterError {
    TemperatureOutOfRange,
    SensorFault,
    InvalidState,
    PidError,
    HardwareError,
    EmergencyShutdown,
}
```

### Error Display Implementation

```rust
impl core::fmt::Display for RoasterError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RoasterError::TemperatureOutOfRange => write!(f, "Temperature out of range"),
            RoasterError::SensorFault => write!(f, "Sensor fault"),
            // ...
        }
    }
}
```

### Error Conversion (From Trait)

```rust
impl From<Max31856Error> for RoasterError {
    fn from(e: Max31856Error) -> Self {
        match e {
            Max31856Error::CommunicationError => RoasterError::SensorFault,
            Max31856Error::FaultDetected => RoasterError::SensorFault,
            Max31856Error::InvalidTemperature => RoasterError::TemperatureOutOfRange,
        }
    }
}
```

### Error Methods

Errors include helper methods for categorization:

```rust
impl AppError {
    pub fn is_recoverable(&self) -> bool { ... }
    pub fn requires_emergency_shutdown(&self) -> bool { ... }
    pub fn category(&self) -> &'static str { ... }
    pub fn user_message(&self) -> &'static str { ... }
}
```

### Result & Panic Usage

**Result for Recoverable Errors:**
- Hardware operations return `Result<T, Error>`
- Initialization returns `Result<Self, Error>`
- File: `src/hardware/max31856.rs`:
  ```rust
  pub fn new(spi: SPI) -> Result<Self, Max31856Error> { ... }
  pub fn read_temperature(&mut self) -> Result<f32, Max31856Error> { ... }
  ```

**Unwrap for Known-Good States:**
- Used for initialization that should never fail:
  ```rust
  let app = AppBuilder::new()
      .with_uart(peripherals.UART0)
      .build()
      .expect("Failed to build application");
  ```

**Panic in Critical Sections:**
- Only in main entry after all initialization

## Trait & Interface Design

### Trait Definition Pattern

```rust
pub trait RoasterCommandHandler {
    fn handle_command(
        &mut self,
        command: RoasterCommand,
        current_time: Instant,
        status: &mut SystemStatus,
    ) -> Result<(), RoasterError>;

    fn can_handle(&self, command: RoasterCommand) -> bool;
}
```

### Generic Associated Types

```rust
pub trait OutputManager {
    type Error;
    
    async fn process_status(&mut self, status: &crate::config::SystemStatus) 
        -> Result<(), Self::Error>;
}
```

## Async/Await Patterns

### Async Trait Methods

Uses `#[allow(async_fn_in_trait)]` for compatibility:
```rust
pub trait PrintScheduler {
    #[allow(async_fn_in_trait)]
    async fn should_print(&mut self) -> bool;
}

pub trait SerialOutput {
    #[allow(async_fn_in_trait)]
    async fn print(&mut self, data: &str) -> Result<(), OutputError>;
}
```

### Embassy Integration

- Uses `embassy_time::Instant` for timestamps
- Uses `embassy_executor::Spawner` for task management
- Async/await for non-blocking operations

## Memory Management

### No-STD Support

```rust
#![no_std]
extern crate alloc;
```

### Static Allocation

Uses `static_cell::StaticCell` for static initialization:
```rust
static SPI_BUS: StaticCell<critical_section::Mutex<RefCell<Spi<...>>>> = StaticCell::new();
let spi_mutex = SPI_BUS.init(critical_section::Mutex::new(RefCell::new(spi)));
```

### Heapless Collections

Uses `heapless` for stack-allocated collections:
```rust
use heapless::String;
use heapless::Vec;

let msg: heapless::String<256> = ...;
let history: Vec<f32, 5> = Vec::new();
```

## Comments & Documentation

### Module-Level Docs

```rust
//! Unified error handling system for LibreRoaster
//! Provides consistent error types and handling strategies across all modules
```

### Inner Comments

Descriptive comments for complex operations:
```rust
// Configure Timer0 for Fan (25kHz)
let mut fan_timer = ledc.timer(timer::Number::Timer0);

// SPI2 configured at 1MHz for MAX31856
let spi_config = Config::default().with_frequency(esp_hal::time::Rate::from_khz(1000));
```

### TODO Comments

Located in code (grep results show patterns):
- `// TODO: ...` for deferred work
- `// FIXME: ...` for known issues
- `// Note: ...` for implementation notes

## Builder Pattern

App initialization uses Builder pattern in `src/application/app_builder.rs`:
```rust
let app = AppBuilder::new()
    .with_uart(peripherals.UART0)
    .with_real_ssr(static_ssr)
    .with_fan_control(static_fan)
    .with_temperature_sensors(bean_sensor, env_sensor)
    .with_formatter(ArtisanFormatter::new())
    .build()
    .expect("Failed to build application");
```

## Cargo Configuration

**Target:** riscv32imc-unknown-none-elf via `.cargo/config.toml`

**Compiler Flags:**
```toml
[env]
ESP_LOG="info"
TEST_ENV="1"

[build]
rustflags = [
  "-C", "force-frame-pointers",
]
```

**Unstable Features:**
```toml
[unstable]
build-std = ["alloc", "core", "test"]
```

---

*Convention analysis: 2026-02-04*
