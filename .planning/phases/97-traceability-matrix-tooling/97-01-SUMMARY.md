---
phase: 97-traceability-matrix-tooling
plan: 01
subsystem: instrumentation
tags: [trace, logging, heapless, embassy-time-driver]

# Dependency graph
requires:
  - phase: 96-error-architecture-implementation
    provides: error source chaining with AppError.source() and source metadata
provides:
  - deterministic `TRACE` formatter plus TraceId/TracedCommand helpers and tests
  - artisan channels that carry `TracedCommand` through UART/USB queues and fallback handling
  - control-loop actuator/telemetry/guard events tagged by TraceId
affects: [98-hil-validation-infrastructure, regression-triage]

# Tech tracking
tech-stack:
  added:
    - embassy-time-driver
  patterns:
    - TraceId-propagating `TracedCommand` for queue/control/event correlation
    - deterministic `heapless::String` formatting for `TRACE` events

key-files:
  created:
    - src/logging/traceability.rs
    - src/host_time_driver.rs
  modified:
    - Cargo.toml
    - src/lib.rs
    - src/application/service_container.rs
    - src/hardware/uart/tasks.rs
    - src/hardware/usb_cdc/tasks.rs
    - src/application/tasks.rs
    - tests/mock_uart_integration.rs
    - tests/usb_cdc_tests.rs
    - tests/usb_instrumentation_runner.rs
    - tests/command_errors.rs

key-decisions:
  - "TRACE instrumentation stays on the existing Artisan output channel so hosts consume one unified stream."
  - "Every Artisan command now travels as a `TracedCommand`, enabling consistent TraceId correlation across queue, actuator, telemetry, and guard events."
patterns-established:
  - "TraceId + heapless formatter pattern for deterministic `TRACE` lines."
  - "Queue instrumentation always logs enqueue/dequeue/fallback depth for full matrix visibility."

# Metrics
completed: 2026-03-20
---

# Phase 97 Traceability Matrix Tooling Summary

**TraceId-wrapped Artisan commands now emit deterministic TRACE events from queue through guard, restoring the full correlation matrix.**

## Performance

- **Duration:** 0.23 min
- **Started:** 2026-03-20T13:23:00Z
- **Completed:** 2026-03-20T13:23:14Z
- **Tasks:** 3
- **Files modified:** 13

## Accomplishments

- Created the traceability helper module with TraceId/TracedCommand primitives and deterministic TRACE formatters/tests.
- Propagated `TracedCommand` through UART/USB queues and ServiceContainer so enqueue/dequeue/fallback paths emit TRACE entries.
- Tagged control-loop actuator, telemetry, and guard events with the same TraceId to complete the traceability matrix.

## Task Commits

1. **Task 1: Build traceability helper module** - `b147647` (feat)
2. **Task 2: Wire queues and ServiceContainer to trace commands** - `8334a4e` (feat)
3. **Task 3: Emit control loop actuator/telemetry/guard events** - `465ee2d` (feat)

## Files Created/Modified

- `src/logging/traceability.rs` - TraceId generator, helper APIs, and deterministic TRACE formatter with unit tests.
- `src/host_time_driver.rs` - Host-side embassy-time driver stub supplying `_embassy_time_now`/`_embassy_time_schedule_wake` for cargo test --lib.
- `Cargo.toml`, `src/lib.rs`, `src/application/service_container.rs`, `src/hardware/uart/tasks.rs`, `src/hardware/usb_cdc/tasks.rs`, `src/application/tasks.rs` - Reworked channels, queues, and control loop to carry `TracedCommand` and emit TRACE events.
- `tests/mock_uart_integration.rs`, `tests/usb_cdc_tests.rs`, `tests/usb_instrumentation_runner.rs`, `tests/command_errors.rs` - Updated helpers to accept `TracedCommand` values and ignore TRACE lines.

## Decisions Made

- TRACE instrumentation shares the Artisan output channel so hosts ingest responses and trace data together.
- Every queue/actuation/telemetry/guard event is tagged by the same TraceId carried in `TracedCommand`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added a host time driver stub for embassy-time**
- **Found during:** Task 1 (traceability helper module)
- **Issue:** `cargo test --lib` failed because `_embassy_time_now` and `_embassy_time_schedule_wake` were missing on the host build
- **Fix:** Added the `embassy-time-driver` dependency, created `src/host_time_driver.rs`, and gated it under `std`-enabled builds
- **Files modified:** `Cargo.toml`, `src/lib.rs`, `src/host_time_driver.rs`
- **Verification:** `cargo test --lib`
- **Committed in:** `b147647`

---

**Total deviations:** 1 auto-fixed (blocking)  
**Impact on plan:** Necessary to unblock host tests; instrumentation scope remained as planned.

## Issues Encountered

- Missing embassy-time driver symbols on the host prevented `cargo test --lib`; resolved by the new host driver stub.

## User Setup Required

None - instrumentation runs entirely on-device with no external services.

## Next Phase Readiness

- Traceability instrumentation now spans the entire command matrix, so Phase 98 (HIL Validation Infrastructure) can consume rich TRACE streams.
- No blockers remain; future work can focus on consuming the new TRACE data for regression triage and telemetry dashboards.

---
*Phase: 97-traceability-matrix-tooling*  
*Completed: 2026-03-20*
