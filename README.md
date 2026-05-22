# LibreRoaster ☕🔥

**Open Source Coffee Bean Roaster**  
Firmware written in **Rust** for the ESP32-C3

LibreRoaster is ESP32-C3 firmware for a coffee roaster controller. It exposes a serial control surface that the official Artisan application can drive over native USB CDC or UART, reads two MAX31856 thermocouple channels, controls heater and fan outputs, and runs a safety-aware control loop built in embedded Rust.

The project is aimed at builders who want an inspectable roasting controller rather than a closed appliance. The firmware is not a standalone roasting UI. The intended operating model is: Artisan owns the session, LibreRoaster owns the device-side control, telemetry, and safety interlocks.

**Why LibreRoaster exists →** Read the [Project Philosophy](docs/PHILOSOPHY.md).

---

## 🚧 Project Status

**Alpha — hardware bring-up in progress.**

| Milestone | Status |
|-----------|--------|
| Firmware compiles & flashes to ESP32-C3 | ✅ Pass |
| Boot without panics, USB CDC log output functional | ✅ Pass |
| 380+ host-side unit & integration tests | ✅ Pass |
| Thermocouple reads via MAX31856 (SPI) | ⚠️ Partial — data received but intermittent corruption / decode errors |
| End-to-end roast session controlled by Artisan | ❌ Not tested |
| Real coffee roasted using LibreRoaster | ❌ Not yet |

**What this means in practice:** the firmware boots, talks over serial, and reads sensor data — but the thermocouple pipeline still shows noise or framing errors under real conditions. **No roast has been performed using LibreRoaster.** Do not connect this to a live heater without independent safety mechanisms.

For the latest work-in-progress code, check the **`develop`** branch.

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
- **Primary integration:** official Artisan app over serial using a TC4-style command set
- **Sensors:** two MAX31856 thermocouple channels, mapped to ET and BT
- **Actuators:** SSR-controlled heater plus PWM fan output
- **Safety layers:** over-temperature cutoff, watchdog, stale-temperature protection, heat-source detection, LEDC guard
- **In-memory telemetry:** 256-sample roast ring buffer plus live `READ` and `STATUS` responses

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

The system is wired through a singleton `ServiceContainer`. It owns:

- an async mutex-backed `RoasterControl` instance for task-safe access,
- a sync mirror used by older critical-section paths,
- the shared command/output channels,
- the command multiplexer,
- and the watchdog feeder.

This is the central coordination point for the firmware. If you need to understand command flow or state ownership, start there.

### Control loop structure

The control loop runs on a 100 ms cadence and is internally instrumented in stages:

1. **command drain** with rate limiting,
2. **sensor read**,
3. **control update**,
4. **LEDC write / actuation**,
5. **watchdog feed**,
6. **telemetry emission**.

The loop also records command latency, guard timeout state, watchdog health, and PID internals so automation can inspect not only roast values but also runtime health.

---

## 📡 Serial protocol surface

LibreRoaster implements a TC4-oriented serial interface rather than the full breadth of Artisan's device ecosystem.

### Implemented command families

- **Polling:** `READ`, `STATUS`, `STAT`
- **Manual actuation:** `OT1`, `OT2`, `IO3`, `UP`, `DOWN`, `STOP`
- **PID and roast control:** `START`, `SETTARGET`, `PIDGAIN`, `PID;ON`, `PID;OFF`, `PID;SV`, `PID;T`, `PID;CHAN`, `PID;CT`, `PID;LIMIT`
- **Profiles:** `PROFILE`, `FANPROFILE`, `PREHEAT`
- **Handshake / setup:** `CHAN`, `UNITS`, `FILT`
- **Diagnostics:** `REG`, `#DUMP`

### Core response shapes

- **`READ`** returns the TC4-style line `AMB,ET,BT,0.0,0.0` when PID is off
- **`READ` with PID enabled** appends `heater,fan,SV`
- **`STATUS` / `STAT`** returns a 19-field diagnostic line with temperature, actuator, watchdog, PID, latency, and temperature-scale state

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

1. **GPIO9 is a strapping pin**, so the external fan stage must not force an invalid boot state.
2. **SPI MISO is routed through GPIO5 instead of GPIO2** to avoid the ESP32-C3 strap conflict on FSPIQ.

The complete electrical and timing notes live in `docs/HARDWARE.md`.

---

## 🔒 Known architectural constraints

These are not marketing notes. They are the design boundaries readers should understand before modifying the system.

- **No persistence layer:** roast state, telemetry buffer, and profiles are RAM-only
- **Host/embedded split:** the real application exists only under the `embedded` feature; host builds are primarily for tests
- **Command queue limits:** the main command channel is intentionally small and rate-limited
- **Sensor timing pressure:** thermocouple reads are slow relative to the nominal PID sample cadence, so stale-data protection matters

---

## 🔨 Build and verification model

### Embedded build (real sensors)

```bash
cargo build --release --target riscv32imc-unknown-none-elf --features embedded
```

### Simulated sensors mode

Build and flash with the `simulated-sensors` feature to run on a bare ESP32-C3 **without any thermocouples or actuators connected**. The firmware generates synthetic temperature readings from a configurable roast curve and feeds them through the entire control stack — PID, safety, telemetry, and Artisan serial protocol — exactly as if real sensors were connected.

Useful for:
- Verifying Artisan serial connectivity over USB CDC or UART
- Validating PID tuning against a known temperature trajectory
- End-to-end firmware regression without hardware risk
- Demonstrating the roaster control surface without a physical machine

```bash
cargo build --release --target riscv32imc-unknown-none-elf \
  --features "embedded,simulated-sensors"

cargo espflash flash --release --target riscv32imc-unknown-none-elf \
  --features "embedded,simulated-sensors"
```

See [`docs/simulated-curve-test.md`](docs/simulated-curve-test.md) for curve presets, noise injection, and the full architecture.

### Flash (real sensors)

```bash
cargo espflash flash --release --target riscv32imc-unknown-none-elf --features embedded
```

### Host verification

Integration-style host tests depend on the `test` feature because that enables the host-side Embassy time driver:

```bash
cargo test --target x86_64-unknown-linux-gnu --features test
```

Additional quality gates:

```bash
scripts/quality-baseline.sh
scripts/run-regression-checks.sh
```

The development guide in `docs/DEVELOPMENT.md` explains the feature matrix, the host-vs-embedded split, and the recommended verification workflow in more detail.

---

## 📚 Technical documentation map

The main technical documents are:

- **`docs/PHILOSOPHY.md`** — project goals, design principles, and hardware vision
- **`docs/ARCHITECTURE.md`** — runtime architecture, task graph, state ownership, timing, safety invariants
- **`docs/PROTOCOL.md`** — command grammar, responses, telemetry fields, compatibility boundaries
- **`docs/HARDWARE.md`** — pins, buses, PWM topology, electrical constraints, implementation notes
- **`docs/ARTISAN_CONNECTION.md`** — how the official Artisan app should be configured against LibreRoaster
- **`docs/DEVELOPMENT.md`** — build, flash, test, and quality workflow
- **`docs/INSTRUMENTATION_README.MD`** — deep explanation of the 19-field status line and internal diagnostics
- **`docs/TESTING.md`** — test types, coverage, status, and known gaps across all test layers
- **`docs/BUGS.md`** — current technical risk report and likely defect inventory
- **`docs/ARTISAN_COMPATIBILITY_REPORT.md`** — compatibility assessment against the official Artisan application

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
