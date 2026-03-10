---
phase: 79-test-infrastructure
plan: 05
subsystem: testing
tags: [rust, testing, critical-section, async]

# Dependency graph
requires:
  - phase: 79-03
    provides: Shared stub helpers and test shim exports from `libreroaster::common` so host suites compile.
provides:
  - A serialized `TestCriticalSection` guard that lets async sensor reads mutate `ServiceContainer::roaster_sync` without `RefCell` clashes.
  - Helper-driven `critical_section::with` usage that drops the `RefCell` borrow immediately after replacement so each async reader reenters sequentially.
  - Phase 79-04 (mock UART suite) can trust `roaster_sync` access and focus on buffer/stream semantics.
  - Phase 80 (handler pattern work) relies on the host test harness proving concurrency safety.

# Tech tracking
tech-stack:
  added: []
  patterns: ["Atomic flag guard for critical_section", "Helper that limits borrow scope of `critical_section::with`"]

key-files:
  created: []
  modified: [tests/concurrent_sensor_test.rs]

key-decisions:
  - "Use an AtomicBool-backed `TestCriticalSection` guard so each host reader toggles the flag before interacting with `roaster_sync`."
  - "Keep `roaster_sync` borrows inside a helper so the critical section releases immediately after replacing the `RefCell`, giving each async task a clean entry."

patterns-established:
  - "Host-critical sections protecting test helpers now gate access through an atomic spin loop instead of the previous no-op stub."
  - "Mutations inside `critical_section::with` live in small helper functions so the borrow window remains predictable for concurrent readers."

# Metrics
completed: 2026-02-28
---

# Phase 79 Plan 05: Test Infrastructure Summary

**Atomic critical-section guarding keeps `concurrent_sensor_reads_verify_async_mutex` from borrowing `roaster_sync` twice and lets the host test suite finish.**

## Performance

- **Duration:** 0 min
- **Started:** 2026-02-28T19:44:23Z
- **Completed:** 2026-02-28T19:44:49Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Replaced the stub `TestCriticalSection` with an `AtomicBool` guard that spins until the flag flows from `false` to `true`, giving each host reader exclusive access before mutating the `RefCell`.
- Encapsulated `ServiceContainer::roaster_sync` mutations inside `replace_sync_roaster`, which keeps the borrow inside `critical_section::with`, ensuring the guard drops immediately before async sensor reads begin.
- Verified `cargo test tests/concurrent_sensor_test.rs` passes on x86_64 and proves `async_lock_depth_max_for_tests()` never exceeds 1.

## Task Commits

Each task is committed atomically:

1. **Task 1: Implement a robust TestCriticalSection guard** - `c0c3880` (fix(79-05): implement host critical section guard)
2. **Task 2: Limit the RefCell borrow window in the concurrency test** - `1953193` (fix(79-05): minimize roaster_sync borrow window)

**Plan metadata:** docs(79-05): complete guard roaster_sync critical section plan

## Files Created/Modified
- `tests/concurrent_sensor_test.rs` - Added an `AtomicBool` guard, documented the new helper, and kept `roaster_sync` borrows confined to the critical section so async reads serialize safely.

## Decisions Made
- Swap the stub `TestCriticalSection` for an atomic guard to serialize `roaster_sync` access under host concurrency.
- The helper-driven borrow of `roaster_sync` keeps the `RefCell` borrow local to the guard so it never overlaps with another reader.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Align `replace_sync_roaster` with the critical_section API**

- **Found during:** Task 3 (running `cargo test tests/concurrent_sensor_test.rs`)
- **Issue:** The helper expected `&CriticalSection` while `critical_section::with` supplies `CriticalSection`, so `Mutex::borrow` did not accept the argument and compilation failed.
- **Fix:** Updated `replace_sync_roaster` to take the token by value so the critical-section guard and borrow both use the same type.
- **Files modified:** `tests/concurrent_sensor_test.rs`
- **Verification:** `cargo test tests/concurrent_sensor_test.rs` completes on x86_64 with no type errors.
- **Committed in:** `0db1a63`

## Issues Encountered
- The host mock UART suite remains blocked by its buffer/stream semantics (plan 79-04) but now relies on the safe critical section provided here.

## User Setup Required
- None — this plan touches only test infrastructure already in the repo.

## Next Phase Readiness
- The host sensor concurrency test now completes, so plan 79-04 can focus solely on the mock UART buffer expectations while trusting `roaster_sync` access to stay serialized.
- Phase 80 handler-pattern work can rely on these guard guarantees when validating host behavior.

---
*Phase: 79-test-infrastructure*
*Completed: 2026-02-28*
