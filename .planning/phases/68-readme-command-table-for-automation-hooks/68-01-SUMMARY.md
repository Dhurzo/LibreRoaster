---
phase: 68-readme-command-table-for-automation-hooks
plan: 01
subsystem: documentation
tags: [automation, instrumentation, README, docs]

# Dependency graph
requires:
  - phase: 66-instrumentation-observability
    provides: deterministic STATUS instrumentation payload documentation and telemetry hooks
provides:
  - README entries that surface the REG and STATUS/STAT automation hooks alongside the Supported Artisan Commands table
affects: [automation tooling, instrumentation teams]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Keep automation hooks adjacent to the Supported Artisan Commands table so instrumentation crews discover them while scanning the core commands."
    - "Point immediately to INSTRUMENTATION_README.MD for STATUS column definitions and REG regression telemetry handling."

key-files:
  created:
    - .planning/phases/68-readme-command-table-for-automation-hooks/68-01-SUMMARY.md
  modified:
    - README.md

key-decisions:
  - "None - followed plan as specified"

patterns-established:
  - "Automated telemetry hooks (REG, STATUS/STAT) live where readers already expect Artisan commands."
  - "Documentation directly links to the instrumentation guide so STATUS/REG payload expectations are decoded without extra hunting."

# Metrics
completed: 2026-02-23
---

# Phase 68-readme-command-table-for-automation-hooks Plan 01 Summary

**Document REG and STATUS/STAT automation hooks in the command table and point instrumentation readers to the STATUS payload guide.**

## Performance

- **Duration:** 1 min
- **Started:** 2026-02-23T18:45:43Z
- **Completed:** 2026-02-23T18:46:52Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments

- Reworded the REG entry so automation sees the regression-runner trigger, guaranteed watchdog feed, and SAFETY OT-REGRESSION emission that signals over-temperature cycles.
- Clarified the STATUS/STAT entry with every telemetry field, the `STAT` alias, and the fact it surfaces watchdog/guard/regression telemetry without touching `READ`.
- Linked `internalDoc/INSTRUMENTATION_README.MD` immediately after the table for STATUS column definitions, payload expectations, and how REG logs SAFETY OT-REGRESSION.

## Task Commits

1. **Task 1: Add REG and STATUS/STAT rows to the command table** - `46df20f` (docs)
2. **Task 2: Reference the instrumentation guide near automation hooks** - `9814b01` (docs)

**Plan metadata:** docs(68-01): complete README command table automation hooks plan

## Files Created/Modified

- `README.md` - expanded the command table descriptions for REG and STATUS/STAT plus directly referenced `INSTRUMENTATION_README.MD` after the table.

## Decisions Made

None - followed plan as specified.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external services introduced.

## Next Phase Readiness

- Automation teams can now discover REG and STATUS/STAT in the README and follow the instrumentation guide to decode STATUS payloads and REG regression telemetry without digging through other docs.
