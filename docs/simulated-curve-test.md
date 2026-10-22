# Simulated Sensor Curves

**Last updated:** 2026-10-22

## Overview

LibreRoaster can run on real ESP32-C3 hardware **without thermocouples
connected** by enabling the `simulated-sensors` Cargo feature. Instead of
reading MAX31856 chips over SPI, the firmware generates synthetic BT/ET
readings from a roast curve and feeds them through the normal control stack:
PID, safety guards, telemetry, and the Artisan serial protocol.

This lets you validate command parsing, PID control, safety shutdown, and
STATUS telemetry on a bare board with no external sensors, heater, or fan. It
does **not** exercise the real MAX31856 SPI path; use Wokwi F2 for that.

## When to use this

- Bring-up on a new ESP32-C3 board before wiring thermocouples.
- Verifying Artisan serial connectivity over USB CDC or UART.
- Validating PID behavior against a known temperature trajectory.
- End-to-end firmware regression without hardware risk.
- Demonstrating the roaster control surface without a physical machine.

## Quick reference

| Goal | Build/feature | Curve |
|---|---|---|
| Board/UART/PWM bring-up | `embedded,simulated-sensors` | Medium (`default_medium_roast`) |
| Guard-safe `START`/`Heating` | `embedded,simulated-typical-roast` | Typical (`typical_roast`) |
| Light-roast work | No Cargo feature; call `light_roast()` from code/tests | Light |
| Custom trajectory | `SimulatedSensorSource::new(custom_curve)` | Custom `RoastCurve` |

Curve selection is compile-time. There is no serial command or runtime switch.
See “Switching curves” below.

## Build and flash

Only the feature set changes. Use the canonical build and flash commands in
[`DEVELOPMENT.md`](DEVELOPMENT.md) §4–§5, adding `simulated-sensors` to the
`embedded` feature set:

```text
--features "embedded,simulated-sensors"
```

The firmware boots normally and initializes LEDC, UART, USB CDC, watchdog, and
the SSR/fan drivers. SPI and MAX31856 initialization are skipped, and
temperature readings come from the curve generator.

The real-sensor build is the plain `embedded` feature set.

## How it works

### Architecture

The feature flag selects the `SensorConversionHub` backend at compile time:

| Configuration | Sensor backend | Runtime path |
|---|---|---|
| `embedded` (default) | Real MAX31856 over SPI | SPI init → one-shot conversions → fault detection → fallback |
| `embedded,simulated-sensors` | `SimulatedSensorSource` | Curve lookup → interpolation → optional noise → clean `SensorSample` |
| Host tests (`test` feature) | Stub returning 0.0 °C | No hardware involved |

When `simulated-sensors` is active:

1. `init_hardware()` in `src/hardware/init.rs` skips the SPI bus, GPIO3/GPIO4
   chip selects, and MAX31856 driver initialization. LEDC, SSR, fan, UART,
   USB CDC, and the watchdog still initialize normally.
2. `AppBuilder` calls `with_simulated_sensors()` instead of
   `with_temperature_sensors()`, constructing a `SensorConversionHub` backed
   by `SimulatedSensorSource`.
3. The control loop calls `SensorConversionHub::sample()` as usual. The hub
   reads the curve generator instead of triggering SPI transactions.
4. The generated `SensorSample` has the same fields, types, and fault semantics
   as a real sample (there are no faults in simulated mode). PID, safety, and
   telemetry run unmodified.

### Curve model

A curve is a sequence of **waypoints**, each defining temperatures at a point
in time:

```rust
pub struct CurvePoint {
    pub time_secs: u32,   // elapsed seconds since boot
    pub bean_temp: f32,   // °C
    pub env_temp: f32,    // °C
}
```

The generator linearly interpolates between consecutive waypoints at the
control-loop cadence. The simulated path has no MAX31856 conversion wait, so it
ticks at roughly 10 Hz with the 100 ms loop timer; real-hardware builds tick at
roughly 310–330 ms because of that wait.

Before the first waypoint, temperatures hold at that waypoint's values. After
the last waypoint, they hold at the final values. Maximum curve length is
**32 waypoints** (heapless, no heap allocation).

## Built-in curves

### Medium: `default_medium_roast()`

The default of `embedded,simulated-sensors`. It models a typical 10-minute
medium roast:

| Phase | Time (s) | BT (°C) | ET (°C) |
|---|---:|---:|---:|
| Ambient start | 0 | 25 | 25 |
| Pre-heat ramp | 30 | 80 | 100 |
| Charge entry | 60 | 120 | 150 |
| Drying | 120–240 | 150–190 | 180–220 |
| Maillard | 240–420 | 190–215 | 220–240 |
| First crack | 420–540 | 215–225 | 240–250 |
| Development hold | 540–600 | 225 | 250 |

The curve stays below the 260 °C over-temperature threshold.

