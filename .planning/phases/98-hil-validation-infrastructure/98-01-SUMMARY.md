---
phase: 98-hil-validation-infrastructure
plan: 01
subsystem: infra
tags: [hil, hardware, manifest, audits]

# Dependency graph
requires:
  - phase: 96-error-architecture-implementation
    provides: "Cross-module error taxonomy that HIL scenarios rely on for guard/watchdog classification."
  - phase: 97-traceability-matrix-tooling
    provides: "Trace stream instrumentation that makes golden STATUS telemetry auditable."
provides:
  - "Scenario matrix that links every STATUS column to the golden CSV metrics, qualifying criteria, and manifest entry."
  - "Machine-readable scenario manifest enumerating commands, expected columns, golden_output paths, and retention metadata."
  - "Methodology describing how to drive the manifest, where to store telemetry/artifacts, and how long auditors must keep them."
  - "phase 98 follow-up plans (98-02 manifest-aware validation_runner, 98-03 playbook/readme updates)."

# Tech tracking
tech-stack:
  added: [scenario manifest JSON schema, golden-run retention guidance]
  patterns: [manifest-to-matrix linkage, telemetry retention checklist]

key-files:
  created: [tests/hardware/scenario_manifest.json]
  modified: [tests/hardware/SCENARIO_MATRIX.md, tests/hardware/METHODOLOGY.md]

key-decisions:
  - "None - plan executed as specified."

patterns-established:
  - "Manifest-driven scenario definitions that connect SCENARIO_MATRIX.md IDs to automation metadata."
  - "Telemetry and retention guidance that keeps goldens/runs directories consistent for auditors."

# Metrics
duration: 0.2 min
completed: 2026-03-20
---

# Phase 98 Plan 01: HIL Scenario Contract Summary

**Golden-run contract linking the scenario matrix, JSON manifest, and retention guidance so automation and auditors share a single source of truth.**

## Performance

- **Duration:** 13 sec
- **Started:** 2026-03-20T13:55:35Z
- **Completed:** 2026-03-20T13:55:47Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments

- Expanded `SCENARIO_MATRIX.md` with STATUS column mapping, manifest cross-links, golden-output naming, and golden-run criteria.
- Added `tests/hardware/scenario_manifest.json` that enumerates every WD/GD/CM scenario with command sequences, expected columns, golden CSV paths, and retention metadata.
- Documented manifest usage, telemetry deposit directories, and the 60-day evidence retention policy inside `METHODOLOGY.md`.

## Task Commits

1. **Task 1: Expand scenario matrix with golden-output contract** - `77aa7e7` (docs)
2. **Task 2: Publish machine-readable scenario manifest** - `f2af7d4` (feat)
3. **Task 3: Document evidence retention and manifest usage** - `3707bd7` (docs)

## Files Created/Modified

- `tests/hardware/scenario_manifest.json` - machine-readable manifest that documents command sequences, expected STATUS columns, golden CSV targets, and retention metadata for each fault scenario.
- `tests/hardware/SCENARIO_MATRIX.md` - expanded matrix describing STATUS telemetry columns, golden CSV linkage, naming conventions, and the golden-run qualification checklist.
- `tests/hardware/METHODOLOGY.md` - methodology addendum explaining how to use the manifest, where validation_runner deposits telemetry/metadata, retention guidelines, and the golden run checklist.

## Decisions Made

None - plan executed as specified.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration is required for these artifacts.

## Next Phase Readiness

- Ready for `98-02-PLAN.md` so `validation_runner.py`/`analysis.py` can become manifest-aware and produce the packaged golden evidence.
- `98-03-PLAN.md` can now document the operational playbook and README guidance referencing this contract and retention policy.
