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
| **Actuators** | SSR heater (5 Hz zero-cross LEDC, GPIO10), PWM fan (LEDC PWM GPIO9) |
| **Comms** | USB CDC (ttyACM0), UART0 (GPIO20/21) |
| **Safety** | RTC watchdog, over-temperature cutoff, stale-temperature guard, heat-source detection |
| **Language** | Rust 2021 edition, stable toolchain |
| **Build target** | `riscv32imc-unknown-none-elf` (embedded), `x86_64-unknown-linux-gnu` (tests) |

## Runtime Architecture

The firmware boots, initialises LEDC/SPI/USB/UART/sensors/actuators, builds `RoasterControl` through `AppBuilder`, then spawns 5 long-lived Embassy tasks:

1. **USB reader** — gathers bytes from native USB CDC and parses commands
2. **UART reader** — gathers bytes from UART0 and parses commands
3. **Control loop** — drains commands, reads sensors, updates control, feeds watchdog, emits telemetry (100 ms timer; real tick ≈ 310–330 ms with the MAX31856 conversion wait)
4. **Dual output** — routes formatted output to active transport
5. **Regression** — handles over-temperature regression runs on embedded targets

> F5.3 refactor note: the separate USB/UART queue-processor tasks were removed. Reader tasks now own both byte collection and command parsing directly — there is **one shared command channel** for both transports, no intermediate queue-processor stage. Each transport keeps only a byte-level event queue (a buffer, not a task).

The system is wired through a `ServiceContainer` singleton that owns `RoasterControl` (async mutex), the artisan input, and the watchdog feeder. The command/output channels and the command multiplexer are module-level `statics` that `ServiceContainer` exposes via accessors.

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
- ✅ All hardware inits: SPI, MAX31856×2, SSR (5 Hz zero-cross), Fan (25 kHz LEDC), RTC WDT
- ✅ USB CDC responds to Artisan `READ` with TC4 format
- ✅ Control loop ticks at ≈ 310–330 ms (100 ms timer + 210 ms MAX31856 conversion wait)
- ✅ All host tests pass (**631 as of 2026-08-04** — lib + integration with `--features test`, 0 failures)

**Recent architecture work (v5.4):**
- RoasterControl decomposed into focused controllers (SensorController, ActuatorController — heater+fan together —, SafetyController, CommandDispatcher)
- ServiceContainer DI migration (constructor injection instead of `static_cell` singleton)
- 24 clippy warnings fixed, 17 files quality-improved
- All 631 host tests pass, ESP32 build warning-free

## Known Constraints

- `no_std` environment — no alloc, no standard library on target
- Clippy config denies: `unwrap_used`, `expect_used`, `panic` in production code
- GPIO9 is a strapping pin — external fan must not force invalid boot state
- SPI MISO routed through GPIO5 (not GPIO2) to avoid FSPIQ strap conflict
- Command queue intentionally small and rate-limited
- Sensor timing pressure: MAX31856 conversion (210 ms) is slow relative to the 100 ms loop timer — the real tick is ≈ 310–330 ms

## Documentation Map

Read these in order depending on what you need to do:

| If you need to... | Read this first |
|---|---|
| Understand the full system | `docs/ARCHITECTURE.md` |
| Add/modify a serial command | `docs/PROTOCOL.md` |
| Change pin assignments or hardware init | `docs/HARDWARE.md` |
| Fix a bug | Investigate source code (current bug/risk notes live in code comments; see `docs/ARCHITECTURE.md` §13) |
| Build, flash, test | `docs/DEVELOPMENT.md` |
| Understand telemetry/STATUS fields | `docs/INSTRUMENTATION.md` |
| Configure Artisan integration | `docs/ARTISAN_CONNECTION.md` |
| Check compatibility boundaries | Review source code and test implementations |
| Follow coding conventions | See `.planning/codebase/CONVENTIONS.md`, otherwise follow Rust best practices |
| Run quality gates | `.planning/quality/README.md` |

**Source layout:**
- `src/main.rs` — Entry point (binary)
- `src/lib.rs` — Library root (`no_std`)
- `src/application/` — App orchestration, `AppBuilder`, `ServiceContainer`; contains 2 of the 5 Embassy tasks (control loop, dual output)
- `src/control/` — `RoasterControl`, command handlers, PID, safety
- `src/control/controllers/` — Focused controllers (Sensor, Actuator, Safety, Dispatch)
- `src/control/handlers/` — Command handlers and artisans
- `src/hardware/` — MAX31856, SSR, fan, UART, shared SPI
- `src/hardware/sensors/` — Sensor implementations and conversions
- `src/hardware/ssr.rs` + `ssr_stub.rs` — SSR control implementations (the `ssr/` subdirectory is empty)
- `src/hardware/uart/` — UART communication (UART reader task)
- `src/hardware/usb_cdc/` — USB CDC communication (USB reader task)
- `src/input/` — Artisan command parser
- `src/output/` — `ArtisanFormatter`, formatters, traits (the continuous-output state machine, `OutputController`, lives in `src/control/abstractions.rs`)
- `src/output/formatters/` — Output formatting implementations
- `src/config/` — Constants, `SystemStatus`, command enums
- `src/error/` — `AppError` types
- `src/logging/` — Logging infrastructure, telemetry, TRACE stream, roast ring buffer (`roast_logger.rs`)
- `src/memory/` — Memory constants and strategy notes
- `src/safety/` — Safety implementations, watchdogs, regression task
- `src/common/` — Common utilities and shared functionality

## Quality Gates

```bash
# Full baseline (fmt → clippy → test):
# NB: the plain `cargo test --locked --lib --tests` command FAILS at link
# time (`undefined symbol: _embassy_time_now`) — host tests need the `test`
# feature (and the host target) so the Embassy time driver is provided.
cargo fmt --all -- --check && cargo clippy --locked --all-targets -- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic && cargo test --target x86_64-unknown-linux-gnu --features test --lib --tests --no-fail-fast

# Embedded build:
cargo build --release --target riscv32imc-unknown-none-elf --features embedded

# All host tests:
cargo test --target x86_64-unknown-linux-gnu --features test
```

---

*Last updated: 2026-08-04. This file is the single source of truth for project context. If information here conflicts with other docs, update this file.*
