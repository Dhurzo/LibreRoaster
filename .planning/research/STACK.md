# Stack Research: v4.5 Refactoring Tasks

**Project:** LibreRoaster (ESP32-C3 Coffee Roaster Firmware)  
**Researched:** 2026-02-28  
**Focus:** Stack additions/changes for v4.5 refactoring tasks (SSR deduplication, test stub unification, heapless migration, command handler delegation)  
**Confidence:** HIGH

---

## Executive Summary

All four v4.5 refactoring tasks can be completed with **zero new dependencies**. The heapless crate (v0.9.2, already in use) provides the `Deque<f32, N>` type needed for Task 3, and the other tasks are pure refactoring requiring no dependency changes.

---

## Recommended Stack

### Current Dependencies (v4.4)

| Category | Current Version | Status |
|----------|-----------------|--------|
| heapless | 0.9.2 | Latest stable (August 2025) |
| embedded-hal | 1.0.0 | Stable |
| embassy-rs | 0.5.0+ | In use |
| portable-atomic | 1.13 | In use |

**Conclusion:** All dependencies are current. No version changes or additions needed.

---

## Task-by-Task Analysis

### Task 1: Extract detect_heat_source() to SsrControlBase

**Current state:**
- `SsrControlBase` exists in `src/hardware/ssr.rs` with common fields (lines 87-94)
- Both `SsrControl` and `SsrControlSimple` have identical `detect_heat_source()` implementations (lines 213-246 and 329-361)
- `HeatSourceDetector` trait exists (lines 98-100) but both structs implement it separately

**Refactoring approach:**
- Move `detect_heat_source()` logic to `SsrControlBase` as a public method
- Both `SsrControl` and `SsrControlSimple` delegate to `self.base.detect_heat_source(current_time)`
- Follow the existing `StatusGetters` pattern already established in v4.4

**Pattern to follow:**
```rust
// SsrControlBase gets the method
impl SsrControlBase {
    pub fn detect_heat_source<D: InputPin>(
        &mut self, 
        detection_pin: &mut D, 
        current_time: u32
    ) -> Result<(), SsrError> {
        // Move existing logic here
    }
}

// SsrControl delegates
impl<'a, PIN, DETECT, PWM> SsrControl<'a, PIN, DETECT, PWM> {
    pub fn detect_heat_source(&mut self, current_time: u32) -> Result<(), SsrError> {
        self.base.detect_heat_source(&mut self.detection_pin, current_time)
    }
}
```

**Stack impact:** None. Pure refactoring.

---

### Task 2: Migrate test stubs from local files to crate::common::*

**Current state:**
- `tests/common/mod.rs` contains `StubHeater`, `StubFan`, `StubThermometer` with call tracking via `RefCell<Vec<T>>`
- Multiple test files define their own local stub implementations (duplicate code)

**Refactoring approach:**
- Create `src/common/mod.rs` that re-exports from test stubs for library-level access
- OR: Move shared stubs to `src/common/` for use by both library and tests

**Existing stub pattern (already working):**
```rust
pub struct StubHeater {
    pub calls: RefCell<Vec<HeaterCall>>,
    pub status: RefCell<SsrHardwareStatus>,
}

impl Heater for StubHeater {
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError> {
        self.calls.borrow_mut().push(HeaterCall::SetPower(duty));
        Ok(())
    }
}
```

**Stack impact:** None. `RefCell` is already in use for test stubs.

---

### Task 3: Replace Vec<f32> with heapless::Deque<f32, 5>

**Current state:**
- `MutableArtisanFormatter` uses `bt_history: Vec<f32>` (line 193 in `src/output/artisan.rs`)
- Manual ring buffer logic: `if history.len() >= 5 { history.remove(0); }` (lines 51-56)

**heapless::Deque API (already available in 0.9.x):**

```rust
use heapless::Deque;

// Create with capacity 5
let mut bt_history: Deque<f32, 5> = Deque::new();

// Replace manual ring buffer:
// Old: if history.len() >= 5 { history.remove(0); } history.push(current_bt);
// New:
if bt_history.len() == bt_history.capacity() {
    bt_history.pop_front(); // Remove oldest element
}
bt_history.push_back(current_bt).unwrap(); // Cannot fail with capacity 5

// Iterate for ROR calculation
for val in &bt_history { /* ... */ }
```

**Required methods (all available in heapless 0.9.x):**
| Method | Purpose |
|--------|---------|
| `new()` | Constructor |
| `push_back()` | Add element to back |
| `pop_front()` | Remove oldest element |
| `len()` | Current count |
| `capacity()` | Maximum capacity |
| `&self` iteration | Front-to-back iteration |

