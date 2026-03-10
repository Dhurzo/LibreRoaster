---
phase: 71-anti-windup-stabilization
plan: 02
subsystem: control
tags: [pid, telemetry, instrumentation]

# Dependency graph
requires:
  - phase: 71-01
    provides: saturation-aware PID gating and guard busy telemetry
provides:
  - filtered derivative instrumentation that only spikes when PV motion is real
  - actuator feedback routed to the PID so SystemStatus carries the true integrator and MV
affects:
  - Phase 71-03 anti-windup STATUS telemetry verification

# Tech tracking
tech-stack:
  added: []
  patterns:
    - filter derivative telemetry before telemetry emission so STATUS responds only to valid PV motion
    - feed actual actuator feedback and guard busy state into CoffeeRoasterPid so MV stays synchronized with hardware

key-files:
  created: []
  modified:
    - src/control/roaster_refactored.rs
    - src/control/handlers.rs
    - src/control/pid.rs

key-decisions:
  - "Filter PV samples before publishing derivative_rate so telemetry spikes match real plant motion."
  - "Mirror the PID integrator/saturation state and feed guard busy feedback so MV never over-claims actuator energy."

patterns-established:
  - "STATUS now produces filtered derivative rates gated by valid dt samples."
  - "PID now receives guard feedback per tick and reports integrator/saturation state before telemetry snapshots."

# Metrics
completed: 2026-02-24
---

# Phase 71 Plan 02: Anti-windup instrumentation summary

**STATUS now reports a filtered PV derivative and the PID’s real integrator/MV by routing guard feedback into the controller before each tick.**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-24T07:23:51Z
- **Completed:** 2026-02-24T07:28:43Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Added derivative filtering inside `update_control` so `SystemStatus::derivative_rate` only updates on valid PV motion and toggles availability accordingly.
- Captured guard busy status plus applied output before reporting to `CoffeeRoasterPid`, then mirrored the PID’s integrator, saturation, and MV state inside `SystemStatus`.
- Adjusted the PID helper surface so `RoasterControl` can read integrator/derivative/saturation state and keep telemetry aligned with actual hardware.

## Task Commits

1. **Task 1: Compute a filtered derivative from PV samples** - `2d2abbb` (feat)
2. **Task 2: Relay actuator saturation and integrator state into the PID** - `bf23788` (feat)

**Plan metadata:** docs(71-02): complete anti-windup instrumentation plan

_Note: `cargo test --lib` cannot run for the `riscv32imc-unknown-none-elf` target without std support._

## Files Created/Modified

- `src/control/roaster_refactored.rs` - adds filtered derivative helper and PID feedback plumbing before telemetry dumps.
- `src/control/handlers.rs` - exposes PID integrator/derivative/saturation helpers used by RoasterControl.
- `src/control/pid.rs` - tightens saturation detection with a tolerance so tiny rounding differences don’t falsely gate the integrator.

## Decisions Made

- Filter derivative telemetry at `update_control` so STATUS only shows spikes on real PV motion while `derivative_available` reflects trustworthy intervals.
- Mirror the PID integrator and saturation state from `CoffeeRoasterPid` and let the PID survey guard busy feedback before telemetry so MV never outpaces the applied output.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- `cargo test --lib` fails because the `riscv32imc-unknown-none-elf` target cannot link to the `std` crate that dependencies such as `critical-section` and `futures` pull in.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Control loop instrumentation now emits truthful derivative/integrator/saturation snapshots.
- Ready for 71-03 to surface the new STATE tail inside `STATUS` tests and telemetry logs.

---
*Phase: 71-anti-windup-stabilization*
*Completed: 2026-02-24*
