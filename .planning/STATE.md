# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-18)

**Core value:** Artisan can read temperatures and control heater/fan during a roast session via serial connection.
**Current focus:** v2.6 shipped, ready for next milestone

## Current Position

Phase: 54 of 54 (Clean Up Tech Debt)
Plan: 02 complete
Status: In progress
Last activity: 2026-02-18 — Completed 54-02 compilation warnings fix

Progress: ██████░░░░ 67% (v3.0 Phase 54: Clean Up Tech Debt)

## Roadmap

| Phase | Focus | Status |
|-------|-------|--------|
| 49 | Safety Static Fixes | Complete |
| 50 | Test Fix | Complete |
| 51 | Documentation | Complete |
| 52 | Performance Fixes | Complete |
| 53 | Async Temperature Integration | Complete |
| 54 | Clean Up Tech Debt | In progress |

## Performance Metrics

**Velocity:**
- Total plans completed: 10 (v2.6)
- Average duration: ~2-7 min
- Total execution time: ~30 min

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
| 46-48 | 10 | 10 | ~3 min |

**Recent Trend:**
- v2.6 milestone shipped (2026-02-18)
- Ready for next milestone

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- 38-01: READ format is 4-value CSV (ET,BT,HEATER,FAN)
- 38-01: READ format uses one-decimal precision
- 46-01: Focus on SSR/LED PWM reliability and async UART/USB I/O for v2.6
- 46-01: Share SSR guard/tolerance knobs via `config::constants` and lock them with tests so scheduler/monitor helpers stay aligned.
- 46-02: Guard busy windows now live in `ssr_cycle_guard_busy_until_ms` so telemetry reports when SSR commands are rejected.
- 46-02: `log_channel!` routes non-riscv builds through `log::info!` while keeping `esp_println` for hardware.
- 46-02: SSR duty rounding now uses integer math to avoid pulling in `FloatCore` when compiling in `no_std`.
- 46-03: Gated LEDC register reads behind `LedcDutyReader` so monitor helpers stay testable on host builds.
- 46-03: RoasterControl telemetry now exposes `ssr_last_duty_delta_ticks` and `ssr_retry_count` immediately after heater writes.
- 47-01: FanController now depends on LedcChannelHandle so fan writes use the shared mask and report applied duty.
- 47-01: AppBuilder now requires a supplied fan implementation instead of configuring LEDC itself, keeping wiring centralized.
- 47-02: SSR wired through LedcBus handle, RoasterControl reads actual applied fan duty post-write
- 48-01: UART converted to async using embedded_io_async traits with esp-hal's into_async()
- 48-01: Event queue uses heapless::Deque (256 bytes) with ring buffer behavior for burst handling
- 48-02: USB CDC uses WouldBlock error variant for back-pressure detection instead of busy-waiting
- 48-02: USB writer yields with exponential backoff (1ms → 10ms) during congestion
- 48-03: CommandQueue uses heapless::Deque for no_std compatibility with 32-command capacity
- 48-03: On queue full: silently drop command (Artisan times out) - reject-on-full behavior
- 48-04: Integration flood tests use host target (x86_64) since embedded target lacks std
- 48-05: Queue processor tasks wired to consume CommandQueue and send to artisan_channel
- 49-01: Replaced unsafe make_static with StaticCell::init() pattern
- 49-01: Used StaticCell with raw pointer storage for USB/UART driver singletons
- 49-01: Used ConstStaticCell::take() for ServiceContainer singleton pattern
- 50-01: OT2 without value returns InvalidValue error (matches OT1/IO3 pattern)
- 52-01: MAX31856 async read uses embassy_time Timer::after(Duration::from_millis(160)) replacing blocking spin loop
- 52-01: MAX31856 retry logic uses fixed 10ms delay, attempts max_retries+1 times (2 retries = 3 total)
- 53-02: RoasterControl has async read_sensors() method with infrastructure ready for async integration
- 53-03: Concrete Max31856 types enable async temperature reads without blocking executor
- 54-01: All identified dead code removed entirely (fan_timer, ssr_timer, handle_complete_command, send_parse_error)
- 54-01: Timer configuration handled internally by Channel implementation

### Pending Todos

- [x] v3.0: Fix make_static Use-After-Free (main.rs:40-43) - DONE
- [x] v3.0: Fix mutable statics safety (driver.rs:118, 62) - DONE
- [x] v3.0: Fix ServiceContainer::get_instance() (service_container.rs:37-40) - DONE
- [x] v3.0: Fix test_parse_ot2_partial_command failure - DONE
- [x] v3.0: Fix README vs PROTOCOL.md mismatch - DONE
- [x] v3.0: Fix blocking MAX31856 temperature read - DONE
- [x] v3.0: Fix SSR/Fan shared LEDC timer - DONE

### Blockers/Concerns

- `cargo test --test ssr_monitor` cannot run on the default `riscv32imc-unknown-none-elf` target because it lacks `std`/`test`; use a host target to verify the suite.

## Session Continuity

Last session: 2026-02-18 17:00 UTC
Stopped at: Completed 54-02 compilation warnings fix
Resume file: None
