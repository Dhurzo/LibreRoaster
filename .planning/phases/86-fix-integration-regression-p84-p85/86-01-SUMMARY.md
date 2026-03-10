---
phase: 86-fix-integration-regression-p84-p85
plan: 01
subsystem: testing
tags: [integration, regression, telemetry]

# Dependency graph
requires:
  - phase: 85-hardware-acceptance-thresholds-and-real-roaster-validation
    provides: [18-column STATUS expansion]
provides:
  - Restore broken integration tests and fault-injection scenarios after the 18-column STATUS expansion.
affects: [87-wire-modernization-to-quality-policy]

# Tech tracking
tech-stack:
  added: []
  patterns: [Regression snapshot verification for 18-column telemetry]

key-files:
  created: []
  modified: [tests/regression_status.rs, tests/fault_injection_scenarios.rs, src/config/constants.rs, src/output/artisan.rs, src/control/roaster_refactored.rs]

key-decisions:
  - "Update all integration test assertions to expect 18 columns in STATUS output."
  - "Cleaned up pre-existing formatting and some Tier 1 clippy issues to improve quality baseline."

# Metrics
duration: 4min
completed: 2026-03-08
---

# Phase 86 Plan 01: Fix Integration Regression Summary

**Restored broken integration tests and fault-injection scenarios after the 18-column STATUS expansion, ensuring the regression safety net is functional.**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-08T14:50:54Z
- **Completed:** 2026-03-08T14:54:40Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments
- Updated `tests/regression_status.rs` to handle 18-column telemetry layout.
- Updated `tests/fault_injection_scenarios.rs` to handle 18-column telemetry layout.
- Fixed code formatting violations via `cargo fmt`.
- Resolved several Tier 1 Clippy issues in `src/config/constants.rs`, `src/output/artisan.rs`, and `src/control/roaster_refactored.rs`.

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix tests/regression_status.rs** - `0b143b4` (fix)
2. **Task 2: Fix tests/fault_injection_scenarios.rs** - `243302b` (fix)
3. **Task 3: Final Quality Baseline Verification** - Committed in plan metadata (docs)

## Files Created/Modified
- `tests/regression_status.rs` - Updated to 18-column STATUS.
- `tests/fault_injection_scenarios.rs` - Updated to 18-column STATUS.
- `src/config/constants.rs` - Fixed clippy issues (derivable_impls, new_without_default).
- `src/output/artisan.rs` - Fixed clippy issues and cleaned up unused code.
- `src/control/roaster_refactored.rs` - Fixed clippy issues and cleaned up dead code.

## Decisions Made
- **Maintain quality baseline:** Decided to fix pre-existing formatting and Clippy issues encountered during verification to ensure a cleaner baseline for future phases.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed formatting violations**
- **Found during:** Task 3 (Quality baseline verification)
- **Issue:** `cargo fmt --check` failed due to pre-existing violations.
- **Fix:** Ran `cargo fmt`.
- **Files modified:** multiple
- **Verification:** `cargo fmt --check` passes in quality script.
- **Committed in:** docs(86-01) metadata commit

**2. [Rule 2 - Missing Critical] Resolved Tier 1 Clippy findings**
- **Found during:** Task 3 (Quality baseline verification)
- **Issue:** Several Tier 1 (Blocking) findings prevented quality script from passing.
- **Fix:** Fixed `derivable_impls`, `new_without_default`, and `unused_variables`.
- **Files modified:** src/config/constants.rs, src/output/artisan.rs, src/control/roaster_refactored.rs
- **Verification:** Quality script shows fewer Tier 1 findings.
- **Committed in:** docs(86-01) metadata commit

## Issues Encountered
- **Doc-test failures:** Pre-existing legacy doc-tests in `src/common/mod.rs` and other files are failing. These are outside the scope of this plan and do not affect the integration tests being fixed.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Integration tests are green with 18-column telemetry.
- Quality baseline improved but still has pre-existing Tier 1 issues (dead code, type complexity) and failing doc-tests.
- Ready for Phase 87: Wire Modernization to Quality Policy.

---
*Phase: 86-fix-integration-regression-p84-p85*
*Completed: 2026-03-08*
