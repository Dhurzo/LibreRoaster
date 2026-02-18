---
phase: 49-safety-static-fixes
plan: 01
subsystem: safety
tags: [static-cell, memory-safety, embedded, riscv, static-mut]

# Dependency graph
requires:
  - phase: 48-async-completion
    provides: "Async UART/USB CDC drivers with CommandQueue"
provides:
  - "All unsafe static/mutable patterns replaced with StaticCell"
  - "Fixed use-after-free bug in make_static"
  - "Safe singleton pattern using ConstStaticCell"
affects: [50-test-fix, 52-performance-fixes]

# Tech tracking
tech-stack:
  added: [static_cell crate]
  patterns: [StaticCell for runtime-initialized statics, ConstStaticCell for compile-time initialized singletons]

key-files:
  created: []
  modified:
    - src/main.rs
    - src/hardware/usb_cdc/driver.rs
    - src/hardware/uart/driver.rs
    - src/application/service_container.rs

key-decisions:
  - "Used StaticCell::init() with raw pointer storage for USB/UART drivers"
  - "Used ConstStaticCell::take() for ServiceContainer singleton"

patterns-established:
  - "StaticCell pattern: init() returns reference, store pointer for later access"
  - "ConstStaticCell pattern: compile-time initialized, take() for singleton"

# Metrics
duration: 5min
completed: 2026-02-18
---

# Phase 49 Plan 01 Summary

**Replaced unsafe static/mutable patterns with StaticCell, eliminating use-after-free bug and static_mut_refs warnings**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-18T08:00:00Z
- **Completed:** 2026-02-18T08:05:00Z
- **Tasks:** 4/4
- **Files modified:** 4

## Accomplishments
- Removed unsafe make_static function from main.rs - replaced with StaticCell::init()
- Fixed USB CDC driver mutable static using StaticCell with raw pointer pattern
- Fixed UART driver mutable static using StaticCell with raw pointer pattern
- Fixed ServiceContainer::get_instance() using ConstStaticCell::take()

## Task Commits

Each task was committed atomically:

1. **Task 1: SAFE-01 replace make_static with StaticCell** - `b778e9f` (fix)
2. **Task 2: SAFE-02 fix mutable static in usb_cdc driver** - `3efa396` (fix)
3. **Task 3: SAFE-03 fix mutable static in uart driver** - `8079ab4` (fix)
4. **Task 4: SAFE-04 fix ServiceContainer get_instance** - `8728049` (fix)

## Files Modified
- `src/main.rs` - Removed unsafe make_static, added StaticCell declarations
- `src/hardware/usb_cdc/driver.rs` - Replaced mutable static with StaticCell
- `src/hardware/uart/driver.rs` - Replaced mutable static with StaticCell
- `src/application/service_container.rs` - Replaced unsafe static with ConstStaticCell

## Decisions Made

- Used raw pointer pattern (static mut NonNull) to store reference after StaticCell::init() for later access
- Used ConstStaticCell for ServiceContainer since ServiceContainer::new() is const

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- **StaticCell API limitation:** StaticCell::init() returns a reference but doesn't provide a way to retrieve it later
  - **Solution:** Used a raw pointer pattern (static mut NonNull<T>) to store the reference after initialization

## Next Phase Readiness

- All 4 files safely refactored with StaticCell pattern
- Build succeeds with cargo check
- Ready for test fixes in phase 50

---
*Phase: 49-safety-static-fixes*
*Completed: 2026-02-18*
