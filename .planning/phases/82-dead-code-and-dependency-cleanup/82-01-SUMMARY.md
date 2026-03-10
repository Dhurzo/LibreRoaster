---
phase: 82-dead-code-and-dependency-cleanup
plan: 82-01
subsystem: quality
tags: [rust, clippy, dead_code, inventory, risk]

# Dependency graph
requires:
  - phase: 81-quality-baseline-and-ratcheting-policy
    provides: deterministic baseline gating and policy ratchets
provides:
  - dead-code inventory snapshots with metadata for DC-01 reviewers
  - risk classification guidance that links each bucket to explicit evidence expectations
affects: [phase-82-removal-workflow, phase-83-rust-modernization]

# Tech tracking
tech-stack:
  added: [python3]
  patterns:
    - clippy-driven dead-code inventory capturing toolchain + Git metadata
    - evidence-linked risk buckets that cite the latest JSON pointer

key-files:
  created:
    - scripts/dead-code-inventory.sh
    - quality/dead-code/README.md
    - quality/dead-code/inventory/dead-code-inventory.json
  modified:
    - quality/dead-code/inventory/20260307T124811Z-dead-code.json

key-decisions:
  - "Use a Python-powered clippy pipeline to stamp each candidate with Git/toolchain metadata and emit a human summary plus stable pointer."
  - "Documented the high/medium/low buckets so DC-01 graders can map every removal candidate to a concrete inventory entry."

patterns-established:
  - "Emitting timestamped JSON snapshots plus a stable latest pointer lets downstream automation read the freshest dead-code signals without rerunning clippy."
  - "Risk guidance now couples each bucket with required evidence checklists (inventory entry, coverage, manual trace) before removals."

# Metrics
duration: 12m 34s
completed: 2026-03-07
---

# Phase 82 Plan 82-01 Summary

**Deterministic dead-code inventory with risk-aware guidance for DC-01.**

## Performance

- **Duration:** 12 min 34 sec
- **Started:** 2026-03-07T12:36:29Z
- **Completed:** 2026-03-07T12:49:03Z
- **Tasks:** 2
- **Files modified:** 4 (inventory script, README, timestamped snapshot, latest pointer)

## Accomplishments

- Added `scripts/dead-code-inventory.sh` to rerun Clippy, capture `dead_code` metadata, and publish both JSON payloads and a human-readable summary.
- Persisted `quality/dead-code/inventory/dead-code-inventory.json` plus timestamped snapshots so reviewers and automation can gate on the freshest evidence.
- Authored `quality/dead-code/README.md` that maps high/medium/low risk buckets to the new inventory outputs and explains labeling/evidence expectations for DC-01.

## Task Commits

Each task was committed atomically:

1. **Task 1: Capture dead code signals** - `6247625` (feat)
2. **Task 2: Publish risk classification guidance** - `77fc44c` (docs)

**Plan metadata:** `77fc44c` (docs: complete 82-01 plan)

## Files Created/Modified

- `scripts/dead-code-inventory.sh` - runs Clippy + python pipeline, writes metadata-rich JSON snapshots, and prints a curated summary.
- `quality/dead-code/README.md` - defines tiered risk buckets, lists evidence requirements, and references the inventory pointer for cross-checking.
- `quality/dead-code/inventory/20260307T124811Z-dead-code.json` - timestamped `dead_code` snapshot from the latest run.
- `quality/dead-code/inventory/dead-code-inventory.json` - stable pointer that always references the freshest snapshot for automation.

## Decisions Made

- Adopted a Python-based parser so the inventory script remains portable without depending on `jq` while still capturing Git/toolchain metadata and human summaries.
- Tied each risk bucket to the new inventory outputs (exact file, line, and `dead_code` message) so DC-01 reviewers can verify evidence before removal.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Adjusted the Clippy invocation to skip the embedded binary**

- **Found during:** Task 1
- **Issue:** Building the embedded `libreroaster` binary with `--all-targets --all-features` fails on the host due to the riscv32-only entry point and panic handler.
- **Fix:** Limit the inventory script to the library/tests/benches/examples set and parse the JSON via Python so the script exits cleanly on the host environment.
- **Files modified:** `scripts/dead-code-inventory.sh`
- **Verification:** The script now produces the desired JSON/summary output and writes both timestamped and latest files without Clippy aborting.

## Issues Encountered

- None beyond the managed Clippy-target adjustment documented above.

## User Setup Required

None - no external service configuration is required for this instrumentation.

## Next Phase Readiness

- Inventory + risk guidance ready for **82-02 (removal batch workflow)** so DC-01 removals can cite concrete evidence.
- No blockers, just rerun `scripts/dead-code-inventory.sh` before each removal batch to refresh the pointer.
