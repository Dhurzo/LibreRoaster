---
phase: 54-clean-up-tech-debt
plan: 03
subsystem: testing
tags: [integration-tests, std, host-target, Embassy, conditional-compilation]

# Dependency graph
requires:
  - phase: 54-clean-up-tech-debt
    provides: Phase 54-01 (dead code removal) and 54-02 (warnings fixed)
provides:
  - Library compiles on x86_64-unknown-linux-gnu with --features std
  - RoasterControl conditional compilation for host target
  - Test infrastructure ready for host execution (with caveats)
affects: [testing, CI, development-workflow]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Conditional compilation with cfg(target_arch)
    - PhantomData for type preservation on host targets

key-files:
  created: []
  modified:
    - src/hardware/max31856.rs - SPI types now conditional on riscv32
    - src/control/roaster_refactored.rs - Added PhantomData for host target
    - src/hardware/usb_cdc/driver.rs - StaticCell gated for riscv32
    - src/hardware/usb_cdc/tasks.rs - Function visibility for tests
    - src/config/constants.rs - Added Default for SsrHardwareStatus

key-decisions:
  - "Used PhantomData to maintain RoasterControl type consistency on host"
  - "Made bt_spi/et_spi modules conditional on riscv32 target"
  - "Tests requiring ESP-specific hardware gated with cfg attributes"

patterns-established:
  - "Use #[cfg(target_arch)] for ESP-specific type aliases"
  - "Provide host-compatible alternatives with PhantomData for generic types"

# Metrics
duration: 8min
completed: 2026-02-18
---

# Phase 54 Plan 3: Integration Tests with std Feature Summary

**Library compiles on x86_64-unknown-linux-gnu with --features std, enabling host-based testing infrastructure**

## Performance

- **Duration:** 8 min
- **Started:** 2026-02-18T16:59:21Z
- **Completed:** 2026-02-18T17:07:00Z
- **Tasks:** 1 (partial completion of 3)
- **Files modified:** 11

## Accomplishments
- Library now compiles on host target (x86_64-unknown-linux-gnu) with --features std
- RoasterControl struct conditionally compiled with PhantomData for non-riscv32 targets
- SPI type aliases (bt_spi, et_spi) now gated behind riscv32 cfg
- SsrHardwareStatus enum derives Default trait
- Test infrastructure partially enabled for host target

## Task Commits

1. **Task 1: Verify integration tests compile on host target** - `e1c4b77` (fix)
   - Library compiles with conditional compilation

**Plan metadata:** `e1c4b77` (fix: enable integration tests to compile on host target)

## Files Created/Modified
- `src/hardware/max31856.rs` - SPI type aliases now conditional on riscv32
- `src/control/roaster_refactored.rs` - Added PhantomData fields for host target
- `src/hardware/usb_cdc/driver.rs` - StaticCell gated behind riscv32 cfg
- `src/hardware/usb_cdc/tasks.rs` - Made function pub for test access
- `src/config/constants.rs` - Added Default for SsrHardwareStatus
- Multiple test files updated for host compatibility

## Decisions Made
- Used PhantomData to maintain RoasterControl type consistency on host targets
- Made bt_spi/et_spi modules conditional on riscv32 target
- Tests requiring ESP-specific hardware gated with cfg attributes

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Library uses ESP-specific types preventing host compilation**
- **Found during:** Task 1 (Verify integration tests compile)
- **Issue:** max31856.rs had unconditional esp_hal type aliases, RoasterControl had concrete SPI types
- **Fix:** Added #[cfg(target_arch = "riscv32")] to SPI modules, added PhantomData to RoasterControl
- **Files modified:** src/hardware/max31856.rs, src/control/roaster_refactored.rs
- **Verification:** Library compiles on x86_64-unknown-linux-gnu
- **Committed in:** e1c4b77

**2. [Rule 1 - Bug] SsrHardwareStatus missing Default trait**
- **Found during:** Task 1 compilation
- **Issue:** Tests deriving Default for structs containing SsrHardwareStatus failed
- **Fix:** Added Default impl to SsrHardwareStatus enum
- **Files modified:** src/config/constants.rs
- **Verification:** Tests compile without Default error
- **Committed in:** e1c4b77

---

**Total deviations:** 2 auto-fixed (both blocking issues)
**Impact on plan:** Core functionality now works on host target; additional work needed for full test execution

## Issues Encountered

**Embassy-time driver requirements:** The library uses embassy_time::Instant in multiple places (roaster_refactored.rs, handlers.rs, multiplexer.rs, artisan.rs). These require platform-specific implementations (critical_section, embassy_time_now). The embedded target provides these via ESP HAL, but host target needs stub implementations. Tests that depend on these functions require additional mocking work.

**Tests with partial functionality:**
- `ssr_monitor.rs` - gated to riscv32 only (uses esp_hal directly)
- `usb_cdc_tests.rs` - gated to riscv32 only (needs embassy-time)
- `mock_uart_integration.rs` - provides own mocking, works on host
- `multiplexer_tests.rs` - compiles but links fail due to embassy-time deps
- `fan_serialization.rs`, `command_idempotence.rs` - compile but link fails

## Next Phase Readiness

**What's ready:**
- Library compiles on host target - foundation is in place
- Conditional compilation infrastructure established for ESP vs host

**What remains:**
- Full test execution requires embassy-time driver stubs for host
- Some tests need custom critical_section implementations for host
- Warnings in library code (pre-existing from embedded patterns)

**Recommendation:** This is a partial win. The core blocker ("library doesn't compile on host") is resolved. The remaining work is about making all tests executable on host, which requires either:
1. Adding embassy-time driver implementations for host
2. Gating more code with cfg attributes for host targets

---
*Phase: 54-clean-up-tech-debt*
*Completed: 2026-02-18*
