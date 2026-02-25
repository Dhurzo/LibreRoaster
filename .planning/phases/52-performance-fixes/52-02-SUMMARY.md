---
phase: 52-performance-fixes
plan: 02
subsystem: hardware
tags: [esp32c3, ledc, pwm, timer, ssr, fan]

# Dependency graph
requires:
  - phase: 51-documentation
    provides: Project context and documentation
provides:
  - Separate LEDC timers for SSR and Fan
  - SSR runs at ~1Hz for zero-crossing control
  - Fan runs at 25kHz for silent operation
affects: [future hardware phases]

# Tech tracking
tech-stack:
  added: []
  patterns: [Independent LEDC timer configuration per channel]

key-files:
  created: []
  modified:
    - src/config/constants.rs
    - src/main.rs
    - src/hardware/ledc_bus.rs

key-decisions:
  - "Timer0 for SSR at 1Hz (zero-crossing control)"
  - "Timer1 for Fan at 25kHz (silent operation)"

patterns-established:
  - "Independent timer per PWM channel"

# Metrics
duration: 3min
completed: 2026-02-18
---

# Phase 52 Plan 02: Separate LEDC Timers Summary

**SSR uses Timer0 at ~1Hz for zero-crossing control, Fan uses Timer1 at 25kHz for silent operation**

## Performance

- **Duration:** 3 min
- **Started:** 2026-02-18T10:35:25Z
- **Completed:** 2026-02-18T10:38:26Z
- **Tasks:** 3/3
- **Files modified:** 3

## Accomplishments
- Added LEDC timer constants (SSR_LEDC_TIMER=0, FAN_LEDC_TIMER=1)
- Configured Timer0 for SSR at 1Hz and Timer1 for Fan at 25kHz
- Removed unsafe timer_ref lifetime extension pattern
- Updated LedcBus to accept and store timer numbers

## Task Commits

Each task was committed atomically:

1. **Task 1: Add timer constants to config/constants.rs** - `ff55cde` (feat)
2. **Task 2: Configure separate LEDC timers in main.rs** - `0dccd36` (feat)
3. **Task 3: Update LedcBus to work with separate timers** - `deb2d07` (feat)

## Files Created/Modified

- `src/config/constants.rs` - Added SSR_LEDC_TIMER and FAN_LEDC_TIMER constants
- `src/main.rs` - Configured separate timers, removed unsafe timer_ref pattern
- `src/hardware/ledc_bus.rs` - Updated to accept timer numbers in constructor

## Decisions Made

- Timer0 for SSR at 1Hz (zero-crossing control) - Required for SSR zero-crossing operation
- Timer1 for Fan at 25kHz (silent operation) - Inaudible frequency for fan PWM

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None

## Next Phase Readiness

Phase 52 complete. All LEDC timer separation work done. Ready for next milestone.
---
*Phase: 52-performance-fixes*
*Completed: 2026-02-18*
