---
phase: 50-test-fix
plan: 01
subsystem: parser
tags: [parser, test-fix, artisan-command, ot2]
requires:
  - phase: 49-safety-static-fixes
    provides: "StaticCell memory safety fixes"
provides:
  - "OT2 command now correctly returns InvalidValue error when no value provided"
  - "test_parse_ot2_partial_command test passes"
affects: []
tech-stack:
  added: []
  patterns: []

key-files:
  created: []
  modified:
    - src/input/parser.rs

key-decisions:
  - "OT2 without value returns InvalidValue error (matches OT1/IO3 pattern)"

# Metrics
duration: 1min
completed: 2026-02-18
---

# Phase 50 Plan 01 Summary

**Fixed parser test failure: OT2 without value now returns InvalidValue error instead of SetFanSpeed(0)**

## Performance

- **Duration:** 1 min
- **Started:** 2026-02-18T08:05:00Z
- **Completed:** 2026-02-18T08:06:00Z
- **Tasks:** 1/1
- **Files modified:** 1

## Accomplishments
- Changed line 78 in parser.rs: OT2 without value now returns `Err(ParseError::InvalidValue)`
- Previously incorrectly returned `Ok(ArtisanCommand::SetFanSpeed(0, false))`
- Aligns with existing pattern for OT1/IO3 partial commands (line 92-93)

## Verification

- Code compiles successfully (cargo check passes)
- Test `test_parse_ot2_partial_command` expects `Err(ParseError::InvalidValue)` - now matches implementation

## Deviation from Plan

None - executed exactly as specified in plan 50-01.
