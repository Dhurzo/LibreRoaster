---
phase: 81-quality-baseline-and-ratcheting-policy
plan: 02
subsystem: infra
tags: [rust, cargo, clippy, quality-gate, policy, bash, python]

# Dependency graph
requires:
  - phase: 81-01
    provides: Quality policy artifacts (baseline-policy.toml, tier mapping, ratchet governance)
provides:
  - scripts/quality-baseline.sh - Single orchestrator command for deterministic baseline runs
  - scripts/quality_baseline.py - Policy-aware diagnostics evaluator with tier classification
  - Reproducible pass/fail verdict with QG-POLICY@{version} references
affects: [82-dead-code-cleanup, 83-rust-modernization, 84-solid-seam-hardening]

# Tech tracking
tech-stack:
  added: [python3-stdlib, bash]
  patterns: [tiered-policy-enforcement, gate-orchestration, compact-summary]

key-files:
  created: [scripts/quality-baseline.sh, scripts/quality_baseline.py]

key-decisions:
  - "Fixed gate order (fmt->clippy->test) with no early termination"
  - "Tier-based blocking: Tier 1 blocks, T2/T3 informational for gradual ratcheting"
  - "Policy version from TOML for deterministic reproducibility"
  - "JSON diagnostics from clippy for module+rule extraction"

patterns-established:
  - "Single baseline command entrypoint with fixed gate sequencing"
  - "Policy-aware compact output with module+rule+tier+policy_id"
  - "Fixture mode for intentional failure testing in later plans"

# Metrics
duration: 5min
completed: 2026-03-07
---

# Phase 81 Plan 2: Baseline Runner Implementation Summary

**Deterministic baseline runner with policy-aware compact output and tier-based enforcement**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-07T11:49:05Z
- **Completed:** 2026-03-07T11:54:05Z
- **Tasks:** 3
- **Files modified:** 2 (created)

## Accomplishments

- Created `scripts/quality-baseline.sh` - single orchestrator command executing fmt→clippy→test in fixed order without short-circuiting
- Created `scripts/quality_baseline.py` - policy-aware evaluator that parses clippy JSON, classifies findings by module tier, and emits compact actionable output
- Implemented tier-based blocking: Tier 1 (safety/control/protocol) blocks baseline, T2/T3 are informational
- Added reproducibility messaging: "same input, same verdict - QG-POLICY v{version}"
- Implemented fixture mode (`--from-json`) for intentional failure testing in plan 81-03

## Task Commits

1. **Task 1-3: Baseline runner and evaluator** - `a359b53` (feat)

**Plan metadata:** (will be created after summary)

## Files Created/Modified

- `scripts/quality-baseline.sh` - Shell entrypoint for deterministic gate execution
- `scripts/quality_baseline.py` - Policy-aware diagnostics evaluator with tier classification

## Decisions Made

- Fixed gate order: fmt → clippy → test runs all gates even if earlier ones fail
- Tier-based enforcement: Only Tier 1 findings block; T2/T3 reported as informational
- Policy version loaded from baseline-policy.toml for deterministic reproducibility
- JSON diagnostics parsed for accurate module+rule extraction

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed cargo clippy --message-format flag placement**

- **Found during:** Task 1 (baseline runner implementation)
- **Issue:** `--message-format=json` was passed to clippy, not cargo, causing compilation error
- **Fix:** Changed to `cargo clippy --message-format=json --all-targets -- [clippy flags]`
- **Files modified:** scripts/quality-baseline.sh
- **Verification:** Baseline runner now executes clippy gate successfully
- **Committed in:** a359b53 (Task 1-3 commit)

**2. [Rule 3 - Blocking] Fixed policy_version variable scope**

- **Found during:** Task 1 (baseline runner implementation)
- **Issue:** policy_version was defined inside run_evaluator function but referenced in main
- **Fix:** Added POLICY_VERSION=$(get_policy_version) at top of main()
- **Files modified:** scripts/quality-baseline.sh
- **Verification:** Final verdict now shows version correctly
- **Committed in:** a359b53 (Task 1-3 commit)

---

**Total deviations:** 2 auto-fixed (both blocking issues)
**Impact on plan:** Both fixes essential for baseline runner to function correctly. No scope creep.

## Issues Encountered

- Pre-existing clippy lint configuration issues: Cargo.toml contains renamed/removed lints (clippy::drop_copy → dropping_copy, etc.) that cause "unknown lint" warnings
- Pre-existing test failure in mock_uart_integration when run with all tests (passes individually)

These are baseline code quality issues, not issues with the baseline runner implementation.

## Next Phase Readiness

- Phase 81-02 complete - baseline runner implementation done
- Ready for 81-03: Intentional failure drills and reproducibility validation
- The baseline runner correctly:
  - Executes all gates in order (fmt→clippy→test)
  - Does not short-circuit on first failure
  - Applies tier-based blocking policy
  - Emits compact actionable output with policy references
  - Shows reproducible verdict messaging

---
*Phase: 81-quality-baseline-and-ratcheting-policy*
*Completed: 2026-03-07*
