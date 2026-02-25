# Stack Research: SSR Refactoring and Test Infrastructure

**Project:** LibreRoaster (ESP32-C3 Coffee Roaster Firmware)
**Researched:** February 2026
**Focus:** Rust patterns for trait-based code deduplication and embedded test infrastructure
**Confidence:** HIGH

---

## Executive Summary

For the SSR refactoring milestone, the recommended approach is **composition + trait default implementations** rather than inheritance-like patterns. Extract common state into `SsrState`, define a trait with default implementations, and have both `SsrControl` and `SsrControlSimple` embed the shared state while implementing the trait.

For test infrastructure, create a `tests/common/mod.rs` module with manual stub implementations that implement the existing `control::traits` (Heater, Fan, Thermometer). No external crates needed—use `RefCell` for interior mutability in test stubs.

---

## Recommended Stack

### SSR Refactoring: Composition + Trait Pattern

| Approach | Implementation | Why |
|----------|---------------|-----|
| **Primary** | Extract `SsrState` struct with common fields | Zero-cost abstraction, embeds shared state in both types |
| **Secondary** | Define `SsrControlTrait` with default implementations | Eliminates duplicate method implementations |
| **Existing** | Keep `Heater` trait from `control::traits` | Already provides `set_power` abstraction used by roaster control |

### Test Infrastructure: Shared Stubs Module

| Component | Location | Implementation |
|-----------|----------|----------------|
| **Test stubs** | `tests/common/mod.rs` | Manual struct implementations (no external crate needed) |
| **Stub patterns** | StubHeater, StubFan, StubThermometer | Implement existing `control::traits` traits |
| **Helper utilities** | `reset_channels()`, `collect_output()` | Module-level functions for test state management |

### Alternative Crates Considered

| Crate | Why Not |
|-------|---------|
| `faux` | Requires unsafe for mocks; adds proc-macro complexity; overkill for simple stubs |
| `mockall` | Requires nightly or complex setup; better for external trait mocking |
| `embedded-hal-mock` | Targets embedded-hal trait mocking; our stubs need to implement our own traits |
| `inherit_methods_macro` | Adds build complexity; manual delegation is clear enough here |
| `isotest` | Useful for verifying trait impls but adds dependency; manual approach is sufficient |

---

## Recommended Pattern: SSR Refactoring

### Strategy: Extract Common State via Composition

The current `SsrControl` and `SsrControlSimple` share ~90% identical code. The recommended approach:

```rust
// Step 1: Extract shared state into a base struct (no trait needed)
pub struct SsrState {
    pub(crate) hardware_status: SsrHardwareStatus,
    pub(crate) current_duty: u16,
    pub(crate) last_duty_delta_ticks: i16,
    pub(crate) retry_count: u8,
    pub(crate) last_detection_check: Option<u32>,
    pub(crate) is_pwm_enabled: bool,
}

impl SsrState {
    pub fn new() -> Self {
        Self {
            hardware_status: SsrHardwareStatus::NotDetected,
            current_duty: 0,
            last_duty_delta_ticks: 0,
            retry_count: 0,
            last_detection_check: None,
            is_pwm_enabled: true,
        }
    }

    // Common getter/setter implementations
    pub fn get_hardware_status(&self) -> SsrHardwareStatus { ... }
    pub fn is_heating_available(&self) -> bool { ... }
    pub fn get_current_duty(&self) -> u16 { ... }
    // etc.
}

// Step 2: Define trait with default implementations for shared behavior
pub trait SsrControlTrait {
    fn state(&self) -> &SsrState;
    fn state_mut(&mut self) -> &mut SsrState;

    // Default implementations delegate to shared state
    fn detect_heat_source(&mut self, current_time: u32) -> Result<(), SsrError> {
        let detection_pin = self.get_detection_pin(); // requires impl
        match detection_pin.is_low() { ... }
    }

    fn periodic_check(&mut self, current_time: u32) -> Result<(), SsrError> {
        let should_check = self.state().last_detection_check
            .map(|last| current_time.saturating_sub(last) >= HEAT_SOURCE_CHECK_INTERVAL_MS)
            .unwrap_or(true);
        if should_check {
            self.detect_heat_source(current_time)?;
        }
        Ok(())
    }

    // Getters with default implementations
    fn get_hardware_status(&self) -> SsrHardwareStatus { self.state().hardware_status }
    fn is_heating_available(&self) -> bool { self.state().hardware_status == SsrHardwareStatus::Available }
    fn get_current_duty(&self) -> u16 { self.state().current_duty }
    // etc.
}

// Step 3: Both structs embed SsrState and implement the trait
pub struct SsrControl<'a, PIN, DETECT, PWM> {
    pin: PIN,  // Only SsrControl has this
    detection_pin: DETECT,
    pwm_channel: PWM,
    state: SsrState,
}

pub struct SsrControlSimple<'a, DETECT, PWM> {
    detection_pin: DETECT,
    pwm_channel: PWM,
    state: SsrState,
}
```