**Migration example:**
```rust
// Before
bt_history: Vec<f32>,

// After  
bt_history: Deque<f32, 5>,
```

**Stack impact:** None. `Deque` is exported by existing heapless 0.9.2 dependency.

---

### Task 4: Refactor process_artisan_command() to delegate to ArtisanCommandHandler

**Current state:**
- `ArtisanCommandHandler` already exists in `src/control/handlers.rs` (lines 217-342)
- `RoasterCommandHandler` trait already defined in `src/control/abstractions.rs` (lines 51-60)
- `RoasterRefactored` has `process_artisan_command()` method

**Refactoring approach:**
- Ensure `RoasterRefactored` stores an `ArtisanCommandHandler` instance
- `process_artisan_command()` delegates to `handler.handle_command(command, current_time, status)`

**Existing pattern:**
```rust
impl RoasterCommandHandler for ArtisanCommandHandler {
    fn handle_command(
        &mut self,
        command: RoasterCommand,
        current_time: Instant,
        status: &mut SystemStatus,
    ) -> Result<(), RoasterError> {
        match command {
            RoasterCommand::SetHeaterManual(value) => { /* ... */ }
            RoasterCommand::SetFanManual(value) => { /* ... */ }
            // ...
        }
    }

    fn can_handle(&self, command: RoasterCommand) -> bool {
        matches!(command, RoasterCommand::SetHeaterManual(_) | /* ... */)
    }
}
```

**Stack impact:** None. All types already exist.

---

## Explicitly NOT Needed

| Dependency | Reason |
|------------|--------|
| New heapless version | 0.9.2 is latest stable |
| `refcell` | Already have `core::cell::RefCell` in std/alloc |
| `parking_lot` | Not needed; critical-section handles embedded concurrency |
| `anyhow` | Error handling uses custom `RoasterError` enum |
| `thiserror` | Manual error implementation already in place |

---

## Integration with Existing Patterns

### Trait Delegation Pattern (v4.4)

The v4.4 SSR extraction established the delegation pattern:

```rust
impl StatusGetters for SsrControlBase {
    fn get_hardware_status(&self) -> SsrHardwareStatus {
        self.hardware_status
    }
}

impl StatusGetters for SsrControl<'a, PIN, DETECT, PWM> {
    fn get_hardware_status(&self) -> SsrHardwareStatus {
        StatusGetters::get_hardware_status(&self.base) // Delegate
    }
}
```

**v4.5 should follow the same pattern** for `detect_heat_source()`.

### Test Stub Pattern (v4.4)

Existing test stubs use `RefCell<Vec<T>>` for interior mutability:

```rust
pub struct StubHeater {
    pub calls: RefCell<Vec<HeaterCall>>,
    pub status: RefCell<SsrHardwareStatus>,
}
```

This pattern works and requires no changes.

---

## Verification: heapless Version

| Source | Version | Status |
|--------|---------|--------|
| crates.io | 0.9.2 | Latest stable (2025-08-20 release) |
| docs.rs | 0.9.2 | Current |
| Cargo.toml | 0.9.2 | Already in use |

**Conclusion:** heapless is already at the latest stable version. No upgrade needed.

---

## Installation

### No new dependencies required

All v4.5 refactoring tasks use existing dependencies:

```toml
# Current Cargo.toml - no changes needed
heapless = "0.9.2"  # Already provides Deque<f32, N>
```

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| heapless Deque availability | HIGH | Verified in 0.9.x docs; matches use case |
| SSR delegation pattern | HIGH | Already established in v4.4 |
| Test stub migration | HIGH | Pattern exists in tests/common/mod.rs |
| Command handler delegation | HIGH | ArtisanCommandHandler already exists |
| No new deps needed | HIGH | All functionality available in current stack |

---

## Sources

- [heapless crate documentation](https://docs.rs/heapless/latest/heapless/deque/index.html) — Deque API confirmed in 0.9.x
- [heapless v0.9.1 release announcement](https://blog.rust-embedded.org/heapless-091/) — Latest stable release (August 2025)
- Code analysis:
  - `src/hardware/ssr.rs` — SsrControlBase, detect_heat_source() implementations
  - `src/output/artisan.rs` — Vec<f32> bt_history usage
  - `tests/common/mod.rs` — Existing test stub pattern
  - `src/control/handlers.rs` — ArtisanCommandHandler implementation

---

_*Stack research for: LibreRoaster v4.5 refactoring tasks*_
_*Researched: 2026-02-28*_
