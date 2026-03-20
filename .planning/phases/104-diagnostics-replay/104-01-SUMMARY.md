---
phase: 104-diagnostics-replay
plan: 01
subsystem: diagnostics
tags: [python, automation, traceability, replay]

# Dependency graph
requires:
  - phase: 103-diagnostics-artifacts
    provides: packaged safe-shutdown artifact with log, matrix, and metadata
provides:
  - guard-metadata replay CLI that regenerates the traceability CSV and emits JSON for auditors
affects:
  - diagnostics automation
  - traceability verification

# Tech tracking
tech-stack:
  added:
    - python automation script for artifact replay
  patterns:
    - metadata-first artifact replay validated before auditor signoff
    - JSON audit reports that capture guard metadata match/mismatch details

key-files:
  created:
    - scripts/replay_safe_shutdown.py
  modified:
    - scripts/test_traceability_matrix.py
    - internalDoc/INSTRUMENTATION_README.MD

key-decisions:
  - "Replay automation must cross-check metadata.json guard fields (TraceId/watchdog_failure/error_category/error_source) whenever the artifact is reprocessed."
  - "Expose replay results via `--report` so CI/audit automation can assert metadata fidelity and persist evidence."

patterns-established:
  - "Artifact replay flow that rebuilds traceability CSVs and enforces guard metadata fidelity before auditors consume the bundle."
  - "JSON audit reports capturing metadata match/mismatch details for downstream automation."

# Metrics
duration: 3m 27s
completed: 2026-03-20
---

# Phase 104 Plan 01: Safe-Shutdown Artifact Replay Summary

**Replay automation decompresses the safe-shutdown bundle, regenerates the matrix, validates the guard metadata, and emits JSON evidence so auditors can rerun the failure path without hardware.**

## Performance

- **Duration:** 3m 27s
- **Started:** 2026-03-20T22:14:32Z
- **Completed:** 2026-03-20T22:17:59Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments

- Added `scripts/replay_safe_shutdown.py` to unzip the replay artifact, rebuild the traceability matrix, compare guard metadata (TraceId/watchdog_failure/error_category/error_source), and print both the metadata summary and the regenerated CSV path.
- Extended `scripts/test_traceability_matrix.py` so the regression suite builds its own safe-shutdown artifact, runs the replay CLI with `--report`, and asserts the JSON payload matches the expected guard diagnostics.
- Updated `internalDoc/INSTRUMENTATION_README.MD` with a Safe-Shutdown Artifact Replay subsection that documents the CLI invocation, the JSON report format, and how audit/CI automation can leverage the replay automation.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add artifact replay automation** - `a6948cd` (feat)
2. **Task 2: Cover replay automation with regression test** - `18bfc25` (test)
3. **Task 3: Document replay automation workflow** - `8cf10c8` (docs)

**Plan metadata:** docs(104-01): complete artifact replay plan

## Files Created/Modified

- `scripts/replay_safe_shutdown.py` - CLI that unpacks the safe-shutdown bundle, regenerates the matrix, compares guard metadata, and supports JSON reporting for automation.
- `scripts/test_traceability_matrix.py` - Regression coverage that builds the artifact, invokes the replay CLI with `--report`, and asserts the guard metadata matches the canonical failure tokens.
- `internalDoc/INSTRUMENTATION_README.MD` - Enables auditors to run the replay CLI, understand the JSON report structure, and integrate the metadata check into CI/audit automation.

## Decisions Made

- Replay automation must cross-check metadata.json guard fields (TraceId/watchdog_failure/error_category/error_source) whenever the artifact is reprocessed.
- Expose replay results via `--report` so CI/audit automation can assert metadata fidelity and persist evidence.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- The replay CLI + regression coverage gives future diagnostics/audit phases a reproducible guard metadata validation step to plug into their automation.
- No blockers remain; artifact replay and documentation are ready for subsequent verification or audit work.