### Why This Approach

1. **Zero runtime cost**: No dynamic dispatch, no heap allocation
2. **Clear ownership**: The `pin` field stays with `SsrControl` where it belongs
3. **DRY**: Shared logic in one place, updated once
4. **Trait polymorphism available**: If needed later, `SsrControlTrait` enables `dyn` usage
5. **Idiomatic Rust**: Follows "composition over inheritance" principle

### What NOT to Do

| Anti-pattern | Why Avoid |
|--------------|-----------|
| Create a base struct with inheritance | Rust doesn't have inheritance; forces awkward patterns |
| Use `dyn SsrControlTrait` in hot paths | Dynamic dispatch adds cost; embedded Rust prefers static dispatch |
| Duplicate the methods in each struct | Creates maintenance burden, drift risk |
| Make SsrState public fields | Breaks encapsulation; use getters/setters |

---

## Recommended Pattern: Test Infrastructure

### Structure: `tests/common/mod.rs`

```
tests/
├── common/
│   ├── mod.rs              # Re-exports, helper functions
│   ├── stub_heater.rs      # StubHeater implementation
│   ├── stub_fan.rs         # StubFan implementation
│   └── stub_thermometer.rs # StubThermometer implementation
├── ssr_monitor.rs          # Existing tests (will use common stubs)
└── ...
```

### Module Content: `tests/common/mod.rs`

```rust
//! Shared test stubs and utilities for LibreRoaster integration tests.
//!
//! Provides test doubles for hardware abstractions:
//! - `StubHeater` - implements `control::traits::Heater`
//! - `StubFan` - implements `control::traits::Fan`  
//! - `StubThermometer` - implements `control::traits::Thermometer`
//!
//! # Usage
//!
//! ```rust
//! use libreroaster::control::traits::{Heater, Fan, Thermometer};
//! use tests::common::{StubHeater, StubFan, StubThermometer};
//!
//! let mut heater = StubHeater::new();
//! heater.set_power(50.0).unwrap();
//! assert_eq!(heater.get_status(), SsrHardwareStatus::Available);
//! ```

mod stub_heater;
mod stub_fan;
mod stub_thermometer;

pub use stub_heater::StubHeater;
pub use stub_fan::StubFan;
pub use stub_thermometer::StubThermometer;

// ============================================================================
// Helper Functions
// ============================================================================

use core::cell::RefCell;
use core::collections::VecDeque;

/// Global test output collector for串接 integration tests.
/// 
/// Thread-local storage using RefCell for single-threaded test contexts.
/// Reset between tests to ensure isolation.
static TEST_OUTPUT: RefCell<VecDeque<String>> = RefCell::new(VecDeque::new());

/// Reset all test channels - call between tests to ensure isolation.
/// 
/// Clears:
/// - Output buffer
/// - Call history in all stubs
/// - Any accumulated state
pub fn reset_channels() {
    TEST_OUTPUT.borrow_mut().clear();
    StubHeater::reset_history();
    StubFan::reset_history();
    StubThermometer::reset_history();
}

/// Collect all output strings into a single String.
/// 
/// Useful for verifying complete command → response flows.
pub fn collect_output() -> String {
    TEST_OUTPUT
        .borrow_mut()
        .drain(..)
        .collect::<Vec<_>>()
        .join("")
}

/// Push a string to the test output buffer.
/// 
/// Internal helper for stubs to record their actions.
pub fn push_output(s: &str) {
    TEST_OUTPUT.borrow_mut().push_back(s.to_string());
}
```

### Stub Implementation Pattern

```rust
// tests/common/stub_heater.rs

use core::cell::RefCell;
use crate::config::constants::SsrHardwareStatus;
use crate::control::{traits::Heater, RoasterError};

/// Static call history for verification
static CALL_HISTORY: RefCell<Vec<HeaterCall>> = RefCell::new(Vec::new());

#[derive(Debug, Clone)]
enum HeaterCall {
    SetPower(f32),
    GetStatus,
    LastDelta,
    LastRetry,
}

