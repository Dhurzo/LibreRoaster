---
phase: 102-diagnostics-verification
plan: 01
subsystem: instrumentation
tags: [traceability, diagnostics, safe-shutdown, testing]
requires:
  - phase: 101-traceability-matrix-alignment
    provides: "TRACE docs and parser vocabulary covering queue/telemetry/guard instrumentation"
provides:
  - "Helper emits safe-shutdown guard events that bundle AppError metadata for InitError failures"
  - "Trace emissions now fire before the LED blink loop so every InitError run writes watchdog_failure diagnostics"
  - "Regression tests, sample log, and docs document how to replay the safe-shutdown trace for auditors"
affects:
  - "v5.2 diagnostics verification"
tech-stack:
  added: []
  patterns:
    - "Building reusable guard formatters keeps telemetry and guard rows aligned on AppError metadata"
    - "Regression fixtures replay sample logs so watchdog_failure tokens are always exercised"
key-files:
  created:
    - logs/traceability/sample-safe-shutdown.log
  modified:
    - src/logging/traceability.rs
    - src/main.rs
    - scripts/test_traceability_matrix.py
    - internalDoc/INSTRUMENTATION_README.MD
key-decisions:
  - "None - followed plan as specified"
patterns-established:
  - "Guard events can include AppError metadata via helpers so telemetry and guard rows stay in sync"
  - "Safe-shutdown sample logs become regression fixtures to prove watchdog_failure coverage"
duration: 4 min
completed: 2026-03-20
---

# Phase 102: Plan 01 Summary

**Safe-shutdown guard events now surface AppError diagnostics via reusable trace helpers and replayable logs for regression audits.**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-20T21:12:05Z
- **Completed:** 2026-03-20T21:16:47Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Added `format_safe_shutdown_guard`/`trace_safe_shutdown_guard` helpers plus a unit test that hardcodes `guard_timeout=1`, `guard_timeouts=1`, and `watchdog_failure=init_error_failure` while reusing AppError metadata.
- The safe-shutdown path now converts `InitError` into `AppError::Initialization`, grabs a `TraceId`, and emits the guard TRACE event before entering the LED blink loop so auditors see diagnostics alongside the LED heartbeat.
- Parser regression coverage, the instrumentation guide, and a new `logs/traceability/sample-safe-shutdown.log` prove how to replay InitError guard diagnostics for auditors.

## Task Commits

1. **Task 1: Format guard events for safe shutdown** - `b9f9c6f` (feat)
2. **Task 2: Emit the guard trace from enter_safe_shutdown()** - `5a263db` (feat)
3. **Task 3: Document and test the safe-shutdown trace** - `a0f9935` (feat)

## Files Created/Modified

- `src/logging/traceability.rs` - Adds reusable guard helpers that inject `guard_timeout`, `watchdog_failure`, and AppError metadata consistently for safe-shutdown diagnostics.
- `src/main.rs` - Converts `InitError` into `AppError::Initialization`, generates a `TraceId`, and emits the guard event before the blink loop so InitError failures appear in TRACE logs.
- `scripts/test_traceability_matrix.py` - Imports `Path`, replays the new sample log, and asserts guard telemetry is captured with `watchdog_failure` and AppError fields.
- `logs/traceability/sample-safe-shutdown.log` - Representative TRACE sequence (queue → actuation → telemetry → guard) for auditors to replay safe-shutdown failures.
- `internalDoc/INSTRUMENTATION_README.MD` - Documents how to capture/replay the safe-shutdown trace and links to the sample log and regression test.

## Decisions Made

- None - followed plan as specified.

## Deviations from Plan

- None - plan executed exactly as written.

## Issues Encountered

- `cargo test --package libreroaster traceability` failed because several test binaries (command_idempotence, fan_serialization, etc.) link against `embassy_time` helpers that are unresolved on the host (`_embassy_time_now`, `_embassy_time_schedule_wake`). The failure predates these changes and prevents the targeted test suite from completing.
- `cargo check --release --features embedded`, `PYTHONPATH=. python3 scripts/test_traceability_matrix.py`, and `PYTHONPATH=. python3 scripts/traceability_matrix.py logs/traceability/sample-safe-shutdown.log` all passed.

## User Setup Required

- None - no external configuration required.

## Next Phase Readiness

- Safe-shutdown diagnostics now emit guard events with AppError metadata, the parser/tests replay the guard log, and documentation describes how to capture/replay failure traces for auditors, so the next diagnostics/verification work can consume these artifacts.
