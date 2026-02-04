---
phase: 10-mock-uart-integration
plan: 01
subsystem: testing
tags: [uart, artisan, integration-tests, host, critical-section, embassy-time]

# Dependency graph
requires:
  - phase: 08-02
    provides: Command bounds and START/STOP safety behavior
  - phase: 09-02
    provides: ERR schema and deterministic READ formatter
provides:
  - Mock UART integration tests for READ/START/STOP/OT1/IO3 success and error flows
  - Host-friendly command pipeline harness for UART parsing and output collection
affects: [hardware bring-up, protocol regression]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Host test shims for critical-section and embassy-time"
    - "Host stubs for embedded-only hardware modules"

key-files:
  created:
    - tests/mock_uart_integration.rs
    - src/hardware/fan_host.rs
    - src/hardware/uart/driver_host.rs
  modified:
    - Cargo.toml
    - build.rs
    - src/application/mod.rs
    - src/hardware/mod.rs
    - src/hardware/uart/mod.rs
    - src/hardware/uart/tasks.rs
    - src/input/mod.rs
    - src/main.rs

key-decisions:
  - "Gate embedded binary behind an `embedded` feature to enable host tests"
  - "Provide host stubs/shims for embedded-only modules used by integration tests"

patterns-established:
  - "Integration tests use process_command_data + ServiceContainer channels for end-to-end UART flows"

# Metrics
duration: 11 min
completed: 2026-02-04
---

# Phase 10 Plan 01: Mock UART Integration Summary

**Host-friendly mock UART integration tests exercising Artisan command parsing, handler pipeline, and ERR/READ responses**

## Performance

- **Duration:** 11 min
- **Started:** 2026-02-04T16:31:38Z
- **Completed:** 2026-02-04T16:43:05Z
- **Tasks:** 3
- **Files modified:** 11

## Accomplishments
- Built a reusable mock UART harness that drives real parsing and handler pipelines
- Added success-path tests for READ/START/OT1/IO3/STOP with state and output assertions
- Added error-path tests that verify ERR tokens and no side effects

## Task Commits

Each task was committed atomically:

1. **Task 1: Add mock UART integration harness helpers** - `f2d9202` (test)
2. **Task 2: Add success-path UART flow tests** - `7b9c0b0` (test)
3. **Task 3: Add error-path UART flow tests** - `211d367` (test)

**Plan metadata:** pending

## Files Created/Modified
- `tests/mock_uart_integration.rs` - Mock UART integration harness and end-to-end tests
- `src/hardware/fan_host.rs` - Host stub fan controller for tests
- `src/hardware/uart/driver_host.rs` - Host stub UART driver for tests
- `Cargo.toml` - Host test dependencies and embedded feature gate
- `build.rs` - Skip embedded linker args on non-riscv targets
- `src/application/mod.rs` - Gate embedded-only modules for host builds
- `src/hardware/mod.rs` - Swap hardware modules for host stubs
- `src/hardware/uart/mod.rs` - Conditional driver selection for host/embedded
- `src/hardware/uart/tasks.rs` - Test-only visibility for process_command_data
- `src/input/mod.rs` - Gate UART task startup to embedded targets
- `src/main.rs` - Host-safe binary compilation

## Decisions Made
- Gate embedded binary behind an `embedded` feature to enable host-target test runs without no-std linker failures.
- Provide host stubs/shims for embedded-only modules so UART integration tests compile and link on x86_64.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Host builds failed due to embedded-only modules and linker scripts**
- **Found during:** Task 1 (test harness verification)
- **Issue:** Host-target tests could not compile/link because esp-hal modules, embedded linker args, and no-std binary were always enabled
- **Fix:** Added host stub hardware modules, gated embedded-only modules, skipped embedded linker args on non-riscv, and gated the binary behind an `embedded` feature
- **Files modified:** Cargo.toml, build.rs, src/application/mod.rs, src/hardware/mod.rs, src/hardware/uart/mod.rs, src/main.rs, src/hardware/fan_host.rs, src/hardware/uart/driver_host.rs
- **Verification:** `cargo test --test mock_uart_integration --features test --target x86_64-unknown-linux-gnu`
- **Committed in:** f2d9202

**2. [Rule 3 - Blocking] Host linker missing critical-section and embassy-time symbols**
- **Found during:** Task 2 (success-path verification)
- **Issue:** Tests failed to link due to missing critical-section and embassy-time driver symbols
- **Fix:** Added test-only critical-section and embassy-time shims in the integration test file
- **Files modified:** tests/mock_uart_integration.rs
- **Verification:** `cargo test --test mock_uart_integration --features test --target x86_64-unknown-linux-gnu`
- **Committed in:** 7b9c0b0

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both fixes were required to run host-target integration tests; no scope creep beyond test enablement.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
Phase 10 complete, ready for transition.

---
*Phase: 10-mock-uart-integration*
*Completed: 2026-02-04*
