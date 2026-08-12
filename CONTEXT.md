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
- ✅ All host tests pass (**680 as of 2026-08-12** — 482 unit + 247 integration test functions, 0 failures with `--features test`; the regression numeric suite adds `--features regression` → 740 passing, see Quality Gates below)
- ✅ Full-roast verification suite (`tests/full_roast_verification.rs`, 11 tests) — deterministic L1 simulation of complete roasts: preheat, charge dip, profile/fan-profile following, RoR/first-crack, all 6 safety backstops, STOP/cooldown, two consecutive roasts. Plus an L3 end-to-end pipeline test (real control-loop ticks over `simulated-sensors` curves) gated behind `--features simulated-sensors`

**Recent architecture work (v5.4):**
- RoasterControl decomposed into focused controllers (SensorController, ActuatorController — heater+fan together —, SafetyController, CommandDispatcher)
- ServiceContainer DI migration (constructor injection instead of `static_cell` singleton)
- 24 clippy warnings fixed, 17 files quality-improved
- All 680 host tests pass, ESP32 build warning-free

**Artisan compatibility audit (A-TC4, 2026-08-12):**
- Internal safety traps now emit `ERR safety_fault <reason>` on the wire, once per latch event (`emergency_shutdown` in `roaster_control.rs`) — Artisan/automation no longer discovers latches only via rejected commands. The operator `STOP` path does not emit it.
- Probe-stuck detector is two-stage in manual/software-PID mode (A-TC4-C): `ERR probe_stuck_warning` on the wire at 120 s of flat BT (no latch — a slow finish can legitimately hold BT flat), real latch at 300 s via `ERR safety_fault Probe stuck`. Firmware-PID mode keeps the original single-stage 120 s latch. The dead-probe backstop (Bug S1) stays closed in both modes.
- Handshake commands `CHAN`/`UNITS`/`FILT` are accepted while the safety latch is armed (zero actuator side effects) — Artisan can reconnect to a latched device instead of looping on "Arduino could not set channels/units/filters". All re-energizing commands remain rejected while latched.
- Golden-transcript replay suite (`tests/artisan_transcript_replay.rs` + `tests/fixtures/artisan_transcripts/*.txt`) pins the wire contract against real Artisan session bytes; `tests/pipeline_soak.rs` stress-tests the full pipeline; T-B4 covers byte-level interleave across two transports; degenerate PROFILE/FANPROFILE shapes tested at the control layer.
- CI coverage job now instruments `regression` + `simulated-sensors` (previously the conversion math and L3 pipeline showed as uncovered).

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
- `src/output/` — `ArtisanFormatter` + `MutableArtisanFormatter` (continuous-output state machine), `OutputError`; the `OutputController` flag lives in `src/control/abstractions.rs`
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

# Regression numeric suite (H-9): the strongest numeric tests
# (sensor_conversion.rs — 19-bit two's-complement LSB math, fixture→fault
# mapping) are gated `#![cfg(all(test, not(riscv32), feature = "regression"))]`
# and are NOT part of the `--features test` gate above. Run them explicitly:
cargo test --target x86_64-unknown-linux-gnu --features test --features regression --test sensor_conversion --no-fail-fast

# Simulated-sensors pipeline (L3): real control-loop ticks over the synthetic
# `simulated-sensors` temperature curves (wall clock). Includes the full
# pipeline test `control_loop_tick_simulated_sensors_full_pipeline`:
cargo test --target x86_64-unknown-linux-gnu --features test --features simulated-sensors --lib control_loop_tick_simulated

# Embedded build:
cargo build --release --target riscv32imc-unknown-none-elf --features embedded

# All host tests:
cargo test --target x86_64-unknown-linux-gnu --features test

# Race check (strongest cross-test interference check on the shared
# ServiceContainer channels):
cargo test --target x86_64-unknown-linux-gnu --features test --lib --tests --test-threads=1 --no-fail-fast

# Coverage (as CI): include regression + simulated-sensors, otherwise the
# conversion math and the L3 pipeline show as uncovered:
cargo llvm-cov --target x86_64-unknown-linux-gnu --features "test,regression,simulated-sensors" --no-fail-fast --lcov --output-path target/coverage/lcov.info
```

---

*Last updated: 2026-08-12. This file is the single source of truth for project context. If information here conflicts with other docs, update this file.*
