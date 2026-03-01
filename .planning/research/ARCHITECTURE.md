# Architecture Research: v4.5 Refactoring Tasks

**Project:** LibreRoaster v4.5 Refactoring
**Researched:** 2026-02-28
**Confidence:** HIGH

## Executive Summary

This architecture research addresses four refactoring tasks for LibreRoaster v4.5 that build upon the v4.4 foundation:

1. **SSR Control Enhancement**: Extend the `SsrControlBase` pattern with formal trait delegation
2. **Test Infrastructure Usage**: Integrate existing `tests/common/mod.rs` stubs
3. **Formatter Optimization**: Replace `Vec<f32>` BT history with fixed-size array for embedded compatibility
4. **Command Processing Refactor**: Decompose large match statement in `roaster_refactored.rs`

All refactoring tasks integrate with the existing v4.4 architecture without requiring fundamental architectural changes.

---

## Current Architecture (v4.4 Baseline)

### System Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Application Layer                             │
├─────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │              RoasterControl (roaster_refactored.rs)          │  │
│  │  - State management (RoasterState, SystemStatus)           │  │
│  │  - Command processing (process_command)                      │  │
│  │  - PID control coordination                                  │  │
│  └──────────────────────────┬────────────────────────────────────┘  │
├─────────────────────────────┴────────────────────────────────────────┤
│                        Control Layer                                  │
├─────────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐   │
│  │ Temperature  │  │   Safety     │  │     Artisan          │   │
│  │   Handler    │  │   Handler    │  │     Handler          │   │
│  └──────┬───────┘  └──────┬───────┘  └──────────┬───────────┘   │
│         │                 │                      │                │
│  ┌──────┴─────────────────┴──────────────────────┴───────────┐  │
│  │              Heater Trait (traits.rs)                        │  │
│  │  - set_power(duty) -> Result                                │  │
│  │  - get_status() -> SsrHardwareStatus                        │  │
│  └──────────────────────────┬───────────────────────────────────┘  │
├─────────────────────────────┴────────────────────────────────────────┤
│                        Hardware Layer                                 │
├─────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │           SsrControlBase (hardware/ssr.rs)                  │  │
│  │  ┌─────────────────────────────────────────────────────┐   │  │
│  │  │ SsrControl<'a, PIN, DETECT, PWM>                    │   │  │
│  │  │ SsrControlSimple<'a, DETECT, PWM>                   │   │  │
│  │  └─────────────────────────────────────────────────────┘   │  │
│  └─────────────────────────────────────────────────────────────┘  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐   │
│  │    Fan       │  │   Sensors    │  │      USB/UART        │   │
│  │  (ledc_bus)  │  │  (max31856)  │  │    (communication)   │   │
│  └──────────────┘  └──────────────┘  └──────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Current Implementation |
|-----------|----------------|----------------------|
| `RoasterControl` | Main state machine, command routing | `src/control/roaster_refactored.rs` (722 lines) |
| `SsrControlBase` | SSR state and logic extraction | `src/hardware/ssr.rs` (lines 87-156) |
| `Heater` trait | Hardware abstraction | `src/control/traits.rs` (lines 16-28) |
| `ArtisanFormatter` | CSV output formatting | `src/output/artisan.rs` (603 lines) |
| `tests/common/mod.rs` | Shared test stubs | `tests/common/mod.rs` (317 lines) |

---

## v4.5 Refactoring Tasks

### Task 1: SSR Control — Trait Delegation Enhancement

**Current State (v4.4):**

```rust
// src/hardware/ssr.rs - SsrControlBase already exists
pub struct SsrControlBase {
    pub(crate) hardware_status: SsrHardwareStatus,
    pub(crate) current_duty: u16,
    pub(crate) last_duty_delta_ticks: i16,
    pub(crate) retry_count: u8,
    pub(crate) last_detection_check: Option<u32>,
    pub(crate) is_pwm_enabled: bool,
}
```

**v4.5 Enhancement:**
The v4.4 already provides `StatusGetters` trait. The v4.5 may extend this with:
- `HeatSourceDetector` trait (already exists at lines 98-100)
- `PeriodicCheck` trait (already exists at lines 104-106)
- Formal delegation macros for reduce boilerplate

**Integration Points:**

| Component | File | Change Type |
|-----------|------|-------------|
| `SsrControlBase` | `src/hardware/ssr.rs` | Already complete (v4.4) |
| `StatusGetters` trait | `src/hardware/ssr.rs` | Already complete (v4.4) |
| `HeatSourceDetector` | `src/hardware/ssr.rs` | Already complete (v4.4) |
| `PeriodicCheck` | `src/hardware/ssr.rs` | Already complete (v4.4) |

**Build Impact:** None — v4.4 already completed this work.

---

### Task 2: Test Infrastructure Integration

