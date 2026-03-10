---
phase: 81-quality-baseline-and-ratcheting-policy
plan: 03
subsystem: testing
tags: [quality-gate, clippy, fixtures, reproducibility]

# Dependency graph
requires:
  - phase: 81-01
    provides: Policy definition (QG-POLICY v1.0.0, tier mapping)
  - phase: 81-02
    provides: Baseline runner implementation
provides:
  - Deterministic failure fixtures for Tier 1 and mixed-tier scenarios
  - Selfcheck script for policy output validation
  - Documented operator drill for fail-and-rerun reproducibility
affects: [82-dead-code-cleanup, 83-rust-modernization]

# Tech tracking
tech-stack:
  added: []
  patterns: [fixture-driven testing, deterministic verification, tiered policy enforcement]

key-files:
  created:
    - tests/quality/fixtures/clippy-tier1-fail.jsonl
    - tests/quality/fixtures/clippy-mixed-failures.jsonl
    - scripts/quality-baseline-selfcheck.sh
    - .planning/quality/failure-drill.md

key-decisions:
  - "Selfcheck uses fixture mode to avoid source code modifications during verification"
  - "Reproducibility confirmed by running same fixture twice and comparing exit codes and verdict text"

patterns-established:
  - "Fixture-driven intentional failure testing for policy validation"
  - "Documented operator drills for baseline gate behavior verification"

# Metrics
duration: 2min
completed: 2026-03-07
---

# Phase 81 Plan 03: Intentional Failure Drills Summary

**Deterministic failure fixtures and selfcheck script for quality baseline policy validation**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-07T11:59:00Z
- **Completed:** 2026-03-07T12:00:51Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Created deterministic fixture files for Tier 1 blocking and mixed-tier failure scenarios
- Implemented selfcheck script validating intentional failure visibility, all-findings aggregation, and reproducibility
- Documented fail-and-rerun operator workflow with expected outputs and policy version awareness

## Task Commits

Each task was committed atomically:

1. **Task 1: Create deterministic failure fixtures** - `2008593` (test)
2. **Task 2: Automate intentional-failure and rerun checks** - `8fc565b` (feat)
3. **Task 3: Document fail-and-rerun operator drill** - `d446b84` (docs)

**Plan metadata:** (to be added by final commit)

## Files Created/Modified

- `tests/quality/fixtures/clippy-tier1-fail.jsonl` - Blocking diagnostic fixture for Tier 1 module
- `tests/quality/fixtures/clippy-mixed-failures.jsonl` - Mixed Tier 2/3 informational findings
- `scripts/quality-baseline-selfcheck.sh` - Selfcheck automation script
- `.planning/quality/failure-drill.md` - Documented operator workflow

## Decisions Made

- Selfcheck uses fixture mode (`--from-json`) to validate policy evaluation without modifying source code
- Reproducibility verified by comparing exit codes and verdict text between identical runs
- Output format includes policy reference (`QG-POLICY@version`), module path, tier marker, and lint rule for actionable debugging

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None - all verification criteria passed:
- Selfcheck script passes all 3 tests (Tier 1 blocking, mixed tiers, reproducibility)
- Standard baseline runs correctly and shows actual codebase findings
- Fixtures validated as proper JSON

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 81 complete! The quality baseline policy infrastructure is now fully operational:
- Policy definition (81-01)
- Baseline runner (81-02)
- Selfcheck and reproducibility drills (81-03)

Ready for Phase 82: Dead Code and Dependency Cleanup

---
*Phase: 81-quality-baseline-and-ratcheting-policy*
*Completed: 2026-03-07*
