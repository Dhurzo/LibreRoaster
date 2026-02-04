---
phase: 08-command-hardening
plan: "01"
subsystem: api
tags: [artisan, uart, parser, embassy, heapless]

# Dependency graph
requires:
  - phase: 07-final-cleanup
    provides: streamlined control/output modules
provides:
  - structured command parser errors with bounds enforcement
  - ERR responses for parse/handler failures on UART output
  - regression tests for invalid and out-of-range Artisan commands
affects: [phase-08-command-hardening, phase-09-formatter-compliance, phase-10-mock-uart-integration]

# Tech tracking
tech-stack:
  added: []
  patterns: [ERR prefix with reason codes, gated command channel on valid parses]

key-files:
  created: [tests/command_errors.rs]
  modified: [src/input/parser.rs, src/hardware/uart/tasks.rs, src/application/tasks.rs, tests/artisan_integration_test.rs]

key-decisions:
  - "Use ERR prefix with reason codes (unknown_command, invalid_value, out_of_range, handler_failed)"
  - "Guard Artisan command channel so only validated commands are enqueued"

patterns-established:
  - "Parse failures surface explicit reasons and do not mutate state"
  - "Handler failures emit ERR responses instead of silent logging"

# Metrics
duration: 11 min
completed: 2026-02-04
---

# Phase 08-command-hardening Plan 01: Summary

**Parser now surfaces structured errors, UART ingest emits ERR lines for invalid commands, and regression tests guard against out-of-range inputs**

## Performance

- **Duration:** 11 min
- **Started:** 2026-02-04T15:08:25Z
- **Completed:** 2026-02-04T15:19:33Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Parser returns explicit error reasons (unknown, invalid value, out-of-range, empty) and enforces 0–100 bounds for OT1/IO3
- UART ingest now routes parse failures to ERR responses and only forwards validated commands to the control channel
- Control loop surfaces handler failures as ERR lines, and new regression tests cover empty/unknown/malformed/out-of-range commands with no side effects

## Task Commits

1. **Task 1: Harden parser with structured errors and bounds** - `cf6efbc` (feat)
2. **Task 2: Route parse/handler errors to explicit ERR output** - `c01e90a` (fix)
3. **Task 3: Add regression tests for invalid commands and out-of-range setpoints** - `6f572c9` (test)

## Files Created/Modified

- `src/input/parser.rs` - Structured parse errors and strict OT1/IO3 bounds
- `src/hardware/uart/tasks.rs` - UART ingest routes errors to ERR output and gates command channel
- `src/application/tasks.rs` - Control loop emits ERR responses on handler failures
- `tests/artisan_integration_test.rs` - Updated expectations for new parse error variants
- `tests/command_errors.rs` - Regression tests for invalid/unknown/out-of-range commands and channel gating

## Decisions Made

- Use `ERR` prefix with reason codes (`unknown_command`, `invalid_value`, `out_of_range`, `handler_failed`) for all surfaced errors
- Only validated commands may enter the Artisan command channel; errors return immediately with ERR output

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- Host-target `cargo test` runs fail because the crate is configured for the ESP32-C3 target (esp-hal/embassy dependencies missing on x86); attempted host runs with `--target x86_64-unknown-linux-gnu --features test` but compilation failed. No automated tests executed in this environment.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Ready to execute 08-02-PLAN.md (start/stop idempotence and deterministic READ responses)
- Error response schema now established for formatter tightening in Phase 9

---
*Phase: 08-command-hardening*
*Completed: 2026-02-04*
