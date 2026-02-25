---
phase: 66-instrumentation-observability
plan: 01
subsystem: telemetry
tags: [rust, automation, instrumentation, csv]

# Dependency graph
requires:
  - phase: 65-watchdog-timer-safety
    provides: Task watchdog/guard/regression state in `SystemStatus`
provides:
  - deterministic `STATUS` CSV that includes watchdog health, guard counts, and regression flags beside the familiar readings
  - control loop wiring that publishes the CSV to the shared Artisan output channel whenever `STATUS` arrives
  - documentation so auditors and automation know how to parse the new payload without touching `READ`
affects: [automation harnesses, future observability/monitoring phases]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Dedicated `STATUS` Artisan command for automation telemetry while keeping `READ` human-friendly and backwards compatible
    - Deterministic CSV columns enforced by regression tests to keep automation parses stable

key-files:
  created: []
  modified:
    - src/config/constants.rs
    - src/input/parser.rs
    - src/output/artisan.rs
    - src/control/roaster_refactored.rs
    - src/application/tasks.rs
    - internalDoc/INSTRUMENTATION_README.MD

key-decisions:
  - "Keep instrumentation snapshots on a dedicated `STATUS` command so the legacy `READ` stream remains untouched while automation gets richer data."
  - "Emit watchdog/guard/regression metrics as a fixed CSV column set (flags, counts, reason, guard timeouts, regression flag) so automation parsing stays simple."

patterns-established:
  - "Wiring instrumentation commands through `RoasterControl` keeps `SystemStatus` as the single source of truth."
  - "Formatter regression tests guard the column order so downstream automation does not break."
  - "Document automation hooks immediately after wiring to keep auditors aligned with engineering intent."

# Metrics
duration: 5 min 5 sec
completed: 2026-02-23
---

# Phase 66 Plan 01 Summary

**STATUS command now emits a deterministic CSV that packages watchdog, guard, and regression telemetry alongside the familiar Artisan readings for automation and auditors.**

## Performance

- **Duration:** 5 min 5 sec
- **Started:** 2026-02-23T17:33:51Z
- **Completed:** 2026-02-23T17:38:56Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments
- Added the `StatusReport` variant and parser branch that recognize `STATUS`/`STAT` and keep `READ` stable.
- Introduced `ArtisanFormatter::format_status_response`, wired the `STATUS` command through `RoasterControl`, and sent the CSV through the control loop alongside `READ`.
- Documented the automation-friendly `STATUS` payload, describing every column and instructing auditors to poll this hook instead of altering the legacy stream.
- Regression tests cover parser behavior and the CSV column order so automation fails fast if anything shifts.

## Task Commits

1. **Task 1: Teach the parser about the STATUS command** - `cf3aee3` (feat)
2. **Task 2: Format the STATUS payload and route it through the control loop** - `b8da86f` (feat)
3. **Task 3: Document the STATUS payload for automation** - `9fb29b4` (docs)

## Files Created/Modified
- `src/config/constants.rs` - adds the `StatusReport` variant so the parser can emit the new command.
- `src/input/parser.rs` - recognizes `STATUS`/`STAT` before `READ` and adds dedicated tests to guard the branch.
- `src/output/artisan.rs` - adds `format_status_response` with watchdog/guard/regression columns and regression tests for column order.
- `src/control/roaster_refactored.rs` - surfaces the formatted STATUS CSV during command processing (watchdog/guard metrics from `SystemStatus`).
- `src/application/tasks.rs` - sends the STATUS CSV to the shared Artisan output channel in the control loop.
- `internalDoc/INSTRUMENTATION_README.MD` - documents the `STATUS` payload, sample response, column meanings, and automation guidance.

## Decisions Made
- Added STATUS as a dedicated instrumentation hook instead of overloading READ so legacy clients keep expecting four values.
- Exposed watchdog health, guard counts, and regression activity as a fixed CSV column set (`WatchdogOK`, failure count, reason, guard timeouts, regression flag) so automation parsing stays deterministic.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external services introduced.

## Verification

1. `cargo fmt -- --check`
2. `cargo check --target x86_64-unknown-linux-gnu`
3. `cargo test input::parser::test_parse_status_command --target x86_64-unknown-linux-gnu`
4. `cargo test output::artisan::test_format_status_response --target x86_64-unknown-linux-gnu`

## Next Phase Readiness

- STATUS telemetry and docs are wired; automation harnesses can poll the new command without touching READ.
- No blockers remain; ready for further observability automation work or auditor reviews.
