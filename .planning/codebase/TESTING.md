# Testing Patterns

**Analysis Date:** 2026-02-04

## Test Framework

**Test Runner:** Rust built-in `#[test]` attribute with `cargo test`

**Embedded Test Support:**
- `#[cfg(test)]` attribute gates test modules
- Separate std support for tests: `#![cfg(test)] extern crate std;`

**Configuration:**
- Unstable `build-std` feature enabled in `.cargo/config.toml` for test support
- `test` feature in `Cargo.toml`: `test = ["std"]`

**Run Commands:**
```bash
cargo test              # Run all tests
cargo test --lib        # Run library tests only
cargo test test_name    # Run specific test
```

## Test File Organization

**Location Strategy:**
- Co-located tests within modules using `#[cfg(test)] mod tests { ... }`
- Test modules named: `abstractions_tests.rs`, inline `mod tests` in `parser.rs`, `app_error.rs`

**Naming Convention:**
- Test modules: `mod tests` inside implementation files
- Separate test file pattern: `abstractions_tests.rs` (co-located with `abstractions.rs`)

**Structure:**
```
src/
├── control/
│   ├── abstractions.rs          # Main implementation
│   ├── abstractions_tests.rs    # Tests for abstractions
│   └── handlers.rs              # Implementation
│       └── #[cfg(test)]
│           mod tests { ... }   # Inline tests
├── error/
│   └── app_error.rs
│       └── #[cfg(test)]
│           mod tests { ... }
├── input/
│   └── parser.rs
│       └── #[cfg(test)]
│           mod tests { ... }
```

## Test Suite Organization

### Inline Test Module Pattern

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;
    use crate::output::traits::OutputError;

    // Tests go here
}
```

### Imports in Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;
    use crate::output::traits::OutputError;
}
```

## Mocking Patterns

### Manual Mock Structs

**Mock PidController:**
```rust
struct MockPidController {
    enabled: bool,
    target: f32,
}

impl MockPidController {
    fn new() -> Self {
        Self {
            enabled: false,
            target: DEFAULT_TARGET_TEMP,
        }
    }
}

impl PidController for MockPidController {
    type Error = PidError;

    fn set_target(&mut self, target: f32) -> Result<(), Self::Error> {
        self.target = target;
        Ok(())
    }

    fn enable(&mut self) {
        self.enabled = true;
    }

    fn disable(&mut self) {
        self.enabled = false;
    }

    fn compute_output(&mut self, _current_temp: f32, _current_time: u32) -> f32 {
        if self.enabled {
            50.0  // Fixed output for testing
        } else {
            0.0
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn get_target(&self) -> f32 {
        self.target
    }
}
```

**Mock OutputManager:**
```rust
struct MockOutputManager {
    continuous_enabled: bool,
    process_called: bool,
}

impl MockOutputManager {
    fn new() -> Self {
        Self {
            continuous_enabled: false,
            process_called: false,
        }
    }
}

impl OutputManager for MockOutputManager {
    type Error = OutputError;

    async fn process_status(&mut self, _status: &SystemStatus) -> Result<(), Self::Error> {
        self.process_called = true;
        Ok(())
    }

    fn reset(&mut self) {
        self.continuous_enabled = false;
    }

    fn enable_continuous_output(&mut self) {
        self.continuous_enabled = true;
    }

    fn disable_continuous_output(&mut self) {
        self.continuous_enabled = false;
    }

    fn is_continuous_enabled(&self) -> bool {
        self.continuous_enabled
    }
}
```

### Mock Pattern Characteristics

- **Simple Structs:** Mocks are basic structs with minimal fields
- **Fixed Return Values:** Compute methods return predictable values (50.0, 0.0)
- **State Tracking:** Methods track calls via boolean flags (`process_called`)
- **Trait Implementation:** Mocks implement the same trait as real types
- **Default Initialization:** `new()` constructor with sensible defaults

## Test Structure

### Command Handler Tests

