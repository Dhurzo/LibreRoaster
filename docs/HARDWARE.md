# LibreRoaster Hardware Guide

**Last updated:** 2026-10-24 — pin map, PWM topology, and electrical limits reviewed against `src/config/constants.rs` and `src/hardware/init.rs`.

This document owns the firmware-facing hardware model: the pin mapping the firmware expects, the
subsystem topology, and the electrical constraints that shape the safety case. Physical assembly,
connector-level wiring, and board-damage prevention live in [`pinout.md`](pinout.md). Host-side
USB/UART setup lives in [`CONNECTION_TYPES.md`](CONNECTION_TYPES.md) and
[`ARTISAN_CONNECTION.md`](ARTISAN_CONNECTION.md).

---

## 1. Hardware Role Split

LibreRoaster assumes four hardware responsibilities:

- **compute and transport** on the ESP32-C3;
- **temperature acquisition** through two MAX31856 thermocouple front ends;
- **heater switching** through an SSR-controlled output;
- **airflow control** through a PWM-driven fan stage.

The firmware is opinionated about this topology. It is adaptable, but not dynamically
auto-discovered.

## 2. Pin Map Used by the Firmware

The constants in `src/config/constants.rs` define the effective mapping; boot-time assertions in
`src/hardware/init.rs` enforce it.

| Signal | GPIO | Notes |
|---|---:|---|
| Heat detection input | 1 | Input, internal pull-up; LOW = SSR conducting |
| ET thermocouple chip select | 3 | Output, shared SPI bus |
| BT thermocouple chip select | 4 | Output, shared SPI bus |
| SPI MISO | 5 | Input, routed through the GPIO matrix to avoid the GPIO2 strap |
| SPI SCLK | 6 | Output, FSPI clock |
| SPI MOSI | 7 | Output, FSPI data out |
| Status LED | 8 | Output, push-pull, active-high; not sampled for normal boot |
| Fan PWM | 9 | Output, LEDC 25 kHz; **strapping pin** — external 10 kΩ → 3.3 V mandatory on custom boards |
| SSR control PWM | 10 | Output, LEDC 5 Hz zero-cross, 14-bit; external 10 kΩ pull-down mandatory |
| UART RX | 20 | Input, 3.3 V only, 115200 baud |
| UART TX | 21 | Output, 3.3 V only, 115200 baud |

USB D+/D− is internal to the ESP32-C3 (native USB CDC). It has no external GPIO.

For the full connector-level wiring diagram, subsystem images, and assembly order, see
[`pinout.md`](pinout.md).

## 3. Why the SPI Pins Look Unusual

The ESP32-C3 nominal FSPI MISO maps to GPIO2, but GPIO2 is a strapping pin. LibreRoaster avoids
that boot-risk path and routes MISO through GPIO5 via the GPIO matrix.

That choice is a boot-reliability measure, not a cosmetic preference.

## 4. Thermocouple Subsystem

LibreRoaster expects two MAX31856 devices on one SPI controller with separate chip selects:

- **BT**: bean-temperature channel;
- **ET**: environment/exhaust-temperature channel.

Both devices sit behind a shared SPI abstraction and feed a sensor conversion hub above the raw
drivers. The control core reasons about converted temperatures, not raw MAX31856 frames, which keeps
the protocol and PID layers hardware-agnostic.

The bus runs at 1 MHz in SPI mode 1 (CPOL=0, CPHA=1). Conversion is one-shot with a 210 ms wait;
FAULT and DRDY are not wired.

## 5. Actuator Subsystem

### Heater path

The heater is driven through an SSR-backed LEDC channel. A separate heat-detection input provides a
sanity signal about whether the heating path appears electrically active, so LibreRoaster does not
treat “PWM command sent” as proof of “heat applied.”

### Fan path

The fan is driven through a second LEDC channel. Protocol percentages are scaled into the hardware
driver's PWM domain.

GPIO9 is a strapping pin, so the external fan stage must not assert a dangerous boot level during
reset. The safe failure direction is airflow with no heat.

## 6. PWM and Timer Topology

`src/hardware/init.rs` configures two low-speed LEDC timers:

- **Timer0** for the SSR at **5 Hz** (zero-cross compatible, 14-bit),
  bound to LEDC channel 1;
- **Timer1** for the fan at **25 kHz** (8-bit), bound to LEDC channel 0.

The `SSR_LEDC_CHANNEL`/`SSR_LEDC_TIMER`/`FAN_LEDC_CHANNEL`/`FAN_LEDC_TIMER` constants mirror this
mapping, but the init path currently hard-codes timer and channel numbers. Treat the init path and
its boot assertions as the enforced mapping.

## 7. Timing Values That Matter Physically

The firmware relies on these operational assumptions:

- control-loop period **100 ms** (`CONTROL_LOOP_PERIOD_MS`); the real tick is **≈ 310–330 ms**
  because each tick waits for the MAX31856 conversion;
- MAX31856 one-shot conversion wait **210 ms** (datasheet 185 ms + margin);
- watchdog feed once per control tick (`WATCHDOG_FEED_INTERVAL_MS = CONTROL_LOOP_TICK_MS`);
- hardware watchdog timeout **≈ 2.2 s nominal** (`HW_WATCHDOG_STAGE0_CYCLES = 300000` at 136 kHz,
  with firmware compensation for the efuse `wdt_delay_sel` shift);
- LEDC guard timeout **10 ms**;
- temperature validity timeout **1000 ms**.

These values shape roast behavior and failure behavior, not just performance. The canonical timing
reference is [`ARCHITECTURE.md`](ARCHITECTURE.md) §9; `src/config/constants.rs` is the source of
truth.

