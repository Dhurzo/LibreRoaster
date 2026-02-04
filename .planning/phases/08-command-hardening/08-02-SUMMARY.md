---
phase: 08-command-hardening
plan: "02"
subsystem: control
tags: [artisan, commands, pid, formatting, testing]

# Dependency graph
requires:
  - phase: 08-command-hardening/01
    provides: explicit command errors and bounds enforcement
provides:
  - Idempotent Artisan START/STOP with safe streaming reset
  - Bounded manual OT1/IO3 handling with STOP clearing overrides
  - Deterministic READ formatter with one-decimal ET,BT,Power,Fan fields
affects: [09-formatter-compliance, uart-integration, streaming]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Idempotent command guards for START/STOP"
    - "Safe stop routine zeroing outputs and clearing manual overrides"
    - "Clamped telemetry formatting for READ responses"

key-files:
  created: [tests/command_idempotence.rs]
  modified: [src/control/handlers.rs, src/control/roaster_refactored.rs, src/output/artisan.rs]

key-decisions:
  - "STOP uses a safe streaming shutdown that clears outputs/overrides without raising new faults"
  - "Manual commands are bounded in handlers and keep streaming enabled only when safe"

patterns-established:
  - "OutputController continuous flag drives START/STOP idempotence"
  - "READ responses fixed to ET,BT,Power,Fan with one-decimal precision"

# Metrics
duration: 16 min
completed: 2026-02-04
---

# Phase 8: Command Hardening Summary

**Idempotent Artisan START/STOP with bounded manual overrides and deterministic READ telemetry**

## Performance

- **Duration:** 16 min
- **Started:** 2026-02-04T15:22:59Z
- **Completed:** 2026-02-04T15:38:39Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments
- START/STOP commands guard duplicate activation and run a safe stop routine that disables streaming and zeros outputs.
- Manual OT1/IO3 handlers reject out-of-range values, keep streaming state consistent, and STOP clears manual overrides to 0.
- READ formatter now clamps output fields and returns ET,BT,Power,Fan with fixed one-decimal precision; new tests cover deterministic responses.

## Task Commits

Each task was committed atomically:

1. **Task 1: Make START/STOP idempotent and safe** - `318154b` (feat)
2. **Task 2: Enforce bounded manual outputs and STOP reset** - `bb634d5` (fix)
3. **Task 3: Lock READ response determinism** - `e287119` (feat)

## Files Created/Modified
- `tests/command_idempotence.rs` - Integration tests for START/STOP idempotence, manual bounds, and READ stability.
- `src/control/handlers.rs` - Idempotent START/STOP handling and bounded manual setpoints.
- `src/control/roaster_refactored.rs` - Safe stop routine, manual command handling, and streaming state coordination.
- `src/output/artisan.rs` - Deterministic READ response formatting with clamped values.

## Decisions Made
- STOP uses a safe streaming shutdown that clears outputs/overrides without raising new faults; emergency stop behavior remains unchanged.
- Manual commands are bounded in handlers and keep streaming enabled only when safe; STOP resets manual values to 0.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed const generic usage in `artisan` tasks channel**
- **Found during:** Task 1 (Make START/STOP idempotent and safe)
- **Issue:** Channel type in `src/application/tasks.rs` used an un-braced const generic, breaking compilation when running tests.
- **Fix:** Wrapped the const parameter for the output channel size to satisfy the compiler.
- **Files modified:** src/application/tasks.rs
- **Verification:** Compiles under the adjusted const generic (target toolchain still blocks host tests).

---

**Total deviations:** 1 auto-fixed (blocking)
**Impact on plan:** Required to keep code compiling for test execution; no scope creep.

## Issues Encountered
- Could not run `cargo test --test command_idempotence` on host: project defaults to ESP32-C3 `riscv32imc-unknown-none-elf` target, pulling in `esp-hal`/`embassy` and lacking `std`/host crates. Host execution would need target overrides or cfg-gated stubs.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Phase 8 complete; ready to start Phase 9 (formatter compliance).
- Host-side test runs remain blocked by embedded target dependencies; add host cfg or mock layers if host CI is needed.

---
*Phase: 08-command-hardening*
*Completed: 2026-02-04*
