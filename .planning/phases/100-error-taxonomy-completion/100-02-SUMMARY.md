---
phase: 100-error-taxonomy-completion
plan: 02
subsystem: logging
tags: rust, error-handling, traceability, telemetry, instrumentation, no_std

# Dependency graph
requires:
  - phase: 100-01
    provides: Error struct variants (RoasterError, Max31856Error) with source fields
provides:
  - TRACE events that include AppError category and source fields for telemetry and guard stages
  - Telemetry/guard instrumentation that captures and passes AppError diagnostics to TRACE events
affects: Phase 101 (Traceability Matrix Alignment)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - AppError-aware TRACE event formatting with optional metadata fields
    - Error-to-diagnostics conversion pattern using AppError::category() and AppError::source()

key-files:
  created: []
  modified:
    - src/logging/traceability.rs - Extended trace_telemetry and trace_guard functions to accept AppError metadata
    - src/application/tasks.rs - Added AppError tracking and capture in control loop

key-decisions:
  - "Pass Option<&AppError> to TRACE helpers to allow graceful handling when error metadata is unavailable"
  - "Capture RoasterError → AppError conversion during control updates for diagnostics"
  - "Sensor and watchdog errors (ContainerError) don't convert directly to AppError, so they log but don't emit AppError diagnostics"

patterns-established:
  - "Pattern: TRACE event fields append error_category and error_source when AppError is present"
  - "Pattern: Control loop tick tracks errors in tick_app_error variable for TRACE correlation"

# Metrics
duration: 3 min
completed: 2026-03-20
---

# Phase 100 Plan 02: AppError TRACE Integration Summary

**TRACE telemetry and guard events now include AppError category and source fields when errors occur in control loop stages**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-20T19:54:51Z
- **Completed:** 2026-03-20T19:58:25Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Extended `trace_telemetry` and `trace_guard` to accept `Option<&AppError>` parameters
- Modified formatting helpers to include `error_category` and `error_source` fields when AppError is present
- Added AppError import to control loop tasks module
- Implemented tick-level error tracking with `tick_app_error` variable
- Captured RoasterError → AppError conversion during control update failures
- Passed AppError diagnostics to TRACE events for telemetry and guard correlation
- Added unit tests to verify AppError metadata formatting in TRACE events

## Task Commits

Each task was committed atomically:

1. **Task 1: Extend TRACE helpers with AppError metadata** - `5e18b37` (feat)
2. **Task 2: Surface AppError diagnostics in telemetry + guard instrumentation** - `d516735` (feat)

**Plan metadata:** (to be committed)

## Files Created/Modified

- `src/logging/traceability.rs` - Extended TRACE event functions to accept and format AppError metadata
- `src/application/tasks.rs` - Added AppError tracking and capture in control loop instrumentation

## Decisions Made

None - followed plan as specified with these implementation notes:

- ContainerError (sensor, watchdog) doesn't convert directly to AppError, so these errors log warnings but don't emit AppError diagnostics in TRACE events. This is acceptable since ContainerError is infrastructure-level and RoasterError (control-level) converts to AppError.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None - all tasks completed successfully with no blocking issues.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- TRACE helpers now emit AppError diagnostics for telemetry and guard events
- Control loop captures RoasterError and converts to AppError for TRACE correlation
- Ready for Phase 101: Traceability Matrix Alignment to ensure host-side tooling can parse the new TRACE event format

---

*Phase: 100-error-taxonomy-completion*
*Completed: 2026-03-20*