## 8. Safety-Relevant Electrical Constraints

### Heat detection

GPIO1 is a pulled-up input that detects heater-side activity. Its value feeds safety reasoning
about whether the commanded heater state matches observed behavior.

Expected circuit: a current sensor on the heater load whose output pulls GPIO1 LOW while the heater
conducts (open-collector or optocoupler in the load path). At rest the internal pull-up keeps the
pin HIGH. The sensor type is the builder's choice; the polarity above is the contract.

- **Without the circuit** (default build): GPIO1 is not interpreted. The heat-source check is
  opt-in via the `heat-sense` cargo feature. Without the feature the availability state stays
  `Available`, and all other safety layers (overtemp, RoR, probe-stuck, comms-idle, roast-time cap,
  watchdog, fan floor) stay active.
- **With the circuit** (`heat-sense` build): the pin must stay LOW **continuously** while the SSR
  conducts. A bare AC optocoupler follows the mains sine and releases near every zero crossing, so
  at the per-tick sampling cadence those HIGH stretches trip the time-debounce window
  (`HEAT_MISMATCH_WINDOW_MS = 1500 ms`) on every roast. Stretch the pulse with an RC or a
  retriggerable monostable of 20 ms or more so GPIO1 reads LOW for the whole conduction time.
  Validate with an oscilloscope at 50/75/100 % duty (`examples/hil_gpio.rs` and
  `examples/hil_ssr.rs`) before trusting the interlock.
- A transient “no heat” read is time-debounced: the cross-check latches only after
  `HEAT_MISMATCH_WINDOW_MS` with no LOW at observable duty ≥ 50 %; a single LOW sample re-clears the
  window. The cross-check samples once per control tick from `periodic_check`, never on the write
  path.

### Status LED (GPIO8)

GPIO8 is **not** a strapping pin for normal flash boot; it is safe as a push-pull status indicator.
The service container owns it; `src/hardware/status_led.rs` holds the pattern logic and the control
loop drives the GPIO once per tick.

Pattern: off in `Idle`, 1 Hz blink in `Preheating`, solid in `Heating`/`Stable`, 4 Hz blink on
`Error` or any fault. The safe-shutdown path takes GPIO8 via `Peripherals::steal()` and blinks it
3 × (200 ms low, 200 ms high) plus a ~1 s pause after application tasks are dead. Use a 330 Ω series
resistor; never drive it open-drain.

### Strapping pins

Keep these points visible in any build:

- avoid GPIO2 for SPI MISO (boot-mode strap; see [`pinout.md`](pinout.md) §GPIO2);
- treat GPIO9 carefully because it is the boot-mode strap used for fan PWM. The datasheet 45 kΩ
  internal weak pull-up boots correctly when floating, but firmware neither configures nor verifies
  it. External 10 kΩ → 3.3 V is mandatory on custom boards, bare modules, and any external driver
  that could pull low at reset (see [`CONNECTION_TYPES.md`](CONNECTION_TYPES.md) §4);
- GPIO8 is ignored for normal SPI boot; only the download-mode combination GPIO8=HIGH + GPIO9=LOW
  matters;
- avoid external circuitry that forces invalid boot levels on any strap.

### High-voltage separation

The heater power stage is externally isolated and properly designed. LibreRoaster does not make
unsafe hardware safe by software alone. Keep mains wiring physically separated from logic, bond the
chassis to protective earth, and fit a hardware thermal cutoff in series with the heater.

## 9. Expected Supporting Hardware

Typical build assumptions:

- ESP32-C3 development board;
- two MAX31856 boards;
- two type-K thermocouples;
- zero-cross SSR for heater switching;
- fan stage capable of PWM control;
- appropriate low-voltage supply for logic and fan stage;
- safe isolation and mains-rated heater wiring where applicable.

Exact part choices are flexible; the signal model is not.

## 10. Practical Integration Notes

### USB-first development

Native USB CDC is the simplest development path: one cable, no adapter, and the same transport
Artisan expects. Use UART for bring-up, debug, or isolation from USB.

### Thermal sensor sanity

Before trusting PID behavior, verify that BT and ET are wired to the intended channels. A swapped
pair is electrically valid but behaviorally misleading.

### HIL calibration

The host thermal plant in `src/hardware/sensors/thermal_model.rs` is a provisional model tuned for
closed-loop tests. Its gains and time constants must be recalibrated on real drum hardware before
PID gains derived from that plant are trusted for production. Simulation results are not electrical
safety evidence.

## 11. Hardware-Facing Risks

The hardware layer is stable enough to run, but keep these points in mind:

1. strap-pin sensitivity on GPIO9;
2. slow sensor reads relative to the nominal control cadence;
3. the heat-detection line is safety-relevant, not optional decoration;
4. mains isolation and a hardware thermal cutoff are mandatory, not firmware responsibilities.

## 12. Related Documents

- [`pinout.md`](pinout.md) — physical wiring, subsystem diagrams, and board-damage prevention.
- [`CONNECTION_TYPES.md`](CONNECTION_TYPES.md) — USB/UART choice and GPIO9 boot behavior.
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — how hardware feeds the runtime model.
- [`PROTOCOL.md`](PROTOCOL.md) — how hardware state appears over serial.
- [`SAFETY_CASE.md`](SAFETY_CASE.md) — hazards and guard evidence.
- [`SIMULATION.md`](SIMULATION.md) — Wokwi pinout simulation and its limits.
- [`ARTISAN_CONNECTION.md`](ARTISAN_CONNECTION.md) — host-side connection workflow.

---

*Last updated: 2026-10-24.*
