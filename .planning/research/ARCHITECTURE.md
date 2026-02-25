# Architecture Research: SSR Refactoring and Test Infrastructure

**Project:** LibreRoaster v4.4 SSR Refactoring
**Researched:** 2026-02-24
**Confidence:** HIGH

## Executive Summary

This architecture research addresses the SSR refactoring and shared test infrastructure for LibreRoaster v4.4. The current codebase has significant code duplication between `SsrControl` and `SsrControlSimple` structs, and test helpers are scattered across individual test files rather than being centralized.

The recommended approach is to extract a common `SsrControlBase` struct that holds all shared state and logic, with `SsrControl` and `SsrControlSimple` as thin wrappers that add their specific pin handling. For test infrastructure, a `tests/common/mod.rs` module should provide reusable stubs that eliminate the ~5x duplication observed in current inline mock implementations.

---

## Current Architecture

### SSR Control Components

```
src/hardware/ssr.rs
├── SsrError                     # Error enum (OutputError, InputError, HeatSourceNotDetected, PwmError)
├── SsrHardwareStatus            # Status enum (Available, NotDetected, Error)
├── LedcDutyReader               # Trait for PWM duty readback
├── percentage_to_ledc_duty()   # Conversion helper
├── monitor_ledc_after_set()    # Drift detection and retry logic
├── SsrControl<'a, PIN, DETECT, PWM>    # Full SSR with enable pin
└── SsrControlSimple<'a, DETECT, PWM>   # Simplified SSR without enable pin
```

### Duplicate Code Analysis

Both `SsrControl` and `SsrControlSimple` implement nearly identical methods:

| Method | SsrControl | SsrControlSimple | Duplication |
|--------|-------------|-------------------|-------------|
| `detect_heat_source()` | Lines 150-183 | Lines 298-331 | ~95% identical |
| `periodic_check()` | Lines 185-197 | Lines 333-345 | Identical |
| `get_hardware_status()` | Lines 199-201 | Lines 347-349 | Identical |
| `is_heating_available()` | Lines 203-205 | Lines 351-353 | Identical |
| `set_percentage()` | Lines 207-234 | Lines 355-382 | ~90% identical |
| `get_current_duty()` | Lines 236-238 | Lines 384-386 | Identical |
| `is_pwm_enabled()` | Lines 240-242 | Lines 388-390 | Identical |
| `last_lead_delta_ticks()` | Lines 244-246 | Lines 392-394 | Identical |
| `last_retry_count()` | Lines 248-250 | Lines 396-398 | Identical |
| `Heater` impl | Lines 435-461 | Lines 401-426 | Identical |

### Control Layer Integration

```
src/control/
├── traits.rs              # Heater, Fan, Thermometer traits
├── abstractions.rs       # RoasterError, PidController, RoasterCommandHandler
├── ssr_scheduler.rs      # SsrCycleGuard
├── roaster_refactored.rs # RoasterControl using Box<dyn Heater>
└── mod.rs                # Exports
```

The `Heater` trait (lines 16-28 in `traits.rs`) is the abstraction point:

```rust
pub trait Heater: Send {
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError>;
    fn get_status(&self) -> SsrHardwareStatus;
    fn last_duty_delta_ticks(&self) -> i16 { 0 }
    fn last_retry_count(&self) -> u8 { 0 }
}
```

`RoasterControl` uses `Box<dyn Heater + Send>` to hold the heater implementation, allowing runtime polymorphism.

### Test Infrastructure Current State

```
tests/
├── mock_uart.rs              # MockUartDriver (432 lines)
├── mock_usb_driver.rs        # MockUsbCdcDriver (668 lines)  
├── ssr_monitor.rs           # Inline FakeDetectPin, FakeLedcChannel (91 lines)
├── ssr_scheduler.rs          # Uses real SsrCycleGuard, no mocks
└── [other tests]            # Each has inline stub implementations
```

The duplication pattern: each test file that needs a heater/fan stub defines its own mock types rather than reusing shared implementations.

---

## Recommended Architecture

### SSR Refactoring: Base Struct Pattern

The recommended approach extracts common state into a `SsrControlBase` struct:

```
src/hardware/ssr.rs (refactored)
├── SsrError
├── SsrHardwareStatus
├── LedcDutyReader trait
├── percentage_to_ledc_duty()
├── monitor_ledc_after_set()
├── SsrControlBase<'a, DETECT, PWM>    # NEW: Common state and logic
│   ├── detection_pin: DETECT
│   ├── pwm_channel: PWM
│   ├── hardware_status: SsrHardwareStatus
│   ├── current_duty: u16
│   ├── last_duty_delta_ticks: i16
│   ├── retry_count: u8
│   ├── last_detection_check: Option<u32>
│   ├── is_pwm_enabled: bool
│   └── Methods: detect_heat_source, periodic_check, set_percentage, getters
├── SsrControl<'a, PIN, DETECT, PWM>   # Wraps Base + enable pin
│   ├── pin: PIN                        # Enable pin (stored but not used)
│   └── base: SsrControlBase
└── SsrControlSimple<'a, DETECT, PWM>  # Wraps Base only
    └── base: SsrControlBase
```

**Benefits:**
- Single source of truth for SSR control logic
- Easier to maintain, test, and extend
- Trait implementations delegate to base
- No runtime overhead (zero-cost abstraction)

### Test Infrastructure: Shared Stubs Module

```
tests/common/mod.rs          # NEW: Shared test infrastructure
├── StubHeater               # Implements Heater trait
├── StubFan                  # Implements Fan trait  
├── StubThermometer          # Implements Thermometer trait
├── StubAsyncThermometer     # Implements AsyncThermometer trait
├── reset_channels()         # Helper to reset test channels
├── collect_output()         # Helper to collect queued output
└── TestChannels             # Shared channel storage for tests
```

