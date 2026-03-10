---
phase: 72-centralized-conversion-regression-coverage
plan: 72-02
subsystem: testing
tags: [regression, sensor-conversion, fixtures, automation]

# Dependency graph
requires:
  - phase: 72-01
    provides: SensorConversionHub centralizes MAX31856 math so the regression harness can reuse live conversion logic
provides:
  - Feature-gated regression task that replays fixtures through SensorConversionHub and emits the same 16-column STATUS tail as the live loop
  - Canonical MAX31856 fixture catalog with ADC/fault sequences, SPI transaction metadata, and expected ArtisanFormatter snapshots
affects: [72-03, automation]

# Tech tracking
tech-stack:
  added: [embedded-hal-mock]
  patterns: ["Feature-gate instrumentation so regression math only ships with the regression feature", "Regression harness now drives ArtisanFormatter snapshots from SensorConversionHub samples"]

key-files:
  created: [tests/fixtures/max31856_sequences.rs]
  modified: [Cargo.toml, src/control/roaster_refactored.rs, src/hardware/sensors/conversion.rs, src/safety/regression.rs]

key-decisions:
  - "Gate the over-temperature regression runner behind `feature = \"regression\"` so production builds stay lean while the harness still lands deterministic SensorConversionHub math."
  - "Regression fixtures now bake ADC/fault sequences plus the expected ArtisanFormatter STATUS tail, enabling automation to compare the same 16-column snapshot the live loop emits."

patterns-established:
  - "SensorConversionHub fixtures + ArtisanFormatter STATUS logging form the new regression snapshot pattern."
  - "Regression instrumentation now replicates watchdog feasting and STATUS emission inside a feature flag so regression runs do not alter the default firmware."

# Metrics
completed: 2026-02-24
---

# Phase 72: Plan 72-02 Summary

**Regression instrumentation now uses SensorConversionHub fixtures to prove the ASR STATUS tail without touching the default firmware.**

## Performance

- **Duration:** 10 min 22 sec
- **Started:** 2026-02-24T08:28:56Z
- **Completed:** 2026-02-24T08:39:18Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- Added the `regression` Cargo feature plus SensorConversionHub helpers so the regression task only compiles under the feature and can sample fixture-driven ADC/fault pairs.
- Routed `regression_task` through the hub-based sample/emission loop, feeding watchdogs, emitting each fixture’s STATUS line via `ArtisanFormatter`, and keeping the `SAFETY OT-REGRESSION` trigger in place.
- Built `tests/fixtures/max31856_sequences.rs` with canonical ADC/CJ/fault sequences, SPI transaction metadata, and expected 16-column STATUS snapshots so automation gets deterministic regression logs.

## Task Commits

Each task was committed atomically:

1. **Task 1: Feature-gate the regression harness through the hub** - `1feb2c0` (feat)
2. **Task 2: Write deterministic MAX31856 fixture sequences** - `f9ef89f` (feat)

**Plan metadata:** docs(72-02): complete regression instrumentation plan (pending final metadata commit)

## Files Created/Modified

- `tests/fixtures/max31856_sequences.rs` - regression fixture catalog with ADC/fault data, SPI expectations, and expected STATUS snapshots
- `Cargo.toml` - regression feature that pulls in `embedded-hal-mock`, keeping fixtures out of the default build
- `src/safety/regression.rs` - feature-gated regression_task that replays SensorConversionHub fixtures, logs STATUS, and emits `SAFETY OT-REGRESSION`
- `src/hardware/sensors/conversion.rs` - FixtureReading helpers plus `SensorConversionHub::sample_from_fixture` for deterministic samples
- `src/control/roaster_refactored.rs` - host constructor now initializes `last_filtered_derivative` so the struct compiles when regression hooks depend on the field

## Decisions Made

- Kept the harness behind `feature = "regression"` so production binaries stay lean while host fixtures still instantiate the conversion hub and emit watchdog-friendly snapshots.
- Regression fixtures now pair ADC/fault sequences with expected `ArtisanFormatter::format_status_response` strings so future automation can assert the STATUS tail remains deterministic.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Initialize `last_filtered_derivative` in the non-riscv constructor**

- **Found during:** Task 1 (gating the regression harness)
- **Issue:** Building the host-friendly constructor failed because the struct was missing the `last_filtered_derivative` field that the `riscv` constructor already set.
- **Fix:** Set `last_filtered_derivative` to `0.0` alongside the other host-only fields so the builder compiles.
- **Files modified:** src/control/roaster_refactored.rs
- **Verification:** `cargo check --target x86_64-unknown-linux-gnu --features regression --lib`
- **Committed in:** 1feb2c0

---

**Total deviations:** 1 auto-fixed (blocking initialization)
**Impact on plan:** The fix was necessary for the regression gating changes to compile and did not expand scope.

## Issues Encountered

- None

## User Setup Required

- None

## Next Phase Readiness

- Regression harness and fixture catalog are ready for `72-03` so the next plan can add deterministic conversion/regression tests that assert the shared helper continues to emit the same 16-column STATUS tail.

---
*Phase: 72-centralized-conversion-regression-coverage*
*Completed: 2026-02-24*
