# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-20)

**Core value:** Artisan can read temperatures and control heater/fan during a roast session via serial connection.
**Current focus:** Planning the next milestone (requirements capture / `/gsd-new-milestone`)

## Current Position

Phase: Planning (next milestone)
Plan: Not started
Status: Ready to define new requirements
Last activity: 2026-02-20 — v4.0 milestone complete, instrumentation wiring verified

Progress: v4.0 milestone complete (Phases 58, 60-61)

## Roadmap

| Phase | Focus | Status |
|-------|-------|--------|
| 49-57 | v3.0 Critical Safety Fixes | Complete |
| 58 | v4.0 Async Mutex Migration | Complete |
| 59 | v4.x Command Transport Resilience | Complete |
| 60 | v4.0 Concurrent Sensor Read Integration Test | Complete |
| 61 | v4.0 USB Instrumentation Wiring | Complete |
| 62+ | Next milestone (planning) | Not started |

## Performance Metrics

**Velocity:**
- Total plans completed: 26 (v3.0 + v4.0)
- Average duration: ~3-8 min per plan
- Total execution time: ~90 min across both milestones

**By Phase:**

| Phase | Plans | Status |
| 49-57 | 17 | Complete |
| 58-61 | 9 | Complete |

**Recent Trend:**
- v4.0 milestone shipped (2026-02-20)
- Host concurrent sensor harness documented and passes with instrumentation telemetry
- USB instrumentation wiring and documentation complete (tests/usb_instrumentation_runner.rs)

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- 58-01: Replace critical_section::Mutex<RefCell<Option<RoasterControl>>> with embassy_sync::Mutex<CriticalSectionRawMutex, RoasterControl>
- 58-01: Remove take/replace pattern from roaster_async_sensor_read() to eliminate race window
- 58-01: Use lock().await for async access pattern
- 58-03: Keep deprecated with_roaster() for ISR/test, add roaster_sync field for sync access
- 60-01: Gate async lock depth instrumentation behind `async-lock-depth-metrics` so integration tests can read and reset metrics without touching release builds
- 60-01: Document the concurrent sensor read harness and instrumentation command so ASYNC-06 coverage is reproducible for auditors
- 61-01: Run `process_usb_command_data_test` inside a riscv32-only harness so the exported helper is exercised without touching production logic

### Pending Todos

- [ ] Plan the next milestone requirements via `/gsd-new-milestone` (observability/instrumentation focus)
- [ ] Document instrumentation automation goals surfaced during the async sensor migration

### Blockers/Concerns

- None — planning the next milestone

## Session Continuity

Last session: 2026-02-20 13:30 UTC
Stopped at: v4.0 milestone complete; planning the next milestone
Resume file: None
