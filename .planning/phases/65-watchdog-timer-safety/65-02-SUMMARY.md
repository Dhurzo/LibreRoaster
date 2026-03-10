---
phase: 65-watchdog-timer-safety
plan: 02
subsystem: safety
tags: [ledc, watchdog, regression, telemetry]

# Dependency graph
requires:
  - phase: 65-watchdog-timer-safety-65-01
    provides: WatchdogFeeder visibility plus failure telemetry and SystemStatus hooks
provides:
  - Timeout-aware LEDC guard instrumentation that logs SAFETY LEDC-GUARD and updates SystemStatus
  - Artisan REG command and regression runner that forces emergency shutdown while keeping the watchdog fed
affects:
  - Phase 65 watchdog observability
  - Any future safety instrumentation that needs guard/resets telemetry

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Guarded LEDC writes now drop the guard token before any `await` and expose total timeouts for telemetry
    - Regression runner keeps watchdog pings alive while simulating an over-temperature shutdown and emitting SAFETY OT-REGRESSION

key-files:
  created:
    - src/hardware/ledc_guard.rs
    - src/safety/regression.rs
    - src/safety/mod.rs
  modified:
    - src/hardware/ledc_bus.rs
    - src/application/tasks.rs
    - src/input/parser.rs

key-decisions:
  - "None - followed plan as specified"

patterns-established:
  - "Guard-aware LEDC writes release the token before yielding and push SAFETY LEDC-GUARD telemetry when the counter ticks up"
  - "Regression runner enforces an over-temp shutdown while keeping the watchdog fed and pushing SAFETY OT-REGRESSION"

# Metrics
duration: 9 min 40 sec
completed: 2026-02-23
---

# Phase 65: Watchdog Timer Safety Plan 02 Summary

**LEDC guard timeouts now release hardware, log SAFETY LEDC-GUARD, and let Artisan REG trigger an over-temp regression runner that feeds the watchdog and emits SAFETY OT-REGRESSION telemetry.**

## Performance

- **Duration:** 9 min 40 sec
- **Started:** 2026-02-23T17:04:19Z
- **Completed:** 2026-02-23T17:13:59Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Added `LedcGuard::try_acquire` plus LEDC bus wiring so guard failures log once and the control loop mirrors the timeout counter before sleeping.
- Control loop now observes the guard counter through `SystemStatus::ledc_guard_timeouts` and emits `SAFETY LEDC-GUARD` telemetry whenever the counter increases.
- Instrumented the Artisan `REG` command with `OverTempTestRunner`, which ramps outputs, forces `RoasterControl::emergency_shutdown`, keeps the watchdog fed, and emits `SAFETY OT-REGRESSION` without crashing the scheduler.

## Task Commits

1. **Task 1: Add LEDC guard module and integrate with the bus** - `4402828` (feat)
2. **Task 2: Add over-temp regression runner and SAFETY telemetry** - `806f3f9` (feat)

**Plan metadata:** `docs(65-02): complete watchdog timer safety plan`

## Files Created/Modified
- `src/hardware/ledc_guard.rs` - new timeout-aware guard that exposes total timeouts without holding hardware across awaits.
- `src/hardware/ledc_bus.rs` - now uses the guard, logs SAFETY LEDC-GUARD on timeouts, and returns early so control never touches LEDC when the guard is saturated.
- `src/application/tasks.rs` - the control loop mirrors the guard counter, bumps `SystemStatus::ledc_guard_timeouts`, and pushes SAFETY LEDC-GUARD lines as the counter grows.
- `src/input/parser.rs` - recognizes `REG` and now its parser test ensures instrumentation can trigger the regression runner.
- `src/safety/regression.rs` - implements `OverTempTestRunner`, watchdog feeding during the regression, and SAFETY OT-REGRESSION emission.
- `src/safety/mod.rs` - exposes the regression and watchdog modules to the rest of the crate.

## Decisions Made
None - followed plan as specified.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- `cargo test input::parser::test_parse_regression_command` still cannot run for the `riscv32imc-unknown-none-elf` target because the hardware-only toolchain lacks `std`. Verified the parser by running the same test on the host target (`x86_64-unknown-linux-gnu`), where it now passes after the regression stub/performance tweaks.

## User Setup Required
None - no external services were introduced.

## Next Phase Readiness
- Phase 65 is complete; the watchdog guard telemetry and regression instrumentation are in place for future safety/observability milestones.
