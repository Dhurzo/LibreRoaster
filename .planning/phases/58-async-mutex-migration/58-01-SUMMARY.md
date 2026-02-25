---
phase: 58-async-mutex-migration
plan: 01
subsystem: embedded/async
tags: [embassy-sync, mutex, async, embedded, race-condition]

# Dependency graph
requires:
  - phase: 57-safety-rollup
    provides: "v3.0 critical safety fixes, RoasterControl implementation"
provides:
  - "embassy_sync::Mutex for async-safe RoasterControl access"
  - "Dual API: sync (deprecated) + async with_roaster_async()"
  - "Eliminated take/replace race condition in sensor reading"
affects: [58-02, 58-03, 58-04]

# Tech tracking
tech-stack:
  added: [embassy_sync]
  patterns: [async-mutex, dual-api-sync-async]

key-files:
  created: []
  modified:
    - src/application/service_container.rs

key-decisions:
  - "Use EmbassyMutex<CriticalSectionRawMutex, Option<RoasterControl>> for async access"
  - "Keep critical_section::Mutex as roaster_sync for ISR compatibility"
  - "Add #[deprecated] to sync methods to guide migration"

patterns-established:
  - "Async mutex pattern: lock().await for concurrent async access"
  - "Dual API pattern: deprecated sync for ISR, new async for tasks"

# Metrics
duration: 2min
completed: 2026-02-19
---

# Phase 58 Plan 01: Async Mutex Migration Summary

**Migrated ServiceContainer to use embassy_sync::Mutex for async-safe access, eliminating the unsafe take/replace pattern in sensor reading**

## Performance

- **Duration:** 2 min
- **Started:** 2026-02-19T11:27:43Z
- **Completed:** 2026-02-19T11:36:08Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments
- ServiceContainer now has dual mutex: `roaster` (EmbassyMutex for async) and `roaster_sync` (critical_section::Mutex for ISR)
- Added new async method `with_roaster_async()` using `lock().await` pattern
- Added `#[deprecated]` attribute to sync methods `with_roaster()` and `with_roaster_mut()`
- Updated `roaster_async_sensor_read()` to use `lock().await` instead of unsafe take/replace

## Task Commits

1. **Task 1: Update ServiceContainer to use embassy_sync::Mutex** - ServiceContainer now has roaster: EmbassyMutex field
2. **Task 2: Create async with_roaster_async method** - New async method using lock().await
3. **Task 3: Add deprecated attribute to sync with_roaster methods** - Both methods marked deprecated

**Plan metadata:** Completed via previous session commits

## Files Created/Modified
- `src/application/service_container.rs` - Main migration: EmbassyMutex, async method, deprecated sync methods
- `src/application/app_builder.rs` - Updated initialization to use roaster_sync
- `tests/mock_uart_integration.rs` - Updated test to use roaster_sync

## Decisions Made
- Used dual mutex approach: EmbassyMutex for async contexts, critical_section::Mutex for ISR contexts
- This maintains backward compatibility while enabling safe concurrent async access

## Deviations from Plan

None - plan executed as specified. The work was completed in a previous session (commits already present).

## Issues Encountered
None

## Next Phase Readiness
- Phase 58 Plan 01 complete - async mutex migration foundation laid
- Ready for subsequent plans in Phase 58 that will use the new async API

---
*Phase: 58-async-mutex-migration*
*Completed: 2026-02-19*