**Updated test files would import from common:**

```rust
// Before (ssr_monitor.rs)
struct FakeDetectPin;
impl InputPin for FakeDetectPin { ... }

struct FakeLedcChannel;
impl LedcDutyReader for FakeLedcChannel { ... }

// After
use tests_common::mocks::{FakeDetectPin, FakeLedcChannel};
```

---

## Integration Points

### 1. Hardware Module Integration

| Component | File | Integration Point |
|-----------|------|------------------|
| SSR Base | `src/hardware/ssr.rs` | New `SsrControlBase` struct |
| SSR Full | `src/hardware/ssr.rs` | Delegates to base, adds enable pin |
| SSR Simple | `src/hardware/ssr.rs` | Delegates to base |
| Heater trait | `src/control/traits.rs` | Unchanged, implementations delegate |

### 2. Control Layer Integration

| Component | File | Integration Point |
|-----------|------|------------------|
| RoasterControl | `src/control/roaster_refactored.rs` | Uses `Box<dyn Heater>` - no changes needed |
| ServiceContainer | `src/application/service_container.rs` | No changes needed |

### 3. Test Infrastructure Integration

| Component | File | Integration Point |
|-----------|------|------------------|
| Test stubs | `tests/common/mod.rs` | New module, exports `StubHeater`, `StubFan`, etc. |
| Existing tests | `tests/*.rs` | Refactor to use shared stubs |

---

## Data Flow

### SSR Control Flow (Unchanged by Refactoring)

```
Artisan Command (OT1 75)
    ↓
RoasterControl::handle_command()
    ↓
Heater::set_power(duty)         ← Trait abstraction
    ↓
SsrControlBase::set_percentage()
    ↓
PWM channel set_duty() + monitor_ledc_after_set()
    ↓
SystemStatus updated with hardware status
```

The refactoring preserves this flow; only the internal SSR implementation changes.

### Test Flow with Shared Stubs

```
Test
    ↓
Create StubHeater with configured behavior
    ↓
Inject into RoasterControl (or test component)
    ↓
Execute test actions
    ↓
Assert on StubHeater state / output channels
    ↓
reset_channels() for next test
```

---

## Build Order and Dependencies

### Phase 1: SSR Refactoring

1. **Create `SsrControlBase`** in `src/hardware/ssr.rs`
   - Move shared state fields from both structs
   - Move `detect_heat_source()`, `periodic_check()`, `set_percentage()` implementations
   - Move getter methods

2. **Refactor `SsrControlSimple`** to use base
   - Replace inline implementation with delegation to base
   - Keep `Heater` impl that calls

3. **Refactor `SsrControl`** to use base
   - Replace inline implementation with delegation to base base methods
   - Keep enable pin storage (structural requirement)
   - Keep `Heater` impl that calls base methods

4. **Verify compilation**
   ```bash
   cargo check --target riscv32
   # host tests
   cargo check
   ```

5. **Run existing tests**
   ```bash
   cargo test
   ```

### Phase 2: Test Infrastructure

1. **Create `tests/common/mod.rs`**
   - Define `StubHeater`, `StubFan`, `StubThermometer`
   - Include `reset_channels()`, `collect_output()` helpers

2. **Migrate `ssr_monitor.rs` to use shared stubs**
   - Remove inline `FakeDetectPin`, `FakeLedcChannel`
   - Import from `tests_common`

3. **Migrate other test files**
   - Identify tests using inline stubs
   - Refactor to use shared implementations

4. **Run full test suite**
   ```bash
   cargo test
   ```

---

## Component Summary

### New Components

| Component | Location | Purpose |
|-----------|----------|---------|
| `SsrControlBase` | `src/hardware/ssr.rs` | Shared SSR state and logic |
| `tests/common/mod.rs` | `tests/common/mod.rs` | Shared test stubs |

### Modified Components

| Component | Change Type | Purpose |
|-----------|-------------|---------|
| `SsrControl` | Refactor | Delegate to base |
| `SsrControlSimple` | Refactor | Delegate to base |
| `tests/ssr_monitor.rs` | Refactor | Use shared stubs |
| Other test files | Refactor | Use shared stubs |

### Unchanged Components

| Component | Reason |
|-----------|--------|
| `Heater` trait | Already correct abstraction |
| `RoasterControl` | Works with trait |
| `ServiceContainer` | No SSR-specific changes needed |

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| SSR refactoring pattern | HIGH | Base struct pattern is well-established in Rust, no technical blockers |
| Test infrastructure design | HIGH | Mirrors existing patterns in mock_uart.rs and mock_usb_driver.rs |
| Integration with control layer | HIGH | Trait abstraction already in place, no changes needed |
| Build order | HIGH | Clear dependency chain, can verify at each step |

---

## Risks and Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| Breaking existing SSR behavior | Medium | Keep public API identical, verify all tests pass |
| Trait method signature changes | Low | Heater trait is stable; base struct is internal |
| Test migration effort | Low | Can do incrementally, verify after each file |

---

## Sources

- Current implementation: `src/hardware/ssr.rs` (lines 85-473)
- Heater trait: `src/control/traits.rs` (lines 16-28)
- RoasterControl usage: `src/control/roaster_refactored.rs` (lines 30-31, 64)
- Mock patterns: `tests/mock_uart.rs`, `tests/mock_usb_driver.rs`

---

*Architecture research for: LibreRoaster v4.4 SSR Refactoring*  
*Researched: 2026-02-24*
