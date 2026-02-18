---
phase: 48-async-transport-resilience
plan: 01
subsystem: hardware/uart
tags: [embassy, async, uart, embedded-io-async, heapless]

# Dependency graph
requires:
  - phase: 47-deterministic-fan-control
    provides: FanController and SSR reliability complete
provides:
  - Async UART driver using embedded_io_async traits
  - Buffered event queue for UART input (256-byte ring buffer)
  - Non-blocking UART reads and writes that never block the executor
affects: [phase 48-02, phase 48-03, phase 48-04]

# Tech tracking
tech-stack:
  added:
    - embedded-io-async = "0.6.1" (async I/O traits)
    - heapless = "0.9.2" (Deque for event queue)
    - embassy-usb = "0.5.0" (from previous session)
  patterns:
    - Async UART using esp-hal Async mode with into_async() conversion
    - Buffered event queue separating I/O from command parsing
    - Ring buffer with oldest-drop behavior for back-pressure

key-files:
  created: []
  modified:
    - Cargo.toml - added embedded-io-async and updated heapless
    - src/hardware/uart/driver.rs - async UART with embedded_io_async traits
    - src/hardware/uart/tasks.rs - Deque-based event queue
    - src/hardware/usb_cdc/driver.rs - fixed WouldBlock variant
    - src/application/tasks.rs - fixed type mismatch

key-decisions:
  - "Used esp-hal's built-in async UART (into_async()) instead of external embassy-uart crate"
  - "Upgraded heapless to 0.9.2 for Deque support (VecDeque was removed in 0.8)"
  - "Event queue uses ring buffer with oldest-drop when full to handle bursts"

patterns-established:
  - "Async UART pattern: create blocking UART, call into_async(), use embedded_io_async traits"
  - "Event queue pattern: push to Deque in reader, pop in processor, separate I/O from parsing"

# Metrics
duration: 9min
completed: 2026-02-18
---

# Phase 48 Plan 1: Async UART Driver with Buffered Event Queue Summary

**Async UART driver using embedded_io_async traits with 256-byte Deque-based event queue for non-blocking logging**

## Performance

- **Duration:** 9 min
- **Started:** 2026-02-18T05:52:55Z
- **Completed:** 2026-02-18T06:01:50Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments
- Converted UART driver from blocking to async mode using embedded_io_async::Read/Write traits
- Added heapless::Deque (256 bytes) for buffering incoming UART data
- Separated I/O from parsing - reader pushes to queue, processor pops complete lines
- Ring buffer behavior: drops oldest when full, handles command bursts gracefully

## Task Commits

Each task was committed atomically:

1. **Task 1: Add embassy_usart/embedded-io-async dependency** - `c1e5dca` (feat)
2. **Task 2: Create async UART driver with AsyncRead/AsyncWrite** - `663adf3` (feat)
3. **Task 3: Add buffered event queue to UART reader task** - `220edcf` (feat)

**Plan metadata:** `220edcf` (docs: complete plan)

## Files Created/Modified
- `Cargo.toml` - Added embedded-io-async = "0.6.1", heapless upgraded to 0.9.2
- `src/hardware/uart/driver.rs` - Async UART with embedded_io_async traits (into_async mode)
- `src/hardware/uart/tasks.rs` - Deque-based event queue for buffered input
- `src/hardware/usb_cdc/driver.rs` - Fixed missing WouldBlock variant in Display impl
- `src/application/tasks.rs` - Fixed type mismatch in output channel send

## Decisions Made
- Used esp-hal's built-in async UART instead of external embassy-uart (older, less compatible)
- Upgraded heapless to 0.9.2 (VecDeque was renamed to Deque in 0.8, removed in 0.9)
- Event queue uses critical_section for thread-safe access to static mut

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed pre-existing compilation errors**

- **Found during:** Task 2 (UART driver conversion)
- **Issue:** Pre-existing bugs in USB CDC driver (missing WouldBlock variant) and application tasks (type mismatch) were blocking compilation
- **Fix:** Added WouldBlock to UsbCdcError Display impl, fixed try_send type mismatch in application/tasks.rs
- **Files modified:** src/hardware/usb_cdc/driver.rs, src/application/tasks.rs
- **Verification:** cargo check passes for riscv32 target
- **Committed in:** 220edcf (part of Task 3)

**2. [Rule 3 - Blocking] Corrected heapless version for Deque support**

- **Found during:** Task 3 (event queue implementation)
- **Issue:** heapless 0.8.0 doesn't have VecDeque (renamed to Deque in 0.9)
- **Fix:** Upgraded to heapless 0.9.2 and changed import from VecDeque to Deque
- **Files modified:** Cargo.toml, src/hardware/uart/tasks.rs
- **Verification:** cargo check passes with new heapless
- **Committed in:** 220edcf (part of Task 3)

---

**Total deviations:** 2 auto-fixed (both blocking issues)
**Impact on plan:** Both fixes necessary for compilation. No scope creep.

## Issues Encountered
- Pre-existing USB CDC Display impl was incomplete (missing WouldBlock variant)
- Application tasks had type mismatch on output channel send
- heapless version incompatibility (VecDeque not available in 0.8.0)

## Next Phase Readiness
- UART async driver and event queue complete - ready for plan 48-02 (USB CDC back-pressure)
- The event queue architecture can be reused for USB CDC command buffering
