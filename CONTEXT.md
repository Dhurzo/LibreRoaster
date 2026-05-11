# LibreRoaster — Agent Context

*Entry point for LLM agents. Read this first before modifying code.*

---

## What This Is

ESP32-C3 firmware for a coffee roaster controller. Allows [Artisan](https://artisan-scope.org/) roasting software to read temperatures and control heater/fan via USB CDC or UART using a TC4-compatible serial protocol.

**Core value proposition:** Artisan can read temperatures and control heater/fan during a roast session via serial connection.

**Current milestone:** v0.1 — First working version (released 2026-04-30). Firmware compiles, flashes, boots on ESP32-C3 hardware without panics, responds to Artisan READ with real temperatures.

## Technical Stack

| Layer | Technology |
|-------|-----------|
| **MCU** | ESP32-C3 (RISC-V, single-core) |
| **Runtime** | `no_std` embedded, Embassy async executor |
| **HAL** | esp-hal, embedded-hal 1.0 |
| **Sensors** | 2× MAX31856 thermocouples (SPI, shared bus) |
| **Actuators** | SSR heater (LEDC PWM GPIO10), PWM fan (LEDC PWM GPIO9) |
| **Comms** | USB CDC (ttyACM0), UART0 (GPIO20/21) |
| **Safety** | RTC watchdog, over-temperature cutoff, stale-temperature guard, heat-source detection |
| **Language** | Rust 2021 edition, stable toolchain |
| **Build target** | `riscv32imc-unknown-none-elf` (embedded), `x86_64-unknown-linux-gnu` (tests) |

## Runtime Architecture

The firmware boots, initialises LEDC/SPI/USB/UART/sensors/actuators, builds `RoasterControl` through `AppBuilder`, then spawns 7 long-lived Embassy tasks:

1. **USB reader** — consumes raw USB CDC bytes
2. **UART reader** — consumes raw UART bytes
3. **USB queue processor** — parses USB commands into shared command channel
4. **UART queue processor** — parses UART commands into shared command channel
5. **Control loop** — drains commands, reads sensors, updates control, feeds watchdog, emits telemetry (~100 ms cadence)
6. **Dual output** — routes formatted output to active transport
7. **Regression** — handles over-temperature regression runs on embedded targets

The system is wired through a `ServiceContainer` singleton that owns `RoasterControl` (async mutex), command/output channels, the command multiplexer, and watchdog feeder.

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| **TC4 protocol compatibility** | Artisan expects TC4-style serial responses. All responses must remain byte-identical. |
| **Embassy async model** | Cooperative multitasking — `Send` bounds are critical; no blocking I/O in tasks. |
| **Host/embedded split** | `embedded` feature for real hardware, `test` feature for host-side verification. |
| **No persistence** | Roast state, telemetry, profiles are RAM-only (no storage layer). |
| **Safety-first** | Multiple independent safety layers (watchdog, thermal cutoff, stale-temperature) — no single point of failure. |

## Project State

**v0.1 released** (2026-04-30):
- ✅ Firmware compiles and flashes to ESP32-C3
- ✅ All hardware inits: SPI, MAX31856×2, SSR (310 Hz), Fan (25 kHz LEDC), RTC WDT
- ✅ USB CDC responds to Artisan `READ` with TC4 format
- ✅ Control loop cycles at ~160 ms
- ✅ 243/244 host tests pass (1 pre-existing failure in `ssr_scheduler`)

**Recent architecture work (v5.4):**
- RoasterControl decomposed into focused controllers (Temperature, Heater, Fan, Safety)
- ServiceContainer DI migration (constructor injection instead of `static_cell` singleton)
- 24 clippy warnings fixed, 17 files quality-improved
- All 244 host tests pass, ESP32 build warning-free

## Known Constraints

- `no_std` environment — no alloc, no standard library on target
- Clippy config denies: `unwrap_used`, `expect_used`, `panic` in production code
- GPIO9 is a strapping pin — external fan must not force invalid boot state
- SPI MISO routed through GPIO5 (not GPIO2) to avoid FSPIQ strap conflict
- Command queue intentionally small and rate-limited
- Sensor timing pressure: MAX31856 reads are slow relative to 100 ms PID cadence

## Documentation Map

Read these in order depending on what you need to do:

| If you need to... | Read this first |
|---|---|
| Understand the full system | `docs/ARCHITECTURE.md` |
| Add/modify a serial command | `docs/PROTOCOL.md` |
| Change pin assignments or hardware init | `docs/HARDWARE.md` |
| Fix a bug | `docs/BUGS.md` + `docs/CONTROL_BUG_AUDIT.md` |
| Build, flash, test | `docs/DEVELOPMENT.md` |
| Understand telemetry/STATUS fields | `docs/INSTRUMENTATION.md` |
| Configure Artisan integration | `docs/ARTISAN_CONNECTION.md` |
| Check compatibility boundaries | `docs/ARTISAN_COMPATIBILITY_REPORT.md` |
| Follow coding conventions | `docs/CONVENTIONS.md` |
| Run quality gates | `.planning/quality/README.md` |

**Source layout:**
- `src/main.rs` — Entry point (binary)
- `src/lib.rs` — Library root (`no_std`)
- `src/application/` — App orchestration, `AppBuilder`, `ServiceContainer`, Embassy tasks
- `src/control/` — `RoasterControl`, command handlers, PID, safety
- `src/hardware/` — MAX31856, SSR, fan, UART, shared SPI
- `src/input/` — Artisan command parser
- `src/output/` — `ArtisanFormatter`, output manager, scheduler
- `src/config/` — Constants, `SystemStatus`, command enums
- `src/error/` — `AppError` types

## Quality Gates

```bash
# Full baseline (fmt → clippy → test):
cargo fmt --all -- --check && cargo clippy --locked --all-targets -- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic && cargo test --locked --lib --tests --no-fail-fast

# Embedded build:
cargo build --release --target riscv32imc-unknown-none-elf --features embedded

# All host tests:
cargo test --target x86_64-unknown-linux-gnu --features test
```

---

*Last updated: 2026-05-11. This file is the single source of truth for project context. If information here conflicts with other docs, update this file.*
