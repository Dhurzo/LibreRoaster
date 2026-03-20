---
phase: 103-diagnostics-artifacts
plan: 01
subsystem: diagnostics
tags: [python, zip, traceability, audit]

# Dependency graph
requires:
  - phase: 102-diagnostics-verification
    provides: Safe-shutdown guard TRACE events with AppError metadata and the sample log fixture
provides:
  - CLI that bundles a TRACE log, traceability matrix CSV, metadata, and README into a reproducible safe-shutdown artifact
  - Regression guard that exercises the artifact CLI and validates the packaged metadata fields
  - Instrumentation guidance describing how to build, inspect, and replay the artifact for auditors
affects: [audit-readiness, diagnostics-review]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Safe-shutdown artifacts bundle the original log, parsed matrix, guard metadata, and a README summary so auditors receive one complete bundle
    - Regression coverage runs the CLI inside the existing suite to ensure artifact reproducibility whenever the sample log changes

key-files:
  created:
    - scripts/collect_safe_shutdown.py
  modified:
    - scripts/test_traceability_matrix.py
    - internalDoc/INSTRUMENTATION_README.MD

key-decisions:
  - None - followed plan as specified

patterns-established:
  - Documented a CLI-driven artifact replay workflow so guard failures stay reproducible without hardware
  - Metadata-first packaging (TraceId + watchdog reason + AppError category/source) proves guard diagnostics for auditors

# Metrics
duration: 0 min
completed: 2026-03-20
---

# Phase 103 Plan 01 Summary

**Safe-shutdown guard failures now ship as reproducible trace artifacts with regression coverage and doced replay steps.**

## Performance

- **Duration:** 0 min (22 s)
- **Started:** 2026-03-20T21:30:25Z
- **Completed:** 2026-03-20T21:30:48Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments

- Created `scripts/collect_safe_shutdown.py` so auditors can bundle a TRACE log, its `traceability.csv`, guard metadata JSON, and a README snapshot into a single zip artifact with artifact-name and force helpers.
- Added `TestTraceabilityMatrix.test_collect_safe_shutdown_artifact` so the regression suite runs the CLI, inspects the zip, and validates the guard metadata matches `watchdog_failure=init_error_failure` plus the AppError fields.
- Extended `internalDoc/INSTRUMENTATION_README.MD` with a Safe-Shutdown Replay Artifact section that spells out the CLI call, artifact contents, metadata expectations, and how to unzip and rerun `scripts/traceability_matrix.py` on the packaged log.

## Task Commits

1. **Task 1: Bundle the safe-shutdown artifact** - `f13aeaa` (feat)
2. **Task 2: Cover artifact packaging with a regression test** - `9ac80e9` (test)
3. **Task 3: Document how to replay the safe-shutdown artifact** - `5f85e94` (docs)

**Plan metadata:** docs commit capturing this summary, STATE, and ROADMAP

## Files Created/Modified

- `scripts/collect_safe_shutdown.py` - CLI that parses a TRACE log, produces the CSV matrix and guard metadata, and zips the result with a README summary.
- `scripts/test_traceability_matrix.py` - Regression test that runs the CLI, inspects the artifact zip, and asserts the guard metadata fields match the safe-shutdown row.
- `internalDoc/INSTRUMENTATION_README.MD` - Guide section on running the CLI, describing the artifact contents, and replaying the matrix with explicit metadata values.

## Decisions Made

- None - followed plan as specified

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Safe-shutdown guard trace artifacts are reproducible, regression-protected, and documented so auditors and diagnostics reviewers can replay the failure path without hardware; the next phase can focus on compliance triage or broader instrumentation topics.
