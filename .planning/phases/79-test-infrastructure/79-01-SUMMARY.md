---
phase: 79-test-infrastructure
plan: 01
subsystem: testing
tags: [rust, testing, stubs]

# Dependency graph
requires:
  - phase: 78-01
    provides: SSR deduplication and stable control traits for testing
provides:
  - Library-accessible `crate::common` stubs that share the same enums, helpers, and tracking data
  - Lightweight shim in `tests/common` that continues to surface the helpers to integration suites
affects: [phase 80-01, future test suites]

# Tech tracking
tech-stack:
  added: []
  patterns: [Central crate::common stub module with minimal test shims]

key-files:
  created: [src/common/mod.rs]
  modified: [src/lib.rs, tests/common/mod.rs]

key-decisions:
  - "Guarded the new module with #[cfg(all(test, not(target_arch = \"riscv32\")))] so the std-based helpers only build for tests."

patterns-established:
  - "Expose shared test helpers from crate::common and keep integration shims limited to re-exports."

# Metrics
duration: 4m 18s
completed: 2026-02-28
---

# Phase 79 Plan 01: Test Infrastructure Summary

**Shared test stubs now live in `crate::common` so every suite reuses the same helpers without duplicating implementations.**

## Performance
- **Duration:** 4 m 18 s
- **Started:** 2026-02-28T10:49:29Z
- **Completed:** 2026-02-28T10:53:47Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments
- Migrated `StubHeater`, `StubFan`, `StubThermometer`, the call-history enums, and helper functions into `src/common/mod.rs` with `pub(crate)` visibility and test-only gating.
- Added `#[cfg(all(test, not(target_arch = \"riscv32\")))] pub mod common;` so the library exposes the stubs only when they are needed for testing.
- Replaced `tests/common/mod.rs` with a thin shim that re-exports the crate-level helpers, letting existing integration tests keep relying on `tests::common`.

## Task Commits
1. **Task 1: Create src/common stubs** - `0e9b3e8` (`feat(79-01): create shared stub module`)
2. **Task 2: Re-export stubs for tests** - `d7f0b7d` (`feat(79-01): re-export common stubs for tests`)
3. **Task 3: Run tests (adjust control imports)** - `fac126c` (`fix(79-01): simplify control imports`)

## Files Created/Modified
- `src/common/mod.rs` - Shared stub implementations scoped to the library and gated for tests.
- `src/lib.rs` - Exposes the `common` module only for non-riscv tests so the std-based helpers do not leak into production builds.
- `tests/common/mod.rs` - Shim that re-exports the new helper implementations so existing integration tests still compile.

## Decisions Made
- Guarded the new crate-level module with `#[cfg(all(test, not(target_arch = \"riscv32\")))]` so the std-based helpers only participate in test builds.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Cleaned duplicate control imports after the initial `cargo test` run**
- **Found during:** Task 3 (`cargo test --target x86_64-unknown-linux-gnu`)
- **Issue:** The module already imported the control traits and `RoasterError` for use inside the stubs, so re-exporting them again at the bottom of the file yielded duplicate-definition errors.
- **Fix:** Re-exported the traits and error at the top of the module and removed the redundant block so the names stay in scope only once while still being available to clients.
- **Files modified:** `src/common/mod.rs`
- **Verification:** `cargo test --target x86_64-unknown-linux-gnu` now proceeds past the import errors until it hits upstream `RoasterControl::new` mismatches.
- **Committed in:** `fac126c`

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Required to unblock verification; no scope creep.

## Issues Encountered
- `cargo test` defaults to the `riscv32imc-unknown-none-elf` target (per `.cargo/config.toml`), causing the std-based dependencies to fail before we can exercise the stubs. Running `cargo test --target x86_64-unknown-linux-gnu` bypasses that but still halts because several existing tests call `RoasterControl::new` with only heater/fan arguments; the host signature always expects a `SensorConversionHub`. Those tests will need to be updated before the suite can finish.

## User Setup Required
None - no external services were introduced.

## Next Phase Readiness
- Shared stubs are centralized, but `RoasterControl::new` still needs the missing `SensorConversionHub` argument in the existing tests. Updating those tests is required before the next phase can rely on a passing suite.
