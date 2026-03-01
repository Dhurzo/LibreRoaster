---
phase: 79-test-infrastructure
plan: 03
subsystem: testing
tags: [rust, testing, stubs, alloc, no_std]

# Dependency graph
requires:
  - phase: 79-02
    provides: Shared shim exports and host-target defaults that let integration suites import a single helper surface.
provides:
  - Core/alloc-based stub helpers that compile under the crate's default `#![no_std]` profile.
  - Public enums, structs, and helper functions exported through `libreroaster::common` so `tests_common` can re-export a single API.

# Tech tracking
tech-stack:
  added: []
  patterns: [core/alloc stub helpers exposed via the crate root for no_std hosts]

key-files:
  created: []
  modified: [src/common/mod.rs]

key-decisions:
  - "Expose the stub enums, structs, and helpers via `pub` so `tests_common` can re-export them without privacy errors."
  - "Shift the stub module to `core`/`alloc` primitives so it builds in the library's default `#![no_std]` configuration."

patterns-established:
  - "Test stubs in `libreroaster::common` now rely on minimal `alloc` primitives, keeping them usable for host builds without `std`."
  - "Integration shims can depend on a single set of stub helpers exported from the crate rather than defining per-suite copies."

# Metrics
completed: 2026-02-28
---

# Phase 79 Plan 03: Test Infrastructure Summary

**Core/alloc stub helpers are now public so `tests_common` can re-export them in host builds.**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-28T14:43:42Z
- **Completed:** 2026-02-28T14:47:59Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments

- Rewrote the shared stub module documentation and imports to describe and use `core::cell::RefCell`, `alloc::vec::Vec`, and `alloc::string::String`, ensuring it compiles without `std` under the host `#![no_std]` profile.
- Exposed `HeaterCall`, `FanCall`, `ThermometerCall`, `StubHeater`, `StubFan`, `StubThermometer`, `reset_channels`, and `collect_output` as `pub` items so the shim can re-export a consistent helper API.
- This unlocks `tests_common` to rely on `libreroaster::common` for every stub helper, keeping integration suites aligned with the library's implementation.

## Task Commits

1. **Task 1: Replace std dependencies with core/alloc helpers** - `1f0a350` (`fix(79-03): make common stubs alloc friendly`)
2. **Task 2: Expose stub enums, structs, and helpers publicly** - `27f118b` (`feat(79-03): expose common stub helpers`)

**Plan metadata:** docs(79-03): complete stub helper exposure plan

## Files Created/Modified

- `src/common/mod.rs` - Reworked the documentation/imports so the module uses `core`/`alloc`, and published every stub enum, struct, and helper through the crate root.

## Decisions Made

- Expose the stub helpers via `pub` so `tests_common` can re-export them without privacy errors.
- Shift the stub module to rely on `core`/`alloc` primitives in order to stay compatible with the library's default `#![no_std]` configuration.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- `cargo test` still fails because the host `tests/mock_uart.rs` suite’s expectations (e.g., `test_mock_uart_read_bytes`, `test_mock_uart_streaming`, `test_multiple_commands`) do not yet match the emulator’s buffer semantics, so these tests panic or overflow even though the stub helper re-export compiles cleanly.

## User Setup Required

None — no external services were introduced.

## Next Phase Readiness

- Shared stub helpers are now available through `libreroaster::common`, so integration suites can build on the shim without touching private APIs.
- Host mock UART expectations still fail, so reworking the buffer/streaming assertions is the remaining blocker before Phase 80 can verify the shim end-to-end.

---
*Phase: 79-test-infrastructure*
*Completed: 2026-02-28*
