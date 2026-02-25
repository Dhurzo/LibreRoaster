---
phase: 53-integrate-async-temp
plan: 03
subsystem: control
tags: [async, max31856, temperature, embassy, ssr]

# Dependency graph
requires:
  - phase: 52-01
    provides: MAX31856 async read_temperature_async() with embassy Timer
provides:
  - Control loop uses async temperature reading (no 160ms blocking)
  - Temperature read no longer blocks async executor
  - read_sensors is async and awaited properly
affects: [future async operations, sensor reliability]

# Tech tracking
tech-stack:
  added: []
  patterns: [async sensor reads via ownership take/replace pattern]

key-files:
  created: []
  modified:
    - src/control/roaster_refactored.rs
    - src/application/app_builder.rs
    - src/application/tasks.rs
    - src/application/service_container.rs
    - src/hardware/max31856.rs

key-decisions:
  - "Use concrete Max31856<BtSpi/EtSpi> instead of Box<dyn Thermometer> to enable async calls"
  - "Use take/replace pattern in ServiceContainer to call async methods on owned RoasterControl"

patterns-established:
  - "Async methods on types stored in RefCell behind critical_section: take ownership, async ops, put back"

# Metrics
duration: 14 min
completed: 2026-02-18
---

# Phase 53 Plan 03: Async Temperature Reading Gap Closure

**Concrete Max31856 types enable true async temperature reading without blocking executor**

## Performance

- **Duration:** 14 min
- **Started:** 2026-02-18T15:42:29Z
- **Completed:** 2026-02-18T15:56:44Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments
- RoasterControl now stores concrete Max31856<BtSpi> and Max31856<EtSpi> instead of Box<dyn Thermometer>
- read_sensors() now calls async read_temperature_async().await instead of sync version
- Control loop uses async sensor reading via ServiceContainer::roaster_async_sensor_read()
- The 160ms MAX31856 conversion time no longer blocks the async executor

## Task Commits

1. **Task 1+2: Change RoasterControl and AppBuilder to use concrete Max31856 types** - `27edce2` (feat)
2. **Task 3: Use async read_sensors() in control loop** - `a15b860` (feat)

**Plan metadata:** (docs commit after this summary)

## Files Created/Modified
- `src/control/roaster_refactored.rs` - Stores concrete Max31856 types, async read_sensors()
- `src/application/app_builder.rs` - Passes concrete sensor types to RoasterControl
- `src/application/tasks.rs` - Uses roaster_async_sensor_read() in control loop
- `src/application/service_container.rs` - Added roaster_async_sensor_read() method
- `src/hardware/max31856.rs` - Added BtSpi and EtSpi type aliases

## Decisions Made
- Used concrete Max31856 types instead of Box<dyn Thermometer> to access async methods
- Used take/replace pattern in ServiceContainer to call async methods (ownership pattern)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## Next Phase Readiness
- Phase 53 (Async Temperature Integration) is now complete
- Ready for next phase in the roadmap

---
*Phase: 53-integrate-async-temp*
*Completed: 2026-02-18*
