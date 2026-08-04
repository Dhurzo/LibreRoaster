# Simulated Sensor Curves

**Last updated:** 2026-08-04

## Overview

LibreRoaster can run on real ESP32-C3 hardware **without any thermocouples connected** by enabling the `simulated-sensors` Cargo feature. Instead of reading real MAX31856 chips over SPI, the firmware generates synthetic temperature readings from a configurable roast curve and feeds them through the entire control stack: PID, safety guards, telemetry, and Artisan serial protocol.

This means you can verify the full firmware behaviour — command parsing, PID control, safety shutdown, STATUS telemetry — on a bare ESP32-C3 dev board with no external sensors, no heater, and no fan.

## When to use this

- Bring-up testing on a new ESP32-C3 board before wiring thermocouples
- Verifying Artisan serial connectivity over USB CDC or UART
- Validating PID tuning logic against a known temperature trajectory
- End-to-end firmware regression without hardware risk
- Demonstrating the roaster control surface without a physical machine

## Build and flash

```bash
cargo build --release --target riscv32imc-unknown-none-elf \
  --features "embedded,simulated-sensors"

cargo espflash flash --release --target riscv32imc-unknown-none-elf \
  --features "embedded,simulated-sensors"
```

The firmware boots normally, initialises LEDC, UART, USB CDC, watchdog, and SSR/fan drivers. The only difference is that SPI and MAX31856 initialisation is skipped entirely, and temperature readings come from the curve generator instead.

To build with real sensors (the default):

```bash
cargo build --release --target riscv32imc-unknown-none-elf --features embedded
```

## How it works

### Architecture

The feature flag controls which backend `SensorConversionHub` uses at compile time:

| Configuration | Sensor backend | What happens at runtime |
|---|---|---|
| `embedded` (default) | Real MAX31856 over SPI | SPI init → one-shot conversions → fault detection → fallback logic |
| `embedded,simulated-sensors` | `SimulatedSensorSource` | Curve lookup → interpolation → optional noise → clean `SensorSample` |
| Host tests (`test` feature) | Stub returning 0.0 °C | No real hardware involved |

When `simulated-sensors` is active:

1. `init_hardware()` in `src/hardware/init.rs` skips SPI bus creation, GPIO3/GPIO4 (CS pins), and MAX31856 driver initialisation entirely. LEDC, SSR, fan, UART, USB CDC, and watchdog still initialise normally.
2. `AppBuilder` calls `with_simulated_sensors()` instead of `with_temperature_sensors()`, which constructs a `SensorConversionHub` backed by a `SimulatedSensorSource`.
3. The control loop calls `SensorConversionHub::sample()` as usual. Internally it reads from the curve generator instead of triggering SPI transactions.
4. The generated `SensorSample` is structurally identical to a real one — same fields, same types, same fault semantics (no faults in simulated mode). Everything downstream (PID, safety, telemetry) works unmodified.

### Curve model

A roast curve is a sequence of **waypoints**, each defining temperatures at a point in time:

```rust
pub struct CurvePoint {
    pub time_secs: u32,   // elapsed seconds since boot
    pub bean_temp: f32,   // °C
    pub env_temp: f32,    // °C
}
```

The generator **linearly interpolates** between consecutive waypoints to produce smooth temperature trajectories at the control-loop cadence (~3 Hz: real tick ≈ 310–330 ms because every tick waits on the MAX31856 conversion). Before the first waypoint, temperatures hold at that waypoint's values. After the last waypoint, they hold at the final values.

Maximum curve length: **32 waypoints** (heapless, no heap allocation).

### Default curve

The built-in default curve (`RoastCurve::default_medium_roast()`) models a typical 10-minute medium roast:

| Phase | Time (s) | BT (°C) | ET (°C) |
|---|---:|---:|---:|
| Ambient start | 0 | 25 | 25 |
| Pre-heat ramp | 30 | 80 | 100 |
| Charge entry | 60 | 120 | 150 |
| Drying | 120–240 | 150–190 | 180–220 |
| Maillard | 240–420 | 190–215 | 220–240 |
| First crack | 420–540 | 215–225 | 240–250 |
| Development hold | 540–600 | 225 | 250 |

The curve stays below the over-temperature threshold (260 °C) to avoid triggering the safety shutdown during normal simulation.

### Noise

By default, the simulated signal is perfectly clean (no jitter). You can add deterministic triangle-wave noise to simulate realistic sensor behaviour:

```rust
let source = SimulatedSensorSource::default_curve()
    .with_noise_amplitude(0.5);  // ±0.5 °C
```

The noise is deterministic (same input → same output) so tests remain reproducible. It uses a triangle wave with a period of ~2 seconds, offset between bean and env channels so they don't correlate perfectly.

## Custom curves

You can define a custom roast curve by building a `RoastCurve` with your own waypoints:

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

The simulated sensor source produces valid, fault-free readings at every tick. This means:

- **Stale-temperature guard** (1-second timeout) will not trigger — the curve generator produces a fresh reading on every `sample()` call.
- **Over-temperature cutoff** (260 °C) will only trigger if the curve includes values at or above that threshold. The default medium roast curve stays below 260 °C.
- **Consecutive-fallback guard** (5 failures) will never trigger — there are no read failures in simulated mode.
- **PID and heater control** work normally because temperatures are finite and valid.
- **Heat-source detection** (GPIO1) still reads the real pin. If nothing is connected, the pin reads high (pull-up) and SSR status reports `NotDetected`, which forces heater output to 0%. To test heater output, GPIO1 must be pulled low externally (or the SSR mock can be extended).

## What is and isn't tested

### Tested (exercised by simulated curves)

- Artisan serial protocol (READ, STATUS, OT1, IO3, START, STOP, PID commands)
- PID control loop computation and output
- Stale-temperature and over-temperature safety guards
- Control loop cadence and timing
- Watchdog feeding
- Telemetry emission and STATUS field formatting
- Command parsing and multiplexing over USB CDC and UART
- Temperature scale conversion (Celsius/Fahrenheit)
- Profile and fan-profile interpolation

### Not tested (requires real hardware)

- MAX31856 SPI communication and fault detection
- Thermocouple signal integrity and noise rejection
- Actual SSR switching behaviour
- Fan PWM frequency and acoustic output
- Heat-source detection GPIO response
- Real thermal dynamics (thermal inertia, heat transfer delays)

## Source files

| File | Purpose |
|---|---|
| `src/hardware/sensors/simulated.rs` | `RoastCurve`, `CurvePoint`, `SimulatedSensorSource` — curve model, interpolation, noise |
| `src/hardware/sensors/conversion.rs` | `SensorConversionHub` — conditional real/simulated/host backend |
| `src/hardware/sensors/mod.rs` | Module exports |
| `src/hardware/init.rs` | Hardware init — SPI skipped when `simulated-sensors` is active |
| `src/application/app_builder.rs` | `with_simulated_sensors()` builder method |
| `src/main.rs` | Conditional build path for simulated vs real sensors |
| `Cargo.toml` | `simulated-sensors` feature definition |

## Host-side tests

The simulated curve module includes 18 unit tests covering edge cases (empty curves, single-point curves, interpolation, boundary conditions, overflow, noise). These run on the host target:

```bash
cargo test --target x86_64-unknown-linux-gnu --features test simulated
```

## Related documentation

- `docs/ARCHITECTURE.md` — Runtime architecture and task topology
- `docs/DEVELOPMENT.md` — Build, flash, and test workflows
- `docs/HARDWARE.md` — Pin map and hardware constraints
- `docs/PROTOCOL.md` — Artisan serial command reference
