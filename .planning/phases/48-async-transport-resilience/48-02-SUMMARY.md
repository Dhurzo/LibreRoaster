---
phase: 48-async-transport-resilience
plan: 02
subsystem: hardware
tags: [usb, cdc, back-pressure, embassy, async]

# Dependency graph
requires:
  - phase: 47-deterministic-fan-control
    provides: FanController with LEDC serialization
provides:
  - Back-pressure aware USB CDC driver with WouldBlock error handling
  - USB writer task that yields on congestion (1-10ms exponential backoff)
  - embassy_usb dependency for future async USB stack
affects: [async-transport-resilience]

# Tech tracking
tech-stack:
  added: [embassy-usb]
  patterns: [back-pressure handling, exponential backoff, async I/O]

key-files:
  created: []
  modified: [src/hardware/usb_cdc/driver.rs, src/hardware/usb_cdc/tasks.rs, Cargo.toml]

key-decisions:
  - Used WouldBlock error variant to signal congestion instead of busy-waiting
  - Exponential backoff (1ms → 10ms) prevents executor starvation

patterns-established:
  - Back-pressure aware writer task with yield-on-congestion

# Metrics
duration: ~3 min
completed: 2026-02-18
---

# Phase 48 Plan 2: USB CDC Back-Pressure Summary

**Back-pressure aware USB CDC driver with yield-on-congestion, enabling executor responsiveness during DMA endpoint congestion**

## Performance

- **Duration:** ~3 min
- **Started:** 2026-02-18T05:55:00Z
- **Completed:** 2026-02-18T06:00:00Z
- **Tasks:** 3/3
- **Files modified:** 3

## Accomplishments
- Added embassy_usb dependency for future async USB device stack
- Extended UsbCdcDriver with WouldBlock error and is_write_ready() method
- Implemented write_bytes_with_timeout() for explicit back-pressure handling
- USB writer task now yields with exponential backoff (1ms → 10ms) when WouldBlock received
- Added warning logs for prolonged back-pressure (>100ms)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add embassy_usb dependency** - `aba0fa8` (feat)
2. **Task 2: Create async USB CDC driver with AsyncWrite** - `16fd17b` (feat)
3. **Task 3: Make USB writer task back-pressure aware** - `b98c5f1` (feat)

**Plan metadata:** (to be created after SUMMARY.md)

## Files Created/Modified
- `Cargo.toml` - Added embassy-usb dependency
- `src/hardware/usb_cdc/driver.rs` - Added WouldBlock error, is_write_ready(), write_bytes_with_timeout()
- `src/hardware/usb_cdc/tasks.rs` - Back-pressure aware writer with exponential backoff

## Decisions Made
- Used WouldBlock error variant to signal congestion instead of busy-waiting
- Exponential backoff (1ms → 10ms) prevents executor starvation during USB congestion

## Deviations from Plan

None - plan executed exactly as written.

Note: Pre-existing compilation errors in uart/tasks.rs (from plan 48-01) are unrelated to this plan's deliverables.

## Issues Encountered
- Pre-existing errors in uart/tasks.rs (VecDeque import, type inference issues) - from plan 48-01, not related to USB CDC changes
- USB CDC code compiles successfully with no new errors

## Next Phase Readiness
- USB CDC back-pressure handling implemented as specified
- Ready for 48-03-PLAN.md (command queue with reject-on-full behavior)