> **Rate-of-rise interaction:** this is a time-based profile, not a closed-loop
> plant. Its early ramp (up to ~1.8 °C/s) is steeper than the RoR guard
> (0.75 °C/s soft / 1.0 °C/s hard). Sending `START` against this curve enters
> Heating, arms the guard, and correctly latches
> `Bean temperature rate-of-rise exceeded`. Use `PREHEAT` and `OT1` for
> LED/actuator checks, use the typical curve below for `START`, or define a
> custom curve with a slope below 0.75 °C/s.

### Typical: `typical_roast()`

Selected by `simulated-typical-roast`, which implies `simulated-sensors`. It
models a charge-dip roast whose positive BT slopes stay below the soft RoR
limit (at most ~0.33 °C/s):

```bash
cargo build --release --target riscv32imc-unknown-none-elf \
  --features "embedded,simulated-typical-roast"
```

For Wokwi F3:

```bash
FEATURES=embedded,simulated-typical-roast ./scripts/sim-wokwi-prepare.sh
```

| Phase | Time (s) | BT (°C) | ET (°C) |
|---|---:|---:|---:|
| Charge | 0 | 200 | 220 |
| Turning point | 60 | 100 | 205 |
| Drying | 120–300 | 110–168 | 210–225 |
| Maillard / first crack | 360–420 | 184–196 | 230–234 |
| Development | 420–600 | 196–214 | 234–243 |

The initial negative slope cannot trip the RoR guard. The subsequent positive
slopes stay below 0.75 °C/s, so `START` remains in `Heating` with the solid
green LED. The curve is time-based from source construction, so the first
`READ` after a multi-second boot already shows part of the charge dip. See
[`SIMULATION.md`](SIMULATION.md) §3.3 for the Wokwi checklist.

### Light: `light_roast()`

A light-roast trajectory used by host-side regression work. It has no Cargo
feature; call it from code or tests. See the source module for the exact
waypoints and the light-roast regression suite.

## Switching curves

`SimulatedSensorSource::default_curve()` selects the curve at compile time:

| Curve | How to select it | Typical use |
|---|---|---|
| Medium (`default_medium_roast`) | `--features "embedded,simulated-sensors"` | F1 bring-up; `READ`/`PREHEAT`/`OT1`; `START` correctly latches the RoR guard |
| Typical (`typical_roast`) | `--features "embedded,simulated-typical-roast"` | F3 guard-safe `START`/`Heating` |
| Light (`light_roast`) | No Cargo feature; call it from code or a host test | Light-roast regression work |
| Custom | Build a `RoastCurve` and pass it to `SimulatedSensorSource::new` | Custom trajectories |

There is no runtime curve selector and no need to rebuild after changing Wokwi
MAX31856 sliders (F2 has no synthetic curve). The Wokwi run commands are in
[`SIMULATION.md`](SIMULATION.md) §3.4.1.

## Noise

By default the simulated signal is clean. You can add deterministic
triangle-wave noise to model sensor jitter:

```rust
let source = SimulatedSensorSource::default_curve()
    .with_noise_amplitude(0.5);  // ±0.5 °C
```

The noise is deterministic (same input → same output), so tests remain
reproducible. It uses a triangle wave with a period of about 2 s, offset
between the bean and environment channels so they do not correlate perfectly.

## Custom curves

Build a `RoastCurve` with your own waypoints:

```rust
use libreroaster::hardware::sensors::{CurvePoint, RoastCurve, SimulatedSensorSource};

let mut curve = RoastCurve::new();
curve.add_point(CurvePoint { time_secs: 0,   bean_temp: 25.0,  env_temp: 25.0 });
curve.add_point(CurvePoint { time_secs: 120, bean_temp: 150.0, env_temp: 180.0 });
curve.add_point(CurvePoint { time_secs: 300, bean_temp: 200.0, env_temp: 230.0 });
curve.add_point(CurvePoint { time_secs: 600, bean_temp: 230.0, env_temp: 255.0 });

let source = SimulatedSensorSource::new(curve);
```

Points must be added in ascending `time_secs` order.

## Safety considerations

The simulated source produces valid, fault-free readings every tick:

- **Stale-temperature guard** (1 s) does not trigger; the generator returns a
  fresh reading on every `sample()`.
- **Over-temperature cutoff** (260 °C) triggers only if the curve includes
  values at or above that threshold. The medium curve stays below it.
- **Consecutive-fallback guard** (5 failures) never triggers; simulated mode
  has no read failures.
- **PID and heater control** work normally because temperatures are finite and
  valid.
- **Heat-source detection** (GPIO1) is a no-op: the pin-interpretation path is
  gated on `heat-sense` **without** `simulated-sensors`
  (`src/hardware/ssr_logic.rs`). A simulated build never reports `NotDetected`
  and never forces heater output to 0 %. On real hardware, only the opt-in
  `heat-sense` build interprets GPIO1, and only above ≈50 % duty.

## Coverage: what this mode proves

