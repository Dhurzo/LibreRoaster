# LibreRoaster — Agent Context

*Entry point for LLM agents. Read this first, then follow the links for technical detail.*

---

## What This Is

ESP32-C3 firmware for a coffee roaster controller. It exposes a TC4-compatible serial surface so
[Artisan](https://artisan-scope.org/) can read temperatures and control heater/fan over native USB
CDC or UART.

**Core value proposition:** Artisan owns the roast session; LibreRoaster owns the device-side
control loop, telemetry, and safety interlocks.

**Current milestone:** v0.0.1 Alpha on `develop`. Firmware compiles, flashes, and boots on
ESP32-C3 hardware; USB CDC answers Artisan `READ` with TC4-formatted readings. Real thermocouple
reads, physical Artisan connectivity, and full hardware integration remain pending.

## Technical Stack

| Layer | Technology |
|---|---|
| **MCU** | ESP32-C3 (RISC-V, single-core) |
| **Runtime** | `no_std` embedded, Embassy async executor |
| **HAL** | esp-hal, embedded-hal 1.0 |
| **Sensors** | 2× MAX31856 thermocouples on a shared SPI bus |
| **Actuators** | SSR heater (5 Hz zero-cross LEDC, GPIO10), PWM fan (25 kHz LEDC, GPIO9) |
| **Comms** | USB CDC (`/dev/ttyACM0`), UART0 (GPIO20/21, 115200 8N1) |
| **Safety** | RTC watchdog, over-temperature cutoff, stale-temperature guard, RoR guard, heat-source detection |
| **Language** | Rust 2021, stable toolchain |
| **Targets** | `riscv32imc-unknown-none-elf` (embedded), `x86_64-unknown-linux-gnu` (host tests) |

## Runtime Architecture

`src/main.rs` initializes hardware, builds `RoasterControl` through `AppBuilder`, then starts one
supervisor plus five long-lived Embassy tasks:

1. **USB reader** — collects native USB CDC bytes and parses commands.
2. **UART reader** — collects UART0 bytes and parses commands.
3. **Control loop** — drains commands, reads sensors, updates control, feeds the watchdog, and
   emits telemetry. The timer is 100 ms; a real tick is ≈310–330 ms because of the MAX31856
   conversion wait.
4. **Dual output** — routes formatted output to the active transport.
5. **Regression** — runs explicit over-temperature regression checks on embedded targets.

Both readers feed one shared command channel. Each transport has only a byte-level event queue;
there is no intermediate queue-processor task.

`ServiceContainer` is the process-wide singleton assembled before the executor starts. It owns the
async-mutex `RoasterControl`, the Artisan input, and the watchdog feeder; the command/output
channels and command multiplexer are module-level statics exposed through accessors.

For the full model, read [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) and
[`docs/CONTROL_LOOP.md`](docs/CONTROL_LOOP.md).

## Key Design Decisions

| Decision | Rationale |
|---|---|
| **TC4 wire compatibility** | Artisan expects TC4-style responses; wire bytes are a compatibility contract. |
| **Embassy async model** | Cooperative multitasking with `Send` bounds and no blocking I/O inside tasks. |
| **Host/embedded split** | `embedded` selects firmware hardware paths; `test` supplies host-side verification support. |
| **No persistence** | Roast state, telemetry, and profiles are RAM-only. |
| **Safety-first layering** | Watchdog, thermal cutoff, stale-data guard, and command gates are independent backstops. |
| **Single command channel** | USB and UART share one parser/command path; the multiplexer selects the active transport. |

## Current State

- Firmware compiles and flashes to ESP32-C3.
- Hardware init covers SPI, both MAX31856 channels, SSR (5 Hz), fan (25 kHz), USB CDC, UART0, and
  the RTC watchdog.
- USB CDC responds to Artisan `READ` in TC4 format.
- The control loop runs at the documented ≈310–330 ms real tick.
- Host tests are green. The current command matrix and “last verified” snapshot live in
  [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md).
- Full-roast simulation, the L3 pipeline test, and Wokwi F1/F2/F3/F4 scenarios exist; an
  RFC2217-to-PTY bridge is available for connecting the real Artisan app to the simulation. See
  [`docs/SIMULATION.md`](docs/SIMULATION.md).
- Real Artisan 4.0.2 has been validated against the Wokwi simulation (TC4 handshake, live
  `READ`, `START` recording) including a full 696 s public medium-roast profile in F4; the desktop
  app on physical hardware remains pending.
- Pending: real thermocouple reads, real fan/heater control, and a real coffee roast. Do not
  treat synthetic validation as electrical safety evidence.

## Known Constraints

- `no_std` target: no standard library. A 72 KiB heap allocator is initialized in `main`, so the
  firmware is not allocation-free.
- Clippy denies `unwrap_used`, `expect_used`, and `panic` in production code.
- GPIO9 is a strapping pin; external fan circuitry must not force an invalid boot state.
- SPI MISO uses GPIO5, not GPIO2, because GPIO2 is a boot-mode strap (and the native FSPIQ pin).
- The command channel holds 16 entries and uses `try_send` backpressure only; there is no
  per-tick command budget.
- MAX31856 one-shot conversion (210 ms) dominates the control cadence.

## Documentation and Navigation

- Canonical documentation index: [`docs/README.md`](docs/README.md).
- Code navigation and change recipes: [`docs/FEATURE_MAP.md`](docs/FEATURE_MAP.md).
- Build, flash, test, and quality-gate commands: [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md).
- Baseline policy contract: [`.planning/quality/README.md`](.planning/quality/README.md).
- Historical audits and fix-round chronology: [`docs/archive/README.md`](docs/archive/README.md).

Do not duplicate command tables, pin maps, or STATUS schemas here. Put normative detail in the
owner document and link to it.

---

*Last updated: 2026-10-24. This file tracks current project state and agent-facing constraints,
not historical change logs.*
