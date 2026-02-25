---
phase: 73-code-cleanup-architecture-decision
plan: 01
subsystem: cleanup
tags: [dead-code, logging, architecture]

# Dependency graph
requires:
  - phase: 72-centralized-conversion-regression-coverage
    provides: SensorConversionHub with sample() async method
provides:
  - Removed dead sync code from production binary
  - Documented logging architecture decision
affects: [logging, binary-size]

# Tech tracking
tech-stack:
  added: []
  patterns: [async-only sensor API]

key-files:
  created: []
  modified:
    - src/hardware/sensors/conversion.rs
    - src/control/roaster_refactored.rs
    - .planning/PROJECT.md

key-decisions:
  - "Removed sync methods (sample_sync, read_bean_sync, read_env_sync, read_sensor_sync) from SensorConversionHub - dead code since async API is used"
  - "Removed read_sensors_sync from RoasterControl - dead code for backwards compatibility"
  - "Chose log + esp-println over defmt for logging - simpler, no RTT integration needed, reliable UART0 output"

patterns-established:
  - "Async-only sensor API - production code uses async sample() method"

# Metrics
duration: 12min
completed: 2026-02-24
---

# Phase 73 Plan 1: Code Cleanup & Architecture Decision Summary

**Removed dead sync code from SensorConversionHub and RoasterControl, documented logging architecture decision**

## Performance

- **Duration:** 12 min
- **Started:** 2026-02-24T12:00:00Z
- **Completed:** 2026-02-24T12:11:59Z
- **Tasks:** 4
- **Files modified:** 4

## Accomplishments
- Removed sample_sync(), read_bean_sync(), read_env_sync(), read_sensor_sync() from SensorConversionHub
- Removed read_sensors_sync() from RoasterControl
- Added logging architecture decision to PROJECT.md (log + esp-println over defmt)
- Fixed pre-existing build issue in regression.rs (missing #[task] macro on stub)

## Task Commits

1. **Task 1: Remove dead sync methods from sensors/conversion.rs** - `a6fb2ba` (refactor)
2. **Task 2: Remove read_sensors_sync from roaster_refactored.rs** - `a6fb2ba` (refactor)
3. **Task 3: Document logging architecture decision in PROJECT.md** - `a6fb2ba` (refactor)
4. **Task 4: Verify tests pass (build verification for embedded)** - `a6fb2ba` (refactor)

**Plan metadata:** `a6fb2ba` (refactor: remove dead sync code and document logging architecture)

## Files Created/Modified
- `src/hardware/sensors/conversion.rs` - Removed dead sync methods
- `src/control/roaster_refactored.rs` - Removed read_sensors_sync method
- `.planning/PROJECT.md` - Added logging architecture decision
- `src/safety/regression.rs` - Fixed pre-existing build issue

## Decisions Made
- Removed sync methods from production - these were kept for "backwards compatibility" but never called
- Chose log + esp-println over defmt - simpler architecture, direct UART0 output, no RTT integration needed

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed pre-existing build issue in regression.rs**
- **Found during:** Build verification after sync code removal
- **Issue:** The regression_task() stub for non-riscv32 targets was missing the `#[task]` macro, causing build failure
- **Fix:** Added `#[embassy_executor::task]` macro to the fallback stub function
- **Files modified:** src/safety/regression.rs
- **Verification:** Build succeeds with no errors
- **Committed in:** a6fb2ba

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Build was blocked without this fix. The fix was necessary to verify the sync code removal.

## Issues Encountered
- Tests are configured for riscv32 embedded target and cannot run on host - verified build success instead

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 73 plan 1 complete
- Ready for 73-02 plan (SyncCell consolidation)
- Dead code removed, logging decision documented

---
*Phase: 73-code-cleanup-architecture-decision*
*Completed: 2026-02-24*