pub struct StubHeater {
    power: f32,
    status: SsrHardwareStatus,
    last_delta: i16,
    last_retry: u8,
}

impl StubHeater {
    pub fn new() -> Self {
        Self {
            power: 0.0,
            status: SsrHardwareStatus::Available,
            last_delta: 0,
            last_retry: 0,
        }
    }

    pub fn with_status(mut self, status: SsrHardwareStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_last_delta(mut self, delta: i16) -> Self {
        self.last_delta = delta;
        self
    }

    pub fn reset_history() {
        CALL_HISTORY.borrow_mut().clear();
    }

    pub fn get_call_history() -> Vec<HeaterCall> {
        CALL_HISTORY.borrow().clone()
    }
}

// Implement the trait - this is what makes it useful for testing
impl Heater for StubHeater {
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError> {
        CALL_HISTORY.borrow_mut().push(HeaterCall::SetPower(duty));
        self.power = duty.clamp(0.0, 100.0);
        Ok(())
    }

    fn get_status(&self) -> SsrHardwareStatus {
        CALL_HISTORY.borrow_mut().push(HeaterCall::GetStatus);
        self.status
    }

    fn last_duty_delta_ticks(&self) -> i16 {
        CALL_HISTORY.borrow_mut().push(HeaterCall::LastDelta);
        self.last_delta
    }

    fn last_retry_count(&self) -> u8 {
        CALL_HISTORY.borrow_mut().push(HeaterCall::LastRetry);
        self.last_retry
    }
}
```

### Integration with Existing Code

The key insight is that stubs implement **the same traits** the real hardware uses:

```rust
// In your roaster control code, you likely have:
fn control_loop(heater: &mut impl Heater, thermometer: &mut impl Thermometer) {
    let temp = thermometer.read_temperature().unwrap();
    // ... control logic
    heater.set_power(duty).unwrap();
}

// In tests, just pass the stubs:
#[test]
fn test_control_loop() {
    let mut heater = StubHeater::new();
    let mut thermo = StubThermometer::with_temperature(150.0);
    
    control_loop(&mut heater, &mut thermo);
    
    assert_eq!(heater.get_call_history(), ...);
}
```

### What NOT to Add

| Anti-pattern | Why Avoid |
|--------------|-----------|
| External mock crates (mockall, faux) | Adds build complexity; manual stubs are simple enough |
| `unsafe` in test stubs | Unnecessary; RefCell provides interior mutability |
| Async test stubs | Embedded code uses sync traits for hardware; keep it simple |
| Complex verification frameworks | Simple call history is sufficient; don't over-engineer |

---

## Installation

### For SSR Refactoring
No new dependencies required. The refactoring uses only stdlib features.

### For Test Infrastructure
No new dependencies required. The stub pattern uses:
- `core::cell::RefCell` for interior mutability (already in std)
- Existing trait implementations (from `control::traits`)

Optional if you want more sophisticated testing later:
```toml
[dev-dependencies]
# Only if needed - manual stubs are sufficient for now
# embedded-hal-mock = "0.4"  # For testing embedded-hal drivers
```

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| SSR trait pattern | HIGH | Well-established Rust pattern; matches existing Heater trait usage |
| Test stub structure | HIGH | Follows existing test patterns in codebase (see tests/ssr_monitor.rs) |
| Integration approach | HIGH | Stubs implement existing traits; should integrate cleanly |
| No external deps needed | MEDIUM | Manual approach chosen over crates; could change if complexity grows |

---

## Sources

- **Composition over inheritance**: https://www.oreateai.com/blog/analysis-of-three-typical-patterns-for-implementing-inheritance-in-rust/
- **Stack Overflow discussion on trait deduplication**: https://stackoverflow.com/questions/78926546/how-to-avoid-duplicate-code-when-i-impl-a-trait-for-many-structs-in-rust
- **embedded-hal-mock crate**: https://docs.rs/embedded-hal-mock/latest/embedded-hal_mock
- **faux crate for mocking**: https://docs.rs/faux/latest/faux
- **Existing ssr_monitor.rs test**: tests/ssr_monitor.rs (contains FakeDetectPin, FakeLedcChannel)
- **Existing MockUartDriver**: tests/mock_uart.rs (full example of manual stub implementation)

---

_*Stack research for: LibreRoaster SSR refactoring and test infrastructure milestone*_
_*Researched: 2026-02-24*_
