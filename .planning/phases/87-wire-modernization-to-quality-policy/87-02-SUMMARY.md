---
phase: 87-wire-modernization-to-quality-policy
plan: 02
subsystem: infra
tags: [shell, quality-policy, automation, cargo]

# Dependency graph
requires:
  - phase: 87-01
    provides: quality-baseline.sh script and .cargo/config.toml with clippy deny warnings
provides:
  - run-modernization.sh now enforces quality baseline before fixes
  - run-regression-checks.sh now enforces quality baseline before tests
affects: [future modernization runs, regression test runs]

# Tech tracking
tech-stack:
  added: []
  patterns: [quality policy ratcheting via shell scripts]

key-files:
  modified: [scripts/run-modernization.sh, scripts/run-regression-checks.sh]

key-decisions:
  - "Wired quality-baseline.sh into both run-modernization.sh and run-regression-checks.sh for policy enforcement"

patterns-established:
  - "Quality gates in automation scripts prevent policy bypass (QG-01)"

# Metrics
duration: 2 min
completed: 2026-03-09
---

# Phase 87 Plan 2: Wire Quality Policy into Automation Scripts Summary

**Wired quality-baseline.sh into run-modernization.sh and run-regression-checks.sh to enforce policy ratcheting**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-09T06:06:55Z
- **Completed:** 2026-03-09T06:08:52Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Integrated quality-baseline.sh into run-modernization.sh (calls after cargo fmt, before cargo fix)
- Integrated quality-baseline.sh into run-regression-checks.sh (calls at beginning, before regression tests)
- Both scripts now exit with non-zero status if quality baseline checks fail

## Task Commits

Each task was committed atomically:

1. **Task 1: Update run-modernization.sh to include quality baseline check** - `7886302` (feat)
2. **Task 2: Update run-regression-checks.sh to include quality baseline check** - `b9eb58c` (feat)

**Plan metadata:** (to be committed)

## Files Created/Modified
- `scripts/run-modernization.sh` - Added call to quality-baseline.sh after fmt step
- `scripts/run-regression-checks.sh` - Added call to quality-baseline.sh at start

## Decisions Made
None - plan executed exactly as specified.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None - Both scripts correctly run quality baseline checks and fail as expected when pre-existing clippy issues are detected. This is the intended behavior - the quality policy is now being enforced.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 87 complete - quality policy is now wired into automation scripts
- Policy ratcheting (Tier 1/2) will be enforced during any automated runs

---
*Phase: 87-wire-modernization-to-quality-policy*
*Completed: 2026-03-09*
