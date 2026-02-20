---
phase: 60-concurrent-sensor-test
plan: 01
subsystem: testing
tags: [rust, futures, embassy, async, metrics, instrumentation]

# Dependency graph
requires:
  - phase: 59-command-transport-resilience
    provides: queue processor instrumentation and async sensor read stability work that this harness now extends and validates under load
provides:
  - a host-side concurrent sensor read harness that asserts every read succeeds and the async mutex never reports more than one holder
  - the test-only async lock depth telemetry helpers gated behind `async-lock-depth-metrics`
  - README documentation that reuses the test command and metrics when citing ASYNC-06 coverage
affects:
  - future ASYNC-06 verification audits and any subsequent concurrency validation phases that reference this instrumentation

# Tech tracking
tech-stack:
  added:
    - async-lock-depth-metrics feature to gate AtomicUsize-based mutex diagnostics without touching production builds
  patterns:
    - feature-gated `AsyncLockDepthGuard` ensures the embassy mutex instrumentation only compiles when the test command explicitly requests it
    - the harness resets instrumentation before and after the ThreadPool batch so telemetry remains reproducible between runs

key-files:
  created: []
  modified:
    - `tests/concurrent_sensor_test.rs` - new host `ThreadPool` batch, join_all usage, and instrumentation assertions for `max_async_lock_depth`
    - `src/application/service_container.rs` - feature-gated depth counters, helpers, and re-exports plus the new guard around `roaster_async_sensor_read()`
    - `Cargo.toml` - declares the `async-lock-depth-metrics` feature so instrumentation stays out of releases
    - `README.md` - documents the concurrent sensor read command, instrumentation insights, and why it proves ASYNC-06

key-decisions:
  - "Expose the async lock depth helpers through an `async-lock-depth-metrics` feature so integration tests can see the telemetry while production builds stay untouched."
  - "Execute ten concurrent `ServiceContainer::roaster_async_sensor_read()` futures via `ThreadPool::spawn_with_handle` and assert both the `ContainerError` results and `max_async_lock_depth` metric before resetting it for repeatability."

patterns-established:
  - "Feature-gated instrumentation modules when hooking embassy mutex diagnostics"
  - "Document instrumentation/metric commands in README so auditors can replay the ASYNC-06 proof"

# Metrics
duration: 6 min 36 sec
completed: 2026-02-20
---

# Phase 60 Plan 01 Summary

**Concurrent sensor read harness with async lock depth telemetry proves ASYNC-06 compliance for the host target.**

## Performance

- **Duration:** 6 min 36 sec
- **Started:** 2026-02-20T12:28:00Z
- **Completed:** 2026-02-20T12:34:33Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments
- Hardened `tests/concurrent_sensor_test.rs` to run ten concurrent host sensor reads and assert that every `ContainerError` result is `Ok` while capturing instrumentation data.
- Added feature-gated `AsyncLockDepthGuard` instrumentation in `ServiceContainer` plus helpers for reading and resetting the max depth so the harness can detect dropped locks or concurrent holders.
- Documented the run command, instrumentation helpers, and ASYNC-06 audit tie-in inside the README so operators can reproduce the proof.

## Task Commits

1. **Task 1: Strengthen the host concurrent sensor read harness** - `7a10225` (feat)
2. **Task 2: Expose async mutex depth metrics for the test** - `e4ea184` (feat)
3. **Task 3: Document the concurrent sensor read integration test** - `ba95ed5` (docs)

**Plan metadata:** docs(60-01): complete concurrent sensor read plan (pending)

_Note: 3 task commits complete the work described above._

## Files Created/Modified
- `tests/concurrent_sensor_test.rs` - Host-side ThreadPool harness that verifies every read and the `max_async_lock_depth` instrumentation
- `src/application/service_container.rs` - Feature-gated AsyncLockDepthGuard, helper re-exports, and guard around `roaster_async_sensor_read()`
- `Cargo.toml` - Declares the `async-lock-depth-metrics` feature for test instrumentation
- `README.md` - Walkthrough for running the host test, interpreting the metrics, and citing ASYNC-06

## Decisions Made
- Used the `async-lock-depth-metrics` feature so the instrumentation helpers and guard only compile when the host test requests them, keeping production builds clean while still powering the integration harness.
- Reset the metrics before and after the batch run to ensure repeated executions report `max_async_lock_depth` correctly and the audit command is reproducible.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added the `async-lock-depth-metrics` feature because `#[cfg(test)]` instrumentation was invisible to integration tests**
- **Found during:** Task 2
- **Issue:** Gating the instrumentation helpers and guard solely on `cfg(test)` meant the integration test crate never saw the depth metrics or guard, so the harness couldn't validate concurrent holders.
- **Fix:** Gate the instrumentation under `cfg(any(test, feature = "async-lock-depth-metrics"))`, provide no-op stubs when the feature is absent, and document running the test with the new feature so the guard and helpers are available.
- **Files modified:** `src/application/service_container.rs`, `Cargo.toml`
- **Verification:** `cargo test --features async-lock-depth-metrics --target x86_64-unknown-linux-gnu --test concurrent_sensor_test`
- **Committed in:** `e4ea184`

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** The feature gate was essential for getting instrumentation telemetry into the integration test without dragging it into release builds.

## Issues Encountered
- None

## User Setup Required
- None - no external service configuration is necessary for this instrumentation work.

## Next Phase Readiness
- Concurrent sensor read instrumentation is documented, audited, and ready for any follow-up plans that rely on ASYNC-06 verification.