**Current State (v4.4):**

```rust
// tests/common/mod.rs - Already created
pub struct StubHeater {
    pub calls: RefCell<Vec<HeaterCall>>,
    pub status: RefCell<SsrHardwareStatus>,
}

pub struct StubFan {
    pub calls: RefCell<Vec<FanCall>>,
    pub speed: RefCell<f32>,
}

pub struct StubThermometer {
    pub calls: RefCell<Vec<ThermometerCall>>,
    pub temp: RefCell<f32>,
}
```

**v4.5 Task:**
Use the existing test stubs in more test files rather than defining inline mocks.

**Integration Points:**

| Component | File | Current Usage | v4.5 Target |
|-----------|------|---------------|-------------|
| `StubHeater` | `tests/common/mod.rs` | Limited | Expand usage |
| `StubFan` | `tests/common/mod.rs` | Limited | Expand usage |
| `StubThermometer` | `tests/common/mod.rs` | Limited | Expand usage |

**Files to Update:**
- `tests/ssr_monitor.rs` — inline `FakeDetectPin`, `FakeLedcChannel`
- `tests/command_errors.rs` — inline command builders
- `tests/fan_serialization.rs` — inline fan mocks

**Build Order:**
1. Verify `tests/common/mod.rs` compiles: `cargo test --no-run`
2. Migrate one test file at a time
3. Run tests after each migration

---

### Task 3: Formatter — BT History Optimization

**Current State:**

```rust
// src/output/artisan.rs
pub struct ArtisanFormatter {
    start_time: Instant,
    last_bt: f32,
    bt_history: Vec<f32>,  // DYNAMIC ALLOCATION - problematic for embedded
}

pub struct MutableArtisanFormatter {
    start_time: Instant,
    last_bt: f32,
    bt_history: Vec<f32>,  // Same issue
}
```

**Problem:** `Vec<f32>` requires heap allocation (`#![no_std]` incompatibility), and the history is limited to 5 samples anyway.

**v4.5 Solution:** Replace `Vec<f32>` with fixed-size array:

```rust
// Proposed: Fixed-size array (no heap allocation)
const BT_HISTORY_SIZE: usize = 5;

pub struct ArtisanFormatter {
    start_time: Instant,
    last_bt: f32,
    bt_history: [f32; BT_HISTORY_SIZE],  // FIXED SIZE - no allocation
    bt_history_len: usize,                // Track actual used slots
}

pub struct MutableArtisanFormatter {
    start_time: Instant,
    last_bt: f32,
    bt_history: [f32; BT_HISTORY_SIZE],
    bt_history_len: usize,
}
```

**Integration Points:**

| Component | File | Change Type |
|-----------|------|-------------|
| `ArtisanFormatter` | `src/output/artisan.rs` | Modify — replace Vec with [f32; 5] |
| `MutableArtisanFormatter` | `src/output/artisan.rs` | Modify — replace Vec with [f32; 5] |
| `compute_ror_from_history()` | `src/output/artisan.rs` | Modify — adapt to fixed-size |
| `update_bt_history()` | `src/output/artisan.rs` | Modify — circular buffer or shift |

**Data Flow (Unchanged):**

```
SystemStatus (bean_temp)
    ↓
ArtisanFormatter::format() or MutableArtisanFormatter::format()
    ↓
update_bt_history() → compute_ror_from_history()
    ↓
CSV string: "time,ET,BT,ROR,Gas"
```

**Build Order:**
1. Modify `ArtisanFormatter` struct and methods
2. Modify `MutableArtisanFormatter` struct and methods
3. Update unit tests to reflect new API
4. Verify host tests pass: `cargo test`
5. Verify no_std compatibility: `cargo check --target riscv32`

---

### Task 4: Command Processing — Match Decomposition

**Current State:**

```rust
// src/control/roaster_refactored.rs - Lines 205-241
pub fn process_command(
    &mut self,
    command: RoasterCommand,
    current_time: Instant,
) -> Result<(), RoasterError> {
    // Direct match on command type - 36+ branches
    match command {
        RoasterCommand::StopRoast => { ... }
        RoasterCommand::SetHeaterManual(value) => { ... }
        RoasterCommand::SetFanManual(value) => { ... }
        // ... 30+ more variants
        RoasterCommand::IncreaseHeater => { ... }
        RoasterCommand::DecreaseHeater => { ... }
        // ... more
    }
}
```

Additionally, `process_artisan_command()` (lines 533-660) has another large match with ~15 branches.

**v4.5 Solution:** The handler pattern already exists in `handlers.rs`:

