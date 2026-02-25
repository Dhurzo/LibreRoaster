# Project Research Summary: v4.4 SSR Refactoring & Test Stubs

**Project:** LibreRoaster (ESP32-C3 Coffee Roaster Firmware)
**Domain:** Embedded Rust firmware - coffee roaster control
**Researched:** 2026-02-24
**Confidence:** HIGH

## Executive Summary

This research addresses two key refactoring goals for LibreRoaster v4.4:

1. **SSR Deduplication**: The existing `SsrControl` and `SsrControlSimple` structs share ~95% identical code. Research confirms the recommended approach is **composition + trait default implementations** - extract common state into `SsrControlBase`, define a trait with shared methods, and have both types embed the base while implementing the trait. This eliminates duplication while preserving zero-cost abstraction.

2. **Shared Test Infrastructure**: Currently, test mocks are scattered across individual test files, causing ~5x duplication. Research recommends creating `tests/common/mod.rs` with manual stub implementations (`StubHeater`, `StubFan`, `StubThermometer`) that implement the existing `control::traits`. No external crates needed - use `RefCell` for interior mutability.

**Key risks identified:**
- Breaking embedded-hal trait bounds during deduplication (CRITICAL)
- Losing Send+Sync safety in extracted types (CRITICAL)
- Safety-critical detection logic must be preserved exactly (CRITICAL)

The existing codebase already has substantial infrastructure - SSR control, Heater trait, and some test mocks. This milestone consolidates and shares that infrastructure.

## Key Findings

### Recommended Stack

**SSR Refactoring: Composition + Trait Pattern**

| Approach | Implementation | Why |
|----------|---------------|-----|
| **Primary** | Extract `SsrState` struct with common fields | Zero-cost abstraction, embeds shared state in both types |
| **Secondary** | Define `SsrControlTrait` with default implementations | Eliminates duplicate method implementations |
| **Existing** | Keep `Heater` trait from `control::traits` | Already provides `set_power` abstraction used by roaster control |

**Test Infrastructure: Shared Stubs Module**

| Component | Location | Implementation |
|-----------|----------|----------------|
| **Test stubs** | `tests/common/mod.rs` | Manual struct implementations (no external crate needed) |
| **Stub patterns** | StubHeater, StubFan, StubThermometer | Implement existing `control::traits` traits |
| **Helper utilities** | `reset_channels()`, `collect_output()` | Module-level functions for test state management |

### Expected Features

**Must have (table stakes):**
- SSR on/off control — Basic heating element control via GPIO
- SSR PWM/phase control — Variable heating power (not just on/off)
- Heat source detection — Verify SSR is actually heating
- Cycle guard — Prevent SSR damage from rapid cycling
- Duty readback verification — Confirm PWM duty matches commanded
- **Shared test stubs** — Centralize mock implementations for Heater, Fan, Thermometer traits

**Should have (competitive):**
- MockHeater test double — For PID/controller unit tests (currently missing)
- Error path test coverage — Mocks that simulate error conditions

**Defer (v2+):**
- PID auto-tuning
- Dual SSR channel support
- Hardware-in-the-loop (HIL) tests

### Architecture Approach

**Current Problem:** `SsrControl` and `SsrControlSimple` have ~95% duplicate code across 10+ methods including `detect_heat_source()`, `periodic_check()`, `set_percentage()`, getters, and `Heater` trait implementation.

**Recommended Solution:** Extract `SsrControlBase`:

```
src/hardware/ssr.rs (refactored)
├── SsrControlBase<'a, DETECT, PWM>    # NEW: Common state and logic
│   ├── detection_pin, pwm_channel
│   ├── hardware_status, current_duty, last_duty_delta_ticks, retry_count
│   └── Methods: detect_heat_source, periodic_check, set_percentage, getters
├── SsrControl<'a, PIN, DETECT, PWM>   # Wraps Base + enable pin
└── SsrControlSimple<'a, DETECT, PWM>  # Wraps Base only
```

**Test Infrastructure:**
```
tests/common/
├── mod.rs              # Re-exports, helper functions
├── stub_heater.rs       # StubHeater implementation
├── stub_fan.rs         # StubFan implementation
└── stub_thermometer.rs # StubThermometer implementation
```

### Critical Pitfalls

