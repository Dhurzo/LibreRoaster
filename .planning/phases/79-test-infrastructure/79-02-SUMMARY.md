---
phase: 79-test-infrastructure
plan: 02
subsystem: testing
tags: [rust, testing, stubs, sensors]

# Dependency graph
requires:
  - phase: 79-01
    provides: Shared stub types and helper imports in `tests/common` so every suite has the same implementations.
provides:
  - Integration suites now import the shared shim and use a single `build_test_control` helper that wires `SensorConversionHub` into `RoasterControl`.
  - The stub call history storage is guarded with `critical_section::Mutex` so async sensor reads no longer panic.
  - `cargo test` defaults to the x86_64 host target with a `flash-riscv` alias for embedded builds.

# Tech tracking
tech-stack:
  added: [critical-section synchronization for shared stubs]
  patterns: [central `tests_common` shim re-exporting `libreroaster::common`, `build_test_control` wiring SensorConversionHub]

key-files:
  created: [tests/common/mod.rs, tests/command_idempotence.rs, tests/command_multiplexer_concurrency.rs, tests/concurrent_sensor_test.rs, tests/mock_uart_integration.rs, tests/fan_serialization.rs, src/common/mod.rs, src/lib.rs, .cargo/config.toml]
  modified: []

key-decisions:
  - "Expose the shared stub module for every host build so integration suites can import it without redefining helpers."
  - "Wrap stub call history with `critical_section::Mutex` because async sensor reads borrow the stubs concurrently."

patterns-established:
  - "Every integration suite now loads `tests_common` and defers to `StubHeater`/`StubFan`/`StubThermometer` instead of defining their own copies."
  - "`build_test_control` centralizes RoasterControl construction so SensorConversionHub is supplied uniformly."

# Metrics
completed: 2026-02-28
---

**Shared stubs, SensorConversionHub wiring, and host-friendly cargo config for integration tests**

## Performance

- **Duration:** 27 min
- **Started:** 2026-02-28T13:49:13Z
- **Completed:** 2026-02-28T14:16:39Z
- **Tasks:** 3
- **Files modified:** 9

## Accomplishments

- Extended the `tests/common` shim with `SensorConversionHub` exports and a `build_test_control` helper so every suite can consume the shared stubs.
- Swapped each integration test to import `tests_common`/`build_test_control`, removed their local stub defs, and guarded the shared stubs with `critical_section::Mutex` for concurrency safety.
- Pointed `.cargo/config.toml` at the x86_64 host target by default and documented a `flash-riscv` alias so embedded builds remain accessible.

## Task Commits

1. **Task 1: Extend shim with SensorConversionHub builder** - `7b119c5` (`feat(79-02): add sensor hub builder to shim`)
2. **Task 2: Switch suites to shared stubs and mutex guards** - `47dc117` (`feat(79-02): map suites to shared stubs`)
3. **Task 3: Default cargo test to host target** - `f9b7049` (`fix(79-02): default cargo test to host target`)

**Plan metadata:** `f9b7049`

## Files Created/Modified

- `tests/common/mod.rs` - Shim now re-exports `SensorConversionHub`, adds `build_test_control`, and wraps stub call history in `critical_section::Mutex`.
- `tests/*` (five suites) - Removed local stub definitions, added `tests_common` imports, and point all `build_control()` helpers at the shared helper.
- `src/common/mod.rs` & `src/lib.rs` - Made the stub module host-only while exposing it for testing, and kept `std`/`alloc` imports host-friendly.
- `.cargo/config.toml` - Removed the RISC-V default target and documented a `flash-riscv` alias so `cargo test` runs on x86_64.

## Decisions Made

- Host builds now compile the `common` module under `#[cfg(not(target_arch = "riscv32"))]` so `tests_common` can depend on `libreroaster::common` even when the library is built as a dependency.
- `build_test_control` injects `SensorConversionHub::new()` so every integration suite satisfies the RoasterControl signature without sprawling logic.
- Stub call tracking moved behind `critical_section::Mutex` so multi-threaded tests no longer panic on concurrent access.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Guarded stub call history for concurrent reads**

- **Found during:** Task 2 (Switch the integration suites to the shared stubs and SensorConversionHub builder)
- **Issue:** The shared stubs used `RefCell` for call history, so async sensor reads and multiplexed tests panicked with "RefCell already borrowed".
- **Fix:** Wrapped each stub’s state in `critical_section::Mutex` so each call to the helper happens inside a critical section, preventing overlapping borrows.
- **Files modified:** `src/common/mod.rs`
- **Verification:** `cargo test` now gets past the concurrency suite that previously failed.

**Total deviations:** 1 auto-fixed (Rule 1 - Bug)
**Impact on plan:** Necessary to avoid concurrency panics; no scope creep.

## Issues Encountered

- `cargo test` fails in `tests/mock_uart.rs` (TEST-MOCK-02, -05, -06) because the mock UART’s buffer semantics/streaming expectations no longer align with host-target behavior (missing final byte, `has_data` staying true, heapless vector overflow). These tests must be rewritten or the driver adjusted before the suite can greenlight.

## User Setup Required

None — no external services were introduced.

## Next Phase Readiness

- Shared stubs and the shim helper are now in place, and `build_test_control` injects `SensorConversionHub` everywhere.
- However, the mock UART suite on the host target still fails, so resolve the buffer/streaming expectations before moving on to Phase 80.

---
*Phase: 79-test-infrastructure*
*Completed: 2026-02-28*
