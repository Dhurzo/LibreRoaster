---
phase: 82-dead-code-and-dependency-cleanup
plan: 82-02
subsystem: quality
tags: [bash, cargo, dead_code, quality-baseline, documentation]

# Dependency graph
requires:
  - phase: 81-quality-baseline-and-ratcheting-policy
    provides: deterministic gating baseline with documented policy ratchets
provides:
  - scripts/dead-code-removal.sh that snapshots candidate modules, runs cargo test + the quality baseline, and writes a per-batch summary
  - quality/dead-code/removal-guidelines.md linking DC-01 inventory entries to batch names, gate logs, and failure triage steps
affects: [phase-82-dependency-audit, phase-83-rust-modernization]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Evidence-first batches that snapshot pre/post module lists and record gate outputs before merging any removal candidate

key-files:
  created:
    - quality/dead-code/removal-guidelines.md
  modified:
    - scripts/dead-code-removal.sh

key-decisions:
  - "None - followed plan as specified."

patterns-established:
  - "Batch runners now record both pre/post module snapshots plus gate summaries under quality/dead-code/batches so reviewers can diff inventory claims against the actual gate output."
  - "The DC-02 workflow ties each `BATCH_NAME` to the module list, cargo test log, and quality baseline log so evidence can be attached to the PR or dead-code audit bundle."

# Metrics
duration: 3m 28s
completed: 2026-03-07
---

# Phase 82 Plan 82-02 Summary

**Dead-code batch runner and guidelines that prove each removal passes cargo test plus the quality baseline and ties it back to the DC-01 inventory.**

## Performance

- **Duration:** 3 min 28 sec
- **Started:** 2026-03-07T13:02:38Z
- **Completed:** 2026-03-07T13:06:06Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added `scripts/dead-code-removal.sh` to copy the requested module list into `quality/dead-code/batches/<name>.md`, run `cargo test --locked --lib --tests --no-fail-fast`, call `scripts/quality-baseline.sh`, and append a gate summary that includes the test and baseline log paths plus the exit codes.
- Authored `quality/dead-code/removal-guidelines.md` so batch owners start from the DC-01 inventory, document the collection of module/ID evidence, run the script, link the batch summary to the gated baseline report, and know how to treat failures with log capture/reversion or risk reclassification.
- Explicitly call out failure handling so a failed batch leaves the batch file + logs for triage and auditors, preventing merges until the gate evidence is green.

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement removal batch runner** - `b51496b` (feat)
2. **Task 2: Document batch removal workflow** - `d71c626` (docs)

**Plan metadata:** docs(82-02): complete removal batch workflow plan

## Files Created/Modified

- `scripts/dead-code-removal.sh` - runs the removal batch, snapshots modules lists, gates on `cargo test` and `scripts/quality-baseline.sh`, and writes a gate summary for reviewers.
- `quality/dead-code/removal-guidelines.md` - explains how to pick modules from the DC-01 inventory, run the script, review the batch file, and handle failed batches while linking the baseline report.

## Decisions Made

- None - followed plan as specified.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- None.

## User Setup Required

None - no external service configuration is required for this workflow.

## Next Phase Readiness

- Dead-code batch tooling + documentation ready for **82-03 (dependency audits + allowlist)** so the dependency work can consume the removal policy.
- No additional blockers; rerun the batch script after each inventory refresh before attempting a removal to keep DC-02 evidence tied to the latest data.