**Safety Command Handler:**
```rust
#[test]
fn test_safety_command_handler_priority() {
    let mut handler = SafetyCommandHandler::new();

    let mut status = SystemStatus::default();
    let current_time = embassy_time::Instant::now();

    let result = handler.handle_command(
        RoasterCommand::EmergencyStop,
        current_time,
        &mut status,
    );

    assert!(result.is_err());
    assert!(handler.is_emergency_active());
    assert!(status.fault_condition);
    assert_eq!(status.ssr_output, 0.0);
    assert!(!status.pid_enabled);
}
```

**Artisan Command Handler:**
```rust
#[test]
fn test_artisan_command_handler() {
    let mut handler = ArtisanCommandHandler::new();

    let mut status = SystemStatus::default();
    let current_time = embassy_time::Instant::now();

    let result = handler.handle_command(
        RoasterCommand::SetHeaterManual(80),
        current_time,
        &mut status,
    );

    assert!(result.is_ok());
    assert_eq!(handler.get_manual_heater(), 80.0);
    assert!(status.artisan_control);

    let result = handler.handle_command(
        RoasterCommand::SetFanManual(60),
        current_time,
        &mut status,
    );

    assert!(result.is_ok());
    assert_eq!(handler.get_manual_fan(), 60.0);
    assert_eq!(status.fan_output, 60.0);
}
```

**System Command Handler:**
```rust
#[test]
fn test_system_command_handler() {
    let mut handler = SystemCommandHandler;

    let mut status = SystemStatus {
        state: RoasterState::Heating,
        bean_temp: 150.0,
        env_temp: 100.0,
        ..Default::default()
    };
    let current_time = embassy_time::Instant::now();

    let result = handler.handle_command(RoasterCommand::Reset, current_time, &mut status);

    assert!(result.is_ok());
    assert_eq!(status.state, RoasterState::Idle);
    assert_eq!(status.bean_temp, 0.0);
    assert_eq!(status.env_temp, 0.0);
}
```

### Error Tests

**Error Categorization:**
```rust
#[test]
fn test_error_categorization() {
    let temp_err = AppError::Temperature {
        message: heapless::String::<256>::try_from("Test").unwrap_or_default(),
        source: TemperatureError::OutOfRange,
    };
    assert_eq!(temp_err.category(), "temperature");
    assert!(!temp_err.is_recoverable());
    assert!(temp_err.requires_emergency_shutdown());
}
```

**Error Conversion:**
```rust
#[test]
fn test_error_conversion() {
    let roaster_err = crate::control::RoasterError::TemperatureOutOfRange;
    let app_err = AppError::from(roaster_err);

    assert!(matches!(app_err, AppError::Temperature { .. }));
    assert!(app_err.requires_emergency_shutdown());
}
```

**User Messages:**
```rust
#[test]
fn test_user_messages() {
    let err = AppError::Temperature {
        message: heapless::String::<256>::try_from("Test").unwrap_or_default(),
        source: TemperatureError::SensorFault,
    };
    assert_eq!(err.user_message(), "Temperature sensor malfunction");
}
```

### Parser Tests

**Command Parsing:**
```rust
#[test]
fn test_parse_read_command() {
    let result = parse_artisan_command("READ");
    assert!(matches!(result, Ok(ArtisanCommand::ReadStatus)));
}

#[test]
fn test_parse_ot1_command() {
    let result = parse_artisan_command("OT1 75");
    assert!(matches!(result, Ok(ArtisanCommand::SetHeater(75))));
}

#[test]
fn test_parse_io3_command() {
    let result = parse_artisan_command("IO3 50");
    assert!(matches!(result, Ok(ArtisanCommand::SetFan(50))));
}

#[test]
fn test_invalid_command() {
    let result = parse_artisan_command("INVALID");
    assert!(matches!(result, Err(ParseError::InvalidCommand)));
}

#[test]
fn test_invalid_value() {
    let result = parse_artisan_command("OT1 150");
    assert!(matches!(result, Err(ParseError::InvalidValue)));
}

#[test]
fn test_empty_command() {
    let result = parse_artisan_command("");
    assert!(matches!(result, Err(ParseError::EmptyCommand)));
}
```

### Mock Tests

```rust
#[test]
fn test_mock_output_manager() {
    let mut output = MockOutputManager::new();

    assert!(!output.is_continuous_enabled());

    output.enable_continuous_output();
    assert!(output.is_continuous_enabled());

    output.process_called = false;
    let status = SystemStatus::default();

    output.process_called = true;

    assert!(output.process_called);

    output.disable_continuous_output();
    assert!(!output.is_continuous_enabled());
}
```

