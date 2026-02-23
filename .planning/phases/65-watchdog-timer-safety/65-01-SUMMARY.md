---
phase: 65-watchdog-timer-safety
plan: 01
subsystem: safety
tags: [watchdog, esp32c3, telemetry]

# Dependency graph
requires:
  - phase: 64-documentation-consistency-fixes
    provides: ServiceContainer consistency and safety telemetry documentation
provides:
  - WatchdogFeeder service owning the esp_task_wdt handle and recording last failure metadata
  - ServiceContainer/app builder wiring so every task can borrow the feeder safely
  - control_loop_task telemetry that reports SAFETY WATCHDOG events and faults after two misses
affects:
  - 65-watchdog-timer-safety (next plan: LEDC guard and regression instrumentation)

# Tech tracking
tech-stack:
  added:
    - esp_bootloader_esp_idf esp_task_wdt helpers
  patterns:
    - Safety hardware services stored inside ServiceContainer mutexes for shared async/sync access
    - Control loop emits one SAFETY line per watchdog reason and tracks consecutive misses before faulting

key-files:
  created:
    - src/safety/watchdog.rs
  modified:
    - src/application/service_container.rs
    - src/application/app_builder.rs
    - src/application/tasks.rs
    - src/config/constants.rs
    - src/hardware/uart/tasks.rs

key-decisions:
  - "Leverage esp_task_wdt_{init,reset} from esp_bootloader_esp_idf so the WatchdogFeeder can feed hardware without pulling in the IDF runtime."
  - "Emit `SAFETY WATCHDOG` telemetry only when the failure reason changes to avoid duplicate instrumentation events."

patterns-established:
  - "Safety services own hardware handles through ServiceContainer mutex/RefCell wrappers for cross-task access."
  - "Control loop increments consecutive watchdog failures before flipping `fault_condition` and flooding SAFETY telemetry."

# Metrics
duration: 1 min
completed: 2026-02-23
---

# Phase 65: Watchdog Timer Safety Summary

**WatchdogFeeder wraps esp_task_wdt_reset, the control loop logs SAFETY WATCHDOG telemetry for failures, and the ServiceContainer shares the feeder safely with every async task.**

## Performance

- **Duration:** 1 min 12 s
- **Started:** 2026-02-23T16:59:35Z
- **Completed:** 2026-02-23T17:00:47Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments

- Created a hardware-backed `WatchdogFeeder` service that initializes `esp_task_wdt`, records the last feed time, and exposes reason strings for failures.
- Extended `ServiceContainer` and `AppBuilder` so the feeder is registered during boot and reachable from every task alongside the existing RoasterControl handles.
- Updated `control_loop_task` to log failures, update `SystemStatus`, emit a single `SAFETY WATCHDOG` telemetry line per reason, and escalate after two misses.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add WatchdogFeeder service and container wiring** - `58b014c`
2. **Task 2: Feed the watchdog every control cycle and expose failures** - `b0b1ce3`

**Plan metadata:** docs(65-01): complete watchdog plan

## Files Created/Modified

- `src/safety/watchdog.rs` - WatchdogFeeder service that owns `esp_task_wdt`, tracks the last feed, and exposes failure metadata.
- `src/application/service_container.rs` / `src/application/app_builder.rs` - Container helpers and builder wiring that initialize and expose the feeder to async tasks safely.
- `src/application/tasks.rs` - Control loop now feeds the watchdog, updates `SystemStatus`, logs SAFETY WATCHDOGFEED fails, and emits SAFETY telemetry when reasons change.
- `src/hardware/uart/tasks.rs` - Format tweak to satisfy `cargo fmt` so the helper remains in sync with formatter expectations.
- `src/config/constants.rs` - Documented the 100 ms watchdog feed cadence so the control loop knows how often to ping the hardware.

## Decisions Made

- Used `esp_task_wdt` helpers exported by `esp_bootloader_esp_idf` so the embedded target feeds the Task WDT without dragging in the IDF runtime.
- Limited SAFETY telemetry to one `SAFETY WATCHDOG` line per unique failure reason so instrumentation dashboards can correlate resets with root causes.

## Deviations from Plan

None - the plan described the required wiring and control loop behavior.

## Issues Encountered

- `cargo fmt -- --check` enforced a single-line closure in `src/hardware/uart/tasks.rs`; formatting the helper prevented the formatter from blocking verification.

## User Setup Required

None - no external service configuration was necessary.

## Next Phase Readiness

- WatchdogFeeder and status tracking are in place so Plan `65-02` (LEDC guard timeouts, regression instrumentation, SAFETY telemetry) can focus on the new instrumentation stack without reworking the loop feed logic.
