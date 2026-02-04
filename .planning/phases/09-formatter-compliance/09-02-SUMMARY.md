---
phase: 09-formatter-compliance
plan: "02"
subsystem: api
tags: [rust, uart, artisan, error-schema, formatter]

# Dependency graph
requires:
  - phase: 08-command-hardening
    provides: Explicit command errors and bounds enforcement
  - phase: 09-formatter-compliance
    provides: Deterministic READ formatting (09-01)
provides:
  - ERR code/message helpers for parse and handler errors
  - Consistent ERR schema in UART parse and handler outputs
  - Tests asserting 3-token ERR schema for invalid inputs
affects:
  - 10-mock-uart-integration

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "ERR <code> <message> schema for error output"
    - "RoasterError token mapping for handler_failed"

key-files:
  created: []
  modified:
    - src/input/parser.rs
    - src/control/abstractions.rs
    - src/hardware/uart/tasks.rs
    - src/application/tasks.rs
    - tests/command_errors.rs

key-decisions:
  - "None - followed plan as specified"

patterns-established:
  - "ParseError code/message helpers as canonical tokens"
  - "ERR handler_failed uses bounded message tokens"

# Metrics
duration: 0 min
completed: 2026-02-04
---

# Phase 09 Plan 02: Formatter Compliance Summary

**Unified ERR code/message schema across parse and handler failures with tests enforcing 3-token output.**

## Performance

- **Duration:** 0 min
- **Started:** 2026-02-04T16:06:50Z
- **Completed:** 2026-02-04T16:07:19Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments
- Added canonical error code/message helpers for parse and handler failures
- Standardized ERR output to `ERR <code> <message>` across UART parse and handler paths
- Updated tests to assert stable tokenization for invalid command inputs

## Task Commits

Each task was committed atomically:

1. **Task 1: Add canonical error code/message helpers** - `c879984` (feat)
2. **Task 2: Emit ERR lines with consistent schema** - `7b92259` (feat)
3. **Task 3: Align error tests with schema** - `b97aad8` (test)

**Plan metadata:** TBD

_Note: TDD tasks may have multiple commits (test → feat → refactor)_

## Files Created/Modified
- `src/input/parser.rs` - ParseError code/message token helpers
- `src/control/abstractions.rs` - RoasterError message token mapping
- `src/hardware/uart/tasks.rs` - ERR emission using ParseError helpers
- `src/application/tasks.rs` - ERR handler_failed emission with bounded tokens
- `tests/command_errors.rs` - ERR schema token assertions for invalid inputs

## Decisions Made
None - followed plan as specified.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- `rg` is not available in this environment; verification used the Grep tool instead.
- `cargo test --test command_errors` fails on host target due to missing `std` for riscv32imc-unknown-none-elf.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 09 formatter compliance complete; ready for Phase 10 mock UART integration.
- Host-target tests still require embedded target or host-compatible stubs.

---
*Phase: 09-formatter-compliance*
*Completed: 2026-02-04*
