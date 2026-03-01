---
phase: 80-handler-pattern
plan: 01
subsystem: control
tags: [artisan, handler, manual, roaster]

# Dependency graph
requires:
  - phase: 79-test-infrastructure
    provides: Stable test harnesses that exercise Artisan commands through the mock UART shim.
provides:
  - Manual Artisan commands now route through `forward_artisan_manual_command` so `process_command` stays on the critical path.
  - Guard-aware hardware helpers now use handler-provided manual setpoints when actuating the SSR/fan outputs.
  - The ArtisanCommandHandler remains the single source of truth after removing manual cases from the temperature handler.
  - Handler pattern planning

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Manual commands now flow from `process_artisan_command` → `forward_artisan_manual_command` → `process_command`, keeping handler logic centralized.
    - Hardware actuation happens after handlers succeed so the Artisan handler can synchronize its manual setpoints with the SSR/fan helpers.

key-files:
  created: []
  modified:
    - src/control/roaster_refactored.rs
    - src/control/handlers.rs

key-decisions:
  - Manual Artisan commands should be handled exclusively by `ArtisanCommandHandler` so its manual setpoints stay in sync with SSR and fan outputs.

patterns-established:
  - `forward_artisan_manual_command` centralizes manual command dispatch while leaving the rest of `process_artisan_command` focused on start/stop/status commands.
  - `apply_manual_heater`/`apply_manual_fan` act on handler-supplied values after `process_command` returns `Ok`, keeping hardware updates in one place.

# Metrics
duration: 9 min
completed: 2026-03-01
---

# Phase 80: Handler Pattern Summary

**Manual Artisan commands now travel through `process_command`, letting `ArtisanCommandHandler` stay authoritative for manual heater/fan setpoints while hardware helpers synchronize SSR/fan outputs.**

## Performance

- **Duration:** 9 min
- **Started:** 2026-03-01T10:27:33Z
- **Completed:** 2026-03-01T10:36:30Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments

- Added `forward_artisan_manual_command` so OT1/OT2/UP/DOWN dispatch reuses `process_command` while other Artisan branches stay local.
- Let the handler loop own manual commands and run guard-aware `apply_manual_heater`/`apply_manual_fan` helpers that read handler-set values.
- Removed the manual heater branch from `TemperatureCommandHandler` so `ArtisanCommandHandler` remains the single source of truth and the OT1/IO3 integration test now passes.

## Task Commits

1. **Delegate Artisan manual commands to process_command** - `d827782` (`feat(80-01): delegate artisan manual commands`)
2. **Let ArtisanCommandHandler drive manual state while still actuating hardware** - `f942e9d` (`fix(80-01): let handler own manual outputs`)

**Plan metadata:** pending docs commit

## Files Created/Modified

- `src/control/roaster_refactored.rs` - Manual commands call `forward_artisan_manual_command`, and manual helper functions now act on handler-provided values before guarding the SSR/fan hardware.
- `src/control/handlers.rs` - The temperature handler no longer claims `SetHeaterManual`, leaving the Artisan handler as the authoritative manual mode controller.

## Verification

- `cargo check --lib`
- `cargo test --test fan_serialization --test mock_uart_integration`

## Decisions Made

- Keep manual Artisan commands inside `ArtisanCommandHandler` so its manual setpoints stay aligned with the hardware helpers.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- `mock_uart_integration::start_ot1_io3_stop_sequence_updates_state` initially failed because `SetHeaterManual` reached `TemperatureCommandHandler`; removing the manual branch from that handler and keeping ArtisanCommandHandler authoritative resolved the test.

## User Setup Required

None - no external configuration required.

## Next Phase Readiness

- Handler Pattern plan 80-01 complete; ready for the remaining Handler Pattern plans.

---
*Phase: 80-handler-pattern*
*Completed: 2026-03-01*
