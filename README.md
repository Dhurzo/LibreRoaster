# LibreRoaster ☕🔥

**Open Source Coffee Bean Roaster**  
Firmware written in **Rust** for the ESP32-C3

[![CI](https://github.com/Dhurzo/LibreRoaster/actions/workflows/ci.yml/badge.svg)](https://github.com/Dhurzo/LibreRoaster/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/Dhurzo/LibreRoaster/branch/develop/graph/badge.svg)](https://codecov.io/gh/Dhurzo/LibreRoaster)

LibreRoaster is ESP32-C3 firmware for a coffee roaster controller. It exposes a serial control surface that the official Artisan application can drive over **native USB CDC** (recommended — no extra hardware) or UART, reads two MAX31856 thermocouple channels, controls heater and fan outputs, and runs a safety-aware control loop built in embedded Rust.

> **New to LibreRoaster?** Use the ESP32-C3's **native USB port** to connect to Artisan. If you're building a **custom board** (like LibreRoaster), add a **10kΩ pull-up on GPIO9** for reliable boot. Official dev boards already include it. See [`docs/CONNECTION_TYPES.md`](docs/CONNECTION_TYPES.md) for why.

The project is aimed at builders who want an inspectable roasting controller rather than a closed appliance. The firmware is not a standalone roasting UI. The intended operating model is: Artisan owns the session, LibreRoaster owns the device-side control, telemetry, and safety interlocks.

**Why LibreRoaster exists →** Read the [Project Philosophy](docs/PHILOSOPHY.md).

---

## 🚧 Project Status

**v0.1 Alpha** — development in progress on the `develop` branch.

> **Latest tag:** [`v0.1`](https://github.com/Dhurzo/LibreRoaster/releases/tag/v0.1) (2026-04-30)

| Milestone | Status |
|-----------|--------|
| Firmware compiles & flashes to ESP32-C3 | ✅ Pass |
| Boot without panics, USB CDC + UART functional | ✅ Pass |
| 580 host-side unit + integration tests | ✅ All pass (incl. SSR scheduler) |
| Serial command protocol (TC4-compatible, 20+ commands) | ✅ Implemented |
| Synthetic roast curves (simulated sensors, no hardware) | ✅ Tested — full roast simulation via USB CDC |
| PID control, profiles, safety interlocks | ✅ **Implemented — 11 critical bugs fixed (see below)** |

> ✅ **All 11 critical bugs fixed** (2026-07-16). The closed-loop PID can now raise heater power beyond 5%, Artisan slider syntax (`OT1;75`, `OT2;60`, `IO3;50`, `PID;SV;250`, `UNITS;F`) is accepted, logs no longer interleave with protocol, emergency latch persists until explicit recovery, and sensor fault map matches datasheet. **Validated in simulation** (580 tests pass, 0 failures). Hardware validation with real Artisan + thermal fuse is **planned (Fase 6)**.
| Real hardware: thermocouples, heater, fan | ❌ Not yet tested |
| End-to-end roast with real Artisan | ❌ Not yet tested |
| Real coffee roasted using LibreRoaster | ❌ Not yet |

**What this means in practice:** the firmware compiles, flashes, boots, receives serial commands, and runs complete synthetic roast curves — including PID control, safety interlocks, and TC4-compatible telemetry — all tested on the host and on real ESP32-C3 hardware in simulated-sensors mode. **Hardware integration (thermocouples, heater, fan) and real Artisan connectivity have not been validated yet.** Do not connect this to a live heater without independent safety mechanisms.

## 🔮 Hardware Roadmap

### DIY Drum Roaster Build Guide 🛠️

We are working on a **complete hardware assembly guide** for a DIY drum coffee roaster compatible with LibreRoaster. The guide will include:

- 📐 Step-by-step build instructions with a full bill of materials
- 💰 Component selection focused on **minimum cost** without compromising safety
- 🔌 Plug-and-play compatibility with the LibreRoaster ESP32-C3 firmware

The guide is **currently in progress** and will be published once validated. If you want to contribute or follow along, check the issues and discussions in this repo.

---

## 📋 Current technical baseline

- **Target MCU:** ESP32-C3 (`riscv32imc-unknown-none-elf`)
- **Runtime model:** `no_std` embedded firmware on Embassy + esp-rtos
- **Primary integration:** official Artisan app over **USB CDC** (native USB port) using a TC4-compatible command set (20+ commands). UART via GPIO20/21 is also supported.
- **Sensors:** two MAX31856 thermocouple channels (Type K), mapped to ET and BT, shared SPI bus at 1 MHz. EMA-filtered readings with boot self-test.
- **Actuators:** SSR-controlled heater (5 Hz zero-cross LEDC, GPIO10) + PWM fan (25 kHz LEDC PWM, GPIO9) with slew-rate limiting and heat-source cross-check
- **Safety layers:** 8 independent layers — dual watchdog (software + hardware RWDT), over-temperature cutoff (260°C), rate-of-rise protection (30°C/min), stale-temperature guard (1s), heat-source detection (GPIO1), SSR stuck-on detection, max roast time (30 min), fault-command rejection
- **Control:** full PID with anti-windup, configurable channel (ET/BT), profile interpolation, preheat behavior
- **Simulated sensors:** synthetic roast curves for hardware-free testing on real ESP32-C3 hardware
- **In-memory telemetry:** 256-sample roast ring buffer plus live `READ` and `STATUS` responses
- **Focused controllers:** TemperatureController, HeaterController, FanController, SafetyController (v5.4)
- **Code coverage:** measured via `cargo-llvm-cov` — 71% line coverage on host test suite (target: ≥80% for production)

---

## 🧠 What the firmware actually does

LibreRoaster boots the ESP32-C3, initializes LEDC, SPI, USB CDC, UART, watchdogs, and sensor/actuator drivers, builds a `RoasterControl` instance through an application builder, and then starts a fixed async task graph.

At runtime the device does four things continuously:

1. receive commands from Artisan over USB CDC or UART,
2. parse those commands into internal control intents,
3. read temperatures and update control state,
4. emit TC4-style telemetry and internal diagnostics back to the active channel.

The important detail is that LibreRoaster is not just a thin protocol shim. It contains a full device-side control core: PID state, roast/fan profile interpolation, preheat behavior, charge detection, watchdog telemetry, and safety shutdown behavior all live inside the firmware.

---

## ⚙️ Runtime architecture

### Task topology

The embedded build starts these long-lived tasks:

- **USB reader task** — consumes raw USB CDC bytes
- **UART reader task** — consumes raw UART bytes
- **USB queue processor task** — parses USB-side commands into the shared command channel
- **UART queue processor task** — parses UART-side commands into the shared command channel
- **Control loop task** — drains commands, reads sensors, updates control, feeds watchdogs, emits telemetry
- **Dual output task** — routes formatted output to the currently active transport
- **Regression task** — handles explicit over-temperature regression runs on embedded targets

### Shared application model

The system is wired through a `ServiceContainer` (dependency injection). It owns:

- an async mutex-backed `RoasterControl` instance for task-safe access,
- the shared command/output channels,
- the command multiplexer,
- and the watchdog feeder.

`RoasterControl` is decomposed into four focused controllers (v5.4):

- **TemperatureController** — sensor reads, validation, EMA filtering, rate-of-rise monitoring
- **HeaterController** — SSR PWM with slew-rate limiting, cycle guard, heat-source cross-check
- **FanController** — PWM fan with hardware fading, emergency full-speed override
- **SafetyController** — emergency flag management, safety policy evaluation

This is the central coordination point for the firmware. If you need to understand command flow or state ownership, start there.

### Control loop structure

The control loop runs on a ~160 ms cadence (dominated by MAX31856 conversion time) and is internally instrumented in stages:

1. **command drain** with rate limiting,
2. **sensor read** (MAX31856 one-shot conversion, EMA filter),
3. **control update** (PID computation, profile interpolation, safety checks),
4. **LEDC write / actuation** (slew-rate-limited SSR, fan speed),
5. **watchdog feed** (dual-layer: software + hardware RWDT),
6. **telemetry emission** (TC4-compatible + 20-field STATUS).

The loop also records command latency, guard timeout state, watchdog health, and PID internals so automation can inspect not only roast values but also runtime health.

---

## 📡 Serial protocol surface

LibreRoaster implements a TC4-compatible serial interface with 20+ commands spanning polling, manual actuation, PID control, profiles, handshake, and diagnostics.

### Implemented command families

- **Polling:** `READ`, `STATUS`, `STAT`
- **Manual actuation:** `OT1`, `OT2`, `IO3`, `UP`, `DOWN`, `START`, `STOP`
- **PID and roast control:** `SETTARGET`, `PIDGAIN`, `PID;ON`, `PID;OFF`, `PID;SV`, `PID;T`, `PID;CHAN`, `PID;CT`, `PID;LIMIT`
- **Profiles:** `PROFILE`, `FANPROFILE`, `PREHEAT`
- **Handshake / setup:** `CHAN`, `UNITS`, `FILT`
- **Diagnostics:** `REG`, `#DUMP`

### Core response shapes

- **`READ`** returns the TC4-style line `AMB,ET,BT,0.0,0.0` when PID is off (5 fields)
- **`READ` with PID enabled** appends `heater,fan,SV` (8 fields)
- **`STATUS` / `STAT`** returns a 20-field diagnostic line with temperature, actuator, watchdog, PID, latency, emergency flags, and temperature-scale state
- **Continuous telemetry** streams `time,ET,BT,ROR,Gas` during active sessions
- **Errors** return `ERR <code> <message>` format

If you need the exact field ordering and command grammar, use the deeper protocol reference in `docs/PROTOCOL.md`.

---

## 🔌 Hardware model

LibreRoaster assumes a simple two-sensor / two-actuator hardware topology:

- **ET sensor chip select:** GPIO3
- **BT sensor chip select:** GPIO4
- **SPI clock / MOSI / MISO:** GPIO6 / GPIO7 / GPIO5
- **Fan PWM:** GPIO9
- **SSR control:** GPIO10
- **Heat detection input:** GPIO1
- **UART RX / TX:** GPIO20 / GPIO21

Two constraints matter operationally:

1. **GPIO9 is a strapping pin** — it determines boot mode at reset. **Official ESP32-C3 dev boards** (DevKitC-02, DevKitM-1, RUST-1) include the pull-up already — no extra resistor needed. **Custom boards** (like LibreRoaster) need a **10kΩ pull-up to 3.3V**. See [`docs/CONNECTION_TYPES.md`](docs/CONNECTION_TYPES.md) for the full breakdown.
2. **SPI MISO is routed through GPIO5 instead of GPIO2** to avoid the ESP32-C3 strap conflict on FSPIQ.

For electrical and timing notes see [`docs/HARDWARE.md`](docs/HARDWARE.md).

---

## 🔒 Known architectural constraints

These are not marketing notes. They are the design boundaries readers should understand before modifying the system.

- **No persistence layer:** roast state, telemetry buffer, and profiles are RAM-only
- **Host/embedded split:** the real application exists only under the `embedded` feature; host builds are primarily for tests (under `test` feature)
- **Command queue limits:** the main command channel is intentionally small and rate-limited
- **Sensor timing pressure:** MAX31856 conversion time (~160 ms) dominates the control loop cadence; stale-data protection is critical

---

## 🔨 Build and verification model

### Prerequisites

- Rust stable toolchain (`rustup default stable`)
- `rust-src` component: `rustup component add rust-src`
- ESP32-C3 target: `rustup target add riscv32imc-unknown-none-elf`
- `cargo install espflash` (for flashing)
- `cargo install cargo-generate` (optional, for project templates)

### Host verification

Integration-style host tests depend on the `test` feature (enables the host-side Embassy time driver). **265 unit tests + ~139 integration tests** run on x86_64:

```bash
cargo test --target x86_64-unknown-linux-gnu --features test
```

### CI pipeline

GitHub Actions runs 6 parallel jobs on every push/PR to `develop` and `main`:

| Job | Command |
|-----|---------|
| Format | `cargo fmt --all -- --check` |
| Clippy | `cargo clippy --locked --all-targets` |
| Host tests | `cargo test --target x86_64-unknown-linux-gnu --features test --lib --tests` |
| Regression tests | `cargo test --features "test,regression" --target x86_64-unknown-linux-gnu` |
| Code coverage | `cargo llvm-cov --target x86_64-unknown-linux-gnu --features test --lcov` |
| Embedded build | `cargo build --release --target riscv32imc-unknown-none-elf --features embedded` |

### Embedded build & flash

```bash
# Build (real sensors)
cargo build --release --target riscv32imc-unknown-none-elf --features embedded

# Flash
cargo espflash flash --release --target riscv32imc-unknown-none-elf \
  --features embedded --port /dev/ttyACM0

# Flash + monitor
cargo espflash flash --release --target riscv32imc-unknown-none-elf \
  --features embedded --monitor

# Monitor only (already flashed)
cargo espflash monitor --port /dev/ttyACM0 --speed 115200
```

> **Flash tip:** Use the ESP32-C3's **native USB port** (`/dev/ttyACM0`). For reliable boot on **custom boards**, add a **10kΩ pull-up from GPIO9 to 3.3V**. Official dev boards already include this pull-up. See [`docs/CONNECTION_TYPES.md`](docs/CONNECTION_TYPES.md).

### Simulated sensors mode

Build and flash with the `simulated-sensors` feature to run on a bare ESP32-C3 **without any thermocouples or actuators connected**. The firmware generates synthetic temperature readings from a configurable roast curve and feeds them through the entire control stack — PID, safety, telemetry, and Artisan serial protocol — exactly as if real sensors were connected.

Useful for:
- Verifying serial connectivity over USB CDC or UART
- Validating PID tuning against a known temperature trajectory
- End-to-end firmware regression without hardware risk
- Demonstrating the roaster control surface without a physical machine

```bash
cargo build --release --target riscv32imc-unknown-none-elf \
  --features "embedded,simulated-sensors"

cargo espflash flash --release --target riscv32imc-unknown-none-elf \
  --features "embedded,simulated-sensors" --port /dev/ttyACM0
```

See [`docs/simulated-curve-test.md`](docs/simulated-curve-test.md) for curve presets, noise injection, and the full architecture.

### Additional quality checks

```bash
scripts/quality-baseline.sh
scripts/run-regression-checks.sh
```

The development guide in [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) explains the feature matrix, the host-vs-embedded split, and the recommended verification workflow in more detail.

---

## 📚 Technical documentation map

The main technical documents are:

- **`docs/PHILOSOPHY.md`** — project goals, design principles, and hardware vision
- **`docs/ARCHITECTURE.md`** — runtime architecture, task graph, state ownership, timing, safety invariants
- **`docs/PROTOCOL.md`** — command grammar, responses, telemetry fields, compatibility boundaries
- **`docs/HARDWARE.md`** — pins, buses, PWM topology, electrical constraints, implementation notes
- **`docs/CONNECTION_TYPES.md`** — USB vs UART: which connection to use, why USB boots reliably without extra hardware
- **`docs/ARTISAN_CONNECTION.md`** — how the official Artisan app should be configured against LibreRoaster
- **`docs/DEVELOPMENT.md`** — build, flash, test, and quality workflow
- **`docs/INSTRUMENTATION.md`** — deep explanation of the 20-field status line and internal diagnostics
- **`docs/TESTING.md`** — test types, coverage, status, and known gaps across all test layers
- **`docs/simulated-curve-test.md`** — simulated sensor curve presets, noise injection, and architecture
- **`docs/decisions/`** — Architecture Decision Records (ADRs) for key design choices
- **`docs/pinout.md`** — pin mapping reference
- **`CHANGELOG.md`** — release history and notable changes per version
- **`SECURITY.md`** — supported versions, vulnerability reporting, and disclosure policy
- **`.github/dependabot.yml`** — automated dependency updates (Cargo weekly, Actions monthly)
- **`.github/workflows/release.yml`** — release build and GitHub Release automation (tag `v*`)

---

## ⚠️ Safety Warning

**This project involves serious safety risks.**

LibreRoaster is firmware for high-temperature, mains-adjacent hardware. It works with:

- ⚡ **High voltages**
- 🔥 **Very high temperatures**

Improper handling can result in **severe injury, fire, or death**.

### Please follow these precautions:

- Do not treat a passing build or test suite as proof of electrical safety.
- Do not run the roaster unattended.
- Do not modify the power stage without appropriate electrical competence.
- Always disconnect power before modifying or servicing the device.
- Use appropriate **thermal insulation and heat-resistant materials**.
- Keep a **fire extinguisher nearby at all times** when using the roaster.
- Operate the roaster in a **well-ventilated and fire-safe area**.
- Treat watchdog, thermal cutoff, and heat-detection logic as last-resort mitigations, not as substitutes for safe hardware design.

> ⚠️ You build and use this project **at your own risk**.  
> The authors and contributors are **not responsible** for any damage, injury, or loss.

---

## 📜 License

This project is open source under the **Apache 2.0** license.  
See the `LICENSE` file for more information.