## Assertion Patterns

### Standard Assertions

```rust
assert!(result.is_ok());
assert!(result.is_err());
assert_eq!(handler.get_manual_heater(), 80.0);
assert!(status.artisan_control);
```

### Pattern Matching Assertions

```rust
assert!(matches!(result, Ok(ArtisanCommand::ReadStatus)));
assert!(matches!(app_err, AppError::Temperature { .. }));
assert!(matches!(result, Err(ParseError::InvalidCommand)));
```

### Assert With Messages

```rust
assert!(result.is_ok(), "Expected command to succeed");
assert_eq!(status.ssr_output, 0.0, "SSR should be disabled after emergency stop");
```

## Test Data Setup

### Default System Status

```rust
let status = SystemStatus::default();
```

### Custom System Status

```rust
let mut status = SystemStatus {
    state: RoasterState::Heating,
    bean_temp: 150.0,
    env_temp: 100.0,
    ..Default::default()
};
```

### Time Setup

```rust
let current_time = embassy_time::Instant::now();
```

## Test Scope Categories

### Unit Tests

**Scope:** Individual traits, handlers, parsers
- `test_safety_command_handler_priority` - Safety handler logic
- `test_parse_read_command` - Parser validation
- `test_error_categorization` - Error type behavior

**Pattern:** Direct function/method calls with mock dependencies

### Integration Tests

**Scope:** Handler combinations, error conversions
- `test_artisan_command_handler` - Multiple command handling
- `test_system_command_handler` - State reset behavior

**Pattern:** Multiple handlers, status mutation tracking

### Test Coverage Areas

| Module | Tests Found | Coverage Type |
|--------|-------------|---------------|
| `control/abstractions.rs` | 4 tests | Unit + Integration |
| `error/app_error.rs` | 3 tests | Unit |
| `input/parser.rs` | 7 tests | Unit |
| `control/handlers.rs` | 3+ tests | Integration |

## Fixtures & Factories

### Mock Factories

```rust
impl MockPidController {
    fn new() -> Self {
        Self {
            enabled: false,
            target: DEFAULT_TARGET_TEMP,
        }
    }
}

impl MockOutputManager {
    fn new() -> Self {
        Self {
            continuous_enabled: false,
            process_called: false,
        }
    }
}
```

### Handler Constructors

```rust
impl TemperatureCommandHandler {
    pub fn new() -> Result<Self, RoasterError> { ... }
}

impl SafetyCommandHandler {
    pub fn new() -> Self { ... }
}

impl ArtisanCommandHandler {
    pub fn new() -> Self { ... }
}
```

## Async Testing

### Current State

- Tests use `#[test]` (synchronous)
- Async methods tested through direct calls when possible
- No async test runtime observed in test code

### Pattern for Async

```rust
// Async methods would need embassy-time test support
// Pattern not fully implemented yet
```

## Test Configuration

### Cargo Test Features

```toml
[features]
default = []
std = ["embedded-hal-02"]
test = ["std"]
```

### Test Feature Usage

Tests require `test` feature which enables `std`:
```rust
#[cfg(test)]
extern crate std;
```

## Coverage & Quality

### No Explicit Coverage Enforcement

- No `cargo tarpaulin` or coverage tools configured
- Coverage percentage not tracked
- Manual test design based on functionality

### Test Quality Patterns

**Good Practices Observed:**
- Tests use descriptive names (`test_safety_command_handler_priority`)
- Multiple assertions per test for related behavior
- Clear setup and verify phases
- Uses `matches!` macro for complex assertions

**Improvement Opportunities:**
- No property-based testing observed
- No fuzz testing configured
- No performance/benchmark tests

## Test Execution

### Standard Execution

```bash
cargo test              # All tests
cargo test --lib        # Library tests only
cargo test test_name    # Filtered by name
```

### Embedded Target

Tests run on host with `std` enabled:
- Target: `riscv32imc-unknown-none-elf` (embedded)
- Tests require `test` feature for `std` support
- No hardware-in-loop testing observed

---

*Testing analysis: 2026-02-04*