### Exercised by simulated curves

- Artisan serial protocol (`READ`, `STATUS`, `OT1`, `IO3`, `START`, `STOP`,
  PID commands).
- PID computation and heater output.
- Stale-temperature and over-temperature safety guards.
- Control-loop cadence and timing.
- Watchdog feeding.
- Telemetry emission and `STATUS` field formatting.
- Command parsing and multiplexing over USB CDC and UART.
- Celsius/Fahrenheit conversion.
- Profile and fan-profile interpolation.

### Not exercised by this mode

- MAX31856 SPI communication and fault detection (Wokwi F2 covers the SPI
  path).
- Thermocouple signal integrity and noise rejection.
- Actual SSR switching behavior.
- Fan PWM frequency and acoustic output.
- Heat-source detection GPIO response (against real current sensing).
- Real thermal dynamics: thermal inertia, heat transfer delays, and plant
  feedback.

## Source files

| File | Purpose |
|---|---|
| `src/hardware/sensors/simulated.rs` | `RoastCurve`, `CurvePoint`, `SimulatedSensorSource` — curve model, interpolation, noise |
| `src/hardware/sensors/conversion.rs` | `SensorConversionHub` — real/simulated/host backend selection |
| `src/hardware/sensors/mod.rs` | Module exports |
| `src/hardware/init.rs` | Hardware init — SPI skipped when `simulated-sensors` is active |
| `src/application/app_builder.rs` | `with_simulated_sensors()` builder method |
| `src/main.rs` | Conditional path for simulated vs real sensors |
| `Cargo.toml` | `simulated-sensors` and `simulated-typical-roast` features |

## Host-side tests

The simulated curve module includes 23 tests under `simulated-sensors`
(21 unit + 2 proptest properties). Enabling `simulated-typical-roast` adds one
test for default-curve selection (24 total). Coverage includes
empty/single-point/zero-range curves, interpolation boundaries, the
32-waypoint cap, the BUG-2b-1 exact-segment-end case, and hostile-input
properties. The module compiles only with `simulated-sensors`, so the feature
is required on the host:

```bash
cargo test --target x86_64-unknown-linux-gnu \
  --features test,simulated-sensors \
  --lib hardware::sensors::simulated
```

## Hardware validation scripts (no sensors needed)

Two scripts exercise the simulated curve on a real ESP32-C3 over the Artisan
serial protocol. Flash the board with
`--features "embedded,simulated-sensors"` first. Both follow the repo HIL line
format (`TEST:<name>:<PASS|FAIL>:<detail>`, `TESTSUITE:COMPLETE:x/y:<status>`) and exit 0
only when every check passes. Last full hardware pass: 2026-09-14 (virtual
roast 15/15, soak 12/12).

| Script | What it proves on hardware | Runtime |
|---|---|---|
| `scripts/hw_virtual_roast.py` | Full roast lifecycle (idle → preheat → roast → stop → recovered idle); PROFILE/FANPROFILE interpolation follows roast time; STOP operator path (heater cut, fan 100 %, latch) + `PID;OFF` recovery | ~2 min |
| `scripts/hw_soak.py` | 1 Hz `#` stream integrity over the default curve: format, cadence, monotonic timestamps, no glitches, full BT span, no `safety_fault`, device responsive afterward | ~12 min (660 s capture) |

```bash
python3 scripts/hw_virtual_roast.py --port /dev/ttyUSB0
python3 scripts/hw_soak.py --port /dev/ttyUSB0 --duration 660
```

Two details the scripts encode because they are easy to get wrong by hand:

- **Session keepalive.** In Idle, the firmware drops the active transport after
  60 s without commands (`CommandMultiplexer::IDLE_TIMEOUT_SECS`), which stops
  telemetry routing even with `STREAM;ON`. When the heater is energized or a
  roast is active, the safety `COMMS_IDLE_TIMEOUT_MS` (15 s) latches first, so
  send `READ` or `STATUS` every 5–10 s. `hw_soak.py` sends `STATUS` every 25 s
  because it observes the curve in Idle.
- **Single UART cable.** With the native-USB cable plugged, `esp-println`'s
  `auto` printer may route lines to the unread USB side once the host
  enumerates it. Capture HIL output on UART0 with native USB unplugged. See
  [`DEVELOPMENT.md`](DEVELOPMENT.md) §9.

Soak raw captures are saved under `logs/` (git-ignored) for forensics.

## Related documentation

- [`ARCHITECTURE.md`](ARCHITECTURE.md) — runtime architecture and task
  topology.
- [`DEVELOPMENT.md`](DEVELOPMENT.md) — build, flash, and test workflows.
- [`HARDWARE.md`](HARDWARE.md) — pin map and hardware constraints.
- [`PROTOCOL.md`](PROTOCOL.md) — Artisan serial command reference.
- [`SIMULATION.md`](SIMULATION.md) — Wokwi no-hardware simulation and scripted
  scenarios.