1. **Breaking embedded-hal Trait Bounds** — Extracting shared logic changes generic constraints, breaking `Heater` trait implementation. *Prevention: Define clear trait bounds before refactoring, test compilation of dependent code after each step.*

2. **Losing Send+Sync Safety** — Refactoring may introduce RefCell/Cell for interior mutability, breaking async task boundaries. *Prevention: Preserve existing `unsafe impl Send` pattern, avoid interior mutability in refactored SSR types.*

3. **Safety-Critical Detection Logic Loss** — `detect_heat_source` contains safety logic that must be preserved exactly. *Prevention: Create checklist of all state transitions before refactoring.*

4. **Breaking PWM Readback Contract** — `monitor_ledc_after_set` is essential for safety; must be preserved. *Prevention: Keep readback call in public API after deduplication.*

5. **Test State Pollution** — Shared mocks using RefCell can retain state between tests. *Prevention: Each test creates fresh mock instance, add reset methods.*

## Implications for Roadmap

Based on research, this milestone should be structured as two sequential phases:

### Phase 1: SSR Refactoring (Base Struct Extraction)
**Rationale:** This is foundational - the other work depends on having clean, non-duplicated SSR code. Must be done first to enable proper shared test infrastructure.

**Delivers:**
- `SsrControlBase` struct with shared state and methods
- Refactored `SsrControl` and `SsrControlSimple` delegating to base
- Preserved `Heater` trait implementations

**Addresses:**
- FEATURES: SSR PWM control, heat source detection, duty readback, cycle guard
- ARCHITECTURE: Eliminates ~95% code duplication

**Avoids:**
- PITFALLS: Trait bound breakage, Send+Sync loss, safety logic loss, PWM readback break

**Research Flags:**
- This phase is well-understood (HIGH confidence from STACK.md research)
- Standard patterns - skip `/gsd-research-phase` during planning

### Phase 2: Shared Test Infrastructure
**Rationale:** Depends on SSR refactoring complete (mock implementations may need updating after structural changes). Creates reusable infrastructure for future tests.

**Delivers:**
- `tests/common/mod.rs` with StubHeater, StubFan, StubThermometer
- Helper functions: `reset_channels()`, `collect_output()`
- Migrated existing tests to use shared stubs

**Addresses:**
- FEATURES: Shared mock location, MockHeater test double
- ARCHITECTURE: Centralized test infrastructure

**Avoids:**
- PITFALLS: Mock API drift, test state pollution, missing trait boundary tests

**Research Flags:**
- This phase is well-understood (HIGH confidence from existing test patterns)
- May need research if new error-path tests require complex mock configurations

### Phase Ordering Rationale

1. **SSR first** - The structural changes in Phase 1 could break existing mocks. Completing Phase 1 first ensures Phase 2 builds on stable foundations.

2. **Two-phase structure** - Separates infrastructure creation from migration, allowing verification between steps.

3. **Avoids Pitfall 11** - "Refactoring SSR Then Breaking All Existing Mocks" is mitigated by doing SSR refactoring first.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Composition pattern well-established in Rust; existing codebase verified |
| Features | HIGH | Table stakes verified via code review; gap analysis accurate |
| Architecture | HIGH | Base struct pattern matches existing codebase structure |
| Pitfalls | MEDIUM-HIGH | 10+ pitfalls identified; some mitigation strategies inferred |

**Overall confidence:** HIGH

### Gaps to Address

- **Heater trait boundary tests**: No dedicated tests exist for Heater trait implementation on SSR types. Should add in Phase 2.
- **Error-path mock coverage**: Current mocks implement happy path only. Phase 2 should add error variants.
- **Property-based tests**: Not in scope for v4.4 but would add value for SSR percentage conversion math.

## Sources

### Primary (HIGH confidence)
- LibreRoaster codebase (`src/hardware/ssr.rs`, `src/control/traits.rs`) — Verified SSR implementation
- `tests/mock_uart.rs`, `tests/mock_usb_driver.rs` — Verified existing mock patterns
- Stack Overflow: Rust trait deduplication patterns

### Secondary (MEDIUM confidence)
- embedded-hal-mock crate documentation — Test infrastructure patterns
- Rust Send+Sync requirements — Embedded async safety

---

*Research completed: 2026-02-24*
*Ready for roadmap: yes*
*Milestone: v4.4 SSR Refactoring & Test Stubs*
