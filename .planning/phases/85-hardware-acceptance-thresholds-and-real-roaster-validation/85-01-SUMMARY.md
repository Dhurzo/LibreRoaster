---
phase: 85-hardware-acceptance-thresholds-and-real-roaster-validation
plan: 01
subsystem: hardware
tags: [hil, latency, instrumentation, rust]

# Dependency graph
requires:
  - phase: 84-solid-seam-hardening-and-fault-injection
    provides: [Harden handler/hardware policy boundary]
provides:
  - [Numeric acceptance thresholds for HIL validation]
  - [Firmware instrumentation for command latency reporting]
affects: [85-02-HIL-Validation-Runner-Implementation]

# Tech tracking
tech-stack:
  added: []
  patterns: [Latency instrumentation in task loop]

key-files:
  created: 
    - tests/hardware/thresholds.json
    - tests/hardware/METHODOLOGY.md
  modified:
    - src/config/constants.rs
    - src/output/artisan.rs
    - src/application/tasks.rs

key-decisions:
  - "Define latency as µs delta between command dequeue and handler completion"
  - "Append latency metrics to STATUS response, extending it to 18 fields"

# Metrics
duration: 2 min
completed: 2026-03-08
---

# Phase 85 Plan 01: Threshold Definition and Firmware Instrumentation Summary

**Defined numeric acceptance thresholds for HIL validation and instrumented the firmware to measure and report command-to-actuator latency via STATUS telemetry.**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-08T14:23:53Z
- **Completed:** 2026-03-08T14:25:20Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments
- Created `tests/hardware/thresholds.json` defining limits for latency, thermal envelope, and safety counters.
- Documented the validation methodology in `tests/hardware/METHODOLOGY.md`.
- Updated `SystemStatus` to track `command_latency_us` and `max_command_latency_us`.
- Extended `ArtisanFormatter` to include latency metrics in the `STATUS` CSV response (now 18 fields).
- Implemented real-time latency measurement in the `control_loop_task` around command processing.

## Task Commits

Each task was committed atomically:

1. **Task 1: Define Thresholds and Methodology** - `44ec6fd` (docs)
2. **Task 2: Instrument SystemStatus for Latency Tracking** - `c14a858` (feat)
3. **Task 3: Implement Latency Measurement Logic in Application Layer** - `c9b654e` (feat)

**Plan metadata:** `pending` (docs: complete plan)

## Files Created/Modified
- `tests/hardware/thresholds.json` - Defines acceptance limits for HIL tests.
- `tests/hardware/METHODOLOGY.md` - Explains how metrics are calculated and verified.
- `src/config/constants.rs` - Added latency fields to `SystemStatus`.
- `src/output/artisan.rs` - Updated `STATUS` response format and tests.
- `src/application/tasks.rs` - Added latency measurement logic to the main control task.

## Decisions Made
- **Internal Timing**: Latency is measured inside the firmware to isolate processing overhead from serial transport jitter.
- **µs Precision**: Microsecond precision is used for internal tracking to ensure accurate p95/p99 analysis in downstream tools.

## Deviations from Plan
None - plan executed exactly as written.

## Issues Encountered
None.

## Next Phase Readiness
- Firmware is now capable of reporting "ground truth" performance data.
- Ready for `85-02-PLAN.md` to implement the HIL Validation Runner.

---
*Phase: 85-hardware-acceptance-thresholds-and-real-roaster-validation*
*Completed: 2026-03-08*
