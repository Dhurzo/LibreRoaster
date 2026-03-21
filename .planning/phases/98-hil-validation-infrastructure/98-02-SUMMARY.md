---
phase: 98-hil-validation-infrastructure
plan: 02
subsystem: testing
tags: [manifest, telemetry, analysis, python, hardware, infra]

# Dependency graph
requires:
  - phase: 98-01
    provides: "Scenario contract, manifest, and retention policy that describe HIL scenarios."
provides:
  - "Manifest-aware validation runner that snaps STATUS telemetry into tests/hardware/runs/{scenario}/{timestamp} with metadata."
  - "Analysis pipeline that reads scenario metadata, thresholds, and golden outputs to produce Markdown verdicts."
affects: ["phase 98-03", "HIL audit evidence"]

# Tech tracking
tech-stack:
  added: [json metadata, manifest-driven CLI arguments, scenario reports]
  patterns: [manifest-directed run directories, metadata-first reports that cite golden outputs]

key-files:
  created: []
  modified:
    - tests/hardware/validation_runner.py
    - tests/hardware/analysis.py

key-decisions:
  - "Manifest metadata now governs run directories so analysis has deterministic scenario context."
  - "Reports surface scenario descriptions, command sequences, and thresholds to satisfy HIL audit evidence."

patterns-established:
  - "Runner always stamps tests/hardware/runs/{scenario}/{timestamp}/telemetry.csv + metadata for auditors."
  - "Analysis reuses the template and layers in manifest context, thresholds, and verdict lines referencing golden outputs."

# Metrics
duration: 3m 42s
completed: 2026-03-20
---

# Phase 98 Plan 02: Manifest-aware HIL validation

**Manifest-driven telemetry capture and analysis that binds each HIL run to the manifest metadata and golden outputs.**

## Performance

- **Duration:** 3m 42s
- **Started:** 2026-03-20T13:57:47Z
- **Completed:** 2026-03-20T14:01:29Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Validation runner now loads tests/hardware/scenario_manifest.json, writes runs under tests/hardware/runs/{scenario}/{timestamp}, and emits metadata JSON with manifest entry and max command latency.
- Analysis pipeline now scans runs/{scenario}/{timestamp}, loads manifest entries and thresholds, compares telemetry to thresholds, and writes Markdown verdicts that reference golden_output paths and scenario descriptions.

## Task Commits

1. **Task 1: Make validation runner manifest-aware** - `e11774e` (feat)
2. **Task 2: Teach analysis script to use the manifest** - `2631a7c` (feat)

## Files Created/Modified

- `tests/hardware/validation_runner.py` - added manifest-driven CLI arguments, organized run directories, and recorded metadata per scenario run.
- `tests/hardware/analysis.py` - loads the manifest, thresholds, and per-run metadata to produce scenario-aware Markdown reports referencing golden outputs.

## Decisions Made

- Manifest metadata now dictates run organization so analysis can reliably map telemetry to scenario IDs and artifacts.
- Reports embed scenario descriptions, command sequences, and thresholds to give auditors clear evidence links to golden outputs.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None

## User Setup Required

None

## Next Phase Readiness

- Ready for 98-03: the validation pipeline now produces manifest-aware telemetry and verdict reports, so follow-on retention and reporting work can assume stable evidence paths.