```rust
// Current: handlers.rs already has the pattern we need
pub struct TemperatureCommandHandler { ... }
pub struct SafetyCommandHandler { ... }
pub struct ArtisanCommandHandler { ... }
pub struct SystemCommandHandler { ... }

// roaster_refactored.rs already uses this pattern:
let mut handlers: [&mut dyn RoasterCommandHandler; 4] = [
    &mut self.safety_handler,
    &mut self.temp_handler,
    &mut self.artisan_handler,
    &mut self.system_handler,
];

for handler in &mut handlers {
    if handler.can_handle(command) {
        return handler.handle_command(command, current_time, &mut self.status);
    }
}
```

**Refactoring Options:**

| Option | Description | Trade-off |
|--------|-------------|-----------|
| A: Expand handler pattern | Use existing handlers for ALL commands | Requires adding more commands to handlers |
| B: Command groups | Split into `process_roaster_command()` and `process_artisan_command()` | Better separation, less duplication |
| C: Command enum optimization | Use `#[derive)]` macros for match exhaustiveness | Compile-time help, not runtime improvement |

**Recommended: Option A** — The handler chain already exists; just expand it to cover all commands rather than having special-case handling in `RoasterControl::process_command()`.

**Integration Points:**

| Component | File | Change Type |
|-----------|------|-------------|
| `RoasterControl::process_command()` | `src/control/roaster_refactored.rs` | Refactor — delegate ALL to handlers |
| `RoasterControl::process_artisan_command()` | `src/control/roaster_refactored.rs` | Refactor — may integrate into main flow |
| `TemperatureCommandHandler` | `src/control/handlers.rs` | May need more command coverage |
| `ArtisanCommandHandler` | `src/control/handlers.rs` | May need more command coverage |

**Commands to Migrate to Handlers:**

Currently in `roaster_refactored.rs` direct handling:
- `SetHeaterManual` — already in `ArtisanCommandHandler`
- `SetFanManual` — already in `ArtisanCommandHandler`
- `IncreaseHeater` — already in `ArtisanCommandHandler`
- `DecreaseHeater` — already in `ArtisanCommandHandler`

The main `process_command` already delegates properly. The task is verifying this pattern is complete.

---

## Build Order and Dependencies

### Phase 1: Verify v4.4 Foundation (No Changes)

```bash
# Verify SSR base structure
cargo check --target riscv32

# Verify test stubs exist
cargo test --no-run

# Quick sanity check
cargo check
```

### Phase 2: Formatter Optimization

```bash
# 1. Modify artisan.rs - replace Vec with [f32; 5]
# 2. Update compute_ror_from_history signature
# 3. Update update_bt_history logic
# 4. Run unit tests
cargo test artisan

# 5. Verify embedded build
cargo check --target riscv32
```

### Phase 3: Test Infrastructure Usage

```bash
# 1. Migrate ssr_monitor.rs to use tests/common/mod.rs
# 2. Run tests
cargo test ssr_monitor

# 3. Migrate remaining test files
# 4. Full test suite
cargo test
```

### Phase 4: Command Processing Verification

```bash
# 1. Audit process_command for any remaining direct handling
# 2. Ensure all commands go through handler chain
# 3. Run integration tests
cargo test --test artisan_integration_test

# 4. Full test suite
cargo test
```

---

## Integration Matrix

### New vs Modified Components

| Component | Type | Reason |
|-----------|------|--------|
| `ArtisanFormatter.bt_history` | Modify | Vec → [f32; 5] |
| `MutableArtisanFormatter.bt_history` | Modify | Vec → [f32; 5] |
| `tests/common/mod.rs` | Existing | Use in more tests |
| `tests/ssr_monitor.rs` | Modify | Use shared stubs |
| `process_command()` | Verify | Ensure handler delegation complete |

### Unchanged Components (Already Complete)

| Component | Reason |
|-----------|--------|
| `SsrControlBase` | v4.4 already complete |
| `StatusGetters` trait | v4.4 already complete |
| `HeatSourceDetector` trait | v4.4 already complete |
| `PeriodicCheck` trait | v4.4 already complete |
| `StubHeater`, `StubFan`, `StubThermometer` | v4.4 already complete |
| Handler pattern (`RoasterCommandHandler`) | Already implemented |

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| SSR refactoring (v4.4) | HIGH | Already complete, v4.5 may be verification |
| Test infrastructure | HIGH | Stubs exist, integration is straightforward |
| Formatter optimization | HIGH | Vec→array is well-understood pattern |
| Command processing | HIGH | Handler pattern already exists |

---

## Sources

- SSR implementation: `src/hardware/ssr.rs` (650 lines)
- Test stubs: `tests/common/mod.rs` (317 lines)
- Formatter: `src/output/artisan.rs` (603 lines)
- Command processing: `src/control/roaster_refactored.rs` (722 lines)
- Handler pattern: `src/control/handlers.rs` (471 lines)

---

*Architecture research for: LibreRoaster v4.5 Refactoring Tasks*  
*Researched: 2026-02-28*
