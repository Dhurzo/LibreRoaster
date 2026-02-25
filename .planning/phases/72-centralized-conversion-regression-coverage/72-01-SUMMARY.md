---
phase: 72-centralized-conversion-regression-coverage
plan: 72-01
subsystem: hardware
tags: [embedded-hal, sensors, max31856, conversion]

# Dependency graph
requires:
  - phase: 71-anti-windup-stabilization
    provides: anti-windup/instrumentation baselines that the new hub now feeds
provides:
  - shared `SensorConversionHub` + `SensorSample`/`SensorFault` APIs that enforce the 0.0078125 °C math and capture per-sensor faults
  - cached samples that keep control, telemetry, and regulators aligned even when a MAX31856 channel trips a fault bit
  - AppBuilder wiring so `RoasterControl` and future helpers all receive the same helper instance
affects:
  - Phase 72 regression coverage (72-02) and deterministic telemetry tests that rely on this helper

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Central conversion hub + cached `SensorSample` ensures every consumer uses the same MAX31856 math and fault metadata
    - Builder-owned wiring keeps a single helper instance shared across control, regression, and instrumentation flows

key-files:
  created:
    - src/hardware/sensors/conversion.rs
    - src/hardware/sensors/mod.rs
  modified:
    - src/hardware/max31856.rs
    - src/hardware/mod.rs
    - src/application/app_builder.rs
    - src/control/roaster_refactored.rs

key-decisions:
  - "Centralizing MAX31856 register reads, conversion math, and fault mapping inside `SensorConversionHub` keeps telemetry, regression, and control using the exact same LSB scaling."
  - "RoasterControl should accept the shared hub and expose its cached `SensorSample` so automation never reimplements conversion logic."

patterns-established:
  - "SensorSample caching with explicit `SensorFault` booleans lets the loop surface instrumentation/fault flags even when MAX31856 sampling hiccups occur."
  - "AppBuilder now owns the helper wiring, so any test or context can inject a hub without duplicating initialization logic."

# Metrics
duration: 11 min 56 sec
completed: 2026-02-24
---

# Phase 72: Centralized conversion regression coverage Summary

**Centralized MAX31856 conversion math via `SensorConversionHub` so every consumer reuses the same 0.0078125 °C scaling, cached samples, and fault mapping.**

## Performance
- **Duration:** 11 min 56 sec
- **Started:** 2026-02-24T07:57:39Z
- **Completed:** 2026-02-24T08:09:37Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Implemented `SensorConversionHub` plus `SensorSample`/`SensorFault` helpers that centralize conversion, caching, and fault exposure.
- RoasterControl now consumes the shared hub, propagates the cached sample into `update_temperatures`, and exposes the last sample for instrumentation.
- AppBuilder builds the helper (and exposes `with_sensor_conversion_hub`) so main, regression, and tests all share the same math.

## Task Commits
1. **Task 1: Implement SensorConversionHub helper** - `0adb7a9` (feat)
2. **Task 2: Wire RoasterControl and builders through the hub** - `5aee645` (feat)

**Plan metadata:** docs(72-01): complete centralized conversion plan

## Files Created/Modified
- `src/hardware/sensors/conversion.rs` - new helper owning both sensors, caching `SensorSample`, and exposing scaled conversions/faults.
- `src/hardware/sensors/mod.rs` - re-exports the hub plus helper helpers.
- `src/hardware/max31856.rs` - raw read helpers and shared conversion logic so the hub can reuse the driver without duplicating registers.
- `src/hardware/mod.rs` - exposes the new `sensors` module hierarchy.
- `src/application/app_builder.rs` - new `sensor_hub` field plus wiring paths so the builder delivers the helper to `RoasterControl`.
- `src/control/roaster_refactored.rs` - control loop now holds the hub, uses `SensorSample` for updates, and exposes the cached sample.

## Decisions Made
- Centralizing MAX31856 register reads/conversion/fault mapping inside `SensorConversionHub` prevents downstream consumers from drifting on rounding or fault handling.
- RoasterControl should accept the shared hub so the control loop, telemetry, and future regression helpers operate on the same cached sample and fault flags.

## Deviations from Plan

### Auto-fixed Issues
**1. [Rule 3 - Blocking] Added Max31856 raw read helpers for the hub**

- **Found during:** Task 1 (SensorConversionHub helper)
- **Issue:** The new hub needed the raw ADC bytes and fault register to deliver the 0.0078125 °C scaling and per-sensor fault booleans without duplicating conversion logic.
- **Fix:** Added `Max31856Reading`, synchronous + async raw read helpers, and moved the shared conversion math into `SensorConversionHub` so the driver and hub share the same helper.
- **Files modified:** `src/hardware/max31856.rs`
- **Verification:** `cargo check --lib` (cleared after the shared helpers compiled)
- **Commit:** `0adb7a9`

---

**Total deviations:** 1 auto-fixed (Rule 3)
**Impact on plan:** Essential for the helper to reuse MAX31856 registers while exposing the same math/fault semantics; no scope creep.

## Issues Encountered
- `cargo test --lib` fails on the `riscv32imc-unknown-none-elf` target because the standard library is unavailable and crates like `critical-section`/`futures` depend on `std`. The debug build is `no_std`, so tests must be run in a host or hardware-specific harness later.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- The control loop and builder now share a single SensorConversionHub, so Phase 72-02 can focus on deterministic regression coverage and testing against this helper.

---
*Phase: 72-centralized-conversion-regression-coverage*
*Completed: 2026-02-24*
