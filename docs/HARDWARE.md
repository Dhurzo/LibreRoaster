# LibreRoaster Hardware Guide

**Last updated:** 2026-08-04

This document describes the hardware topology that the current firmware expects, the actual pin mapping in the codebase, and the electrical and timing constraints that matter when you build or modify the roaster.

## 1. Hardware role split

LibreRoaster assumes four major hardware responsibilities:

- **compute and transport** on the ESP32-C3,
- **temperature acquisition** through two MAX31856 thermocouple front ends,
- **heater switching** through an SSR-controlled output,
- **airflow control** through a PWM-driven fan stage.

The firmware is opinionated about this topology. It is adaptable, but not dynamically auto-discovered.

## 2. Pin map used by the firmware

The constants and hardware init code define this effective mapping:

| Signal | GPIO | Notes |
|---|---:|---|
| ET thermocouple chip select | 3 | shared SPI bus |
| BT thermocouple chip select | 4 | shared SPI bus |
| SPI MISO | 5 | routed through GPIO matrix to avoid strap conflict |
| SPI SCLK | 6 | FSPI clock |
| SPI MOSI | 7 | FSPI data out |
| Fan PWM | 9 | strapping pin; external circuit must not break boot |
| SSR control PWM | 10 | heater output path |
| Heat detection input | 1 | pull-up enabled |
| UART RX | 20 | serial ingress from host adapter |
| UART TX | 21 | serial egress to host adapter |

## 3. Why the SPI pins look unusual

ESP32-C3 nominal FSPI MISO maps to GPIO2, but GPIO2 is a strapping pin. LibreRoaster avoids that boot-risk path and instead routes MISO through GPIO5 via the GPIO matrix.

That design choice is not cosmetic. It is a boot-reliability measure.

## 4. Thermocouple subsystem

LibreRoaster expects two MAX31856 devices on the same SPI controller with separate chip selects.

### Functional mapping

- **BT**: bean temperature channel
- **ET**: environment/exhaust temperature channel

Both devices are wrapped through a shared SPI device abstraction and then lifted into a sensor conversion hub above the raw drivers.

### Design consequence

The control core reasons about converted temperatures, not raw MAX31856 frames. That keeps the protocol and PID layers hardware-agnostic once the sensor layer has done its job.

## 5. Actuator subsystem

### Heater path

The heater is driven through an SSR-backed LEDC channel. A separate heat-detection input provides a sanity signal about whether the heating path appears electrically active.

This means LibreRoaster does not blindly trust “PWM command sent” as proof of “heat applied.”

### Fan path

The fan is driven through a second LEDC channel. The firmware expresses fan values in the protocol as percentages and then scales them into the hardware driver’s PWM domain.

Because GPIO9 is a strapping pin, the external fan stage must be designed so it does not assert a dangerous boot level during reset.

## 6. PWM and timer topology

The hardware init code configures two low-speed LEDC timers:

- one timer for the SSR at **5 Hz** (zero-cross compatible),
- one timer for the fan at **25 kHz**.

## 7. Timing values that matter physically

The firmware currently relies on these operational assumptions:

- control-loop period **100 ms** (`CONTROL_LOOP_PERIOD_MS`); the real tick is
  **≈ 310–330 ms** because each tick also waits for the MAX31856 conversion
  (`MAX31856_CONVERSION_TIME_MS = 210`),
- MAX31856 one-shot conversion wait **210 ms** (datasheet 185 ms + margin),
- watchdog feed interval **100 ms**,
- hardware watchdog timeout **2 s**,
- LEDC guard timeout **10 ms**,
- temperature validity timeout **1000 ms**.

These values shape both roast behavior and failure behavior. If you change them, you are changing more than performance.

## 8. Safety-relevant electrical constraints

### Heat detection

GPIO1 is used as a pulled-up input to detect heater-side activity. Its value feeds safety reasoning about whether the commanded heater state matches observed behavior.

**BUG-02 (2026-08-21): the current-sense circuit on GPIO1 is OPTIONAL but strongly recommended.** The expected circuit: a current sensor on the heater
load whose output pulls GPIO1 LOW while the heater conducts (open-collector
or optocoupler in the load path); at rest the internal pull-up keeps the pin
HIGH. The exact sensor (current transformer, optocoupler) is builder's
choice — the only contract is the polarity above.

- **Without the circuit**, the pin floats HIGH ("no heat") and the firmware
  latches `NotDetected` at duty ≥ 50 % within ≈1.7 s, forcing the heater to
  0 % until an explicit operator recovery (`OFF`/`START`/`PREHEAT`/`StopRoast`
  re-arms the availability state machine). For builds that deliberately omit
  the circuit, compile with the `no-heat-sense` feature, which disables the
  heat-source interpretation (all other safety layers stay active).
- **With the circuit**, a transient "no heat" read is debounced
  (`HEAT_ABSENT_DEBOUNCE = 5` consecutive samples ≈ 1.7 s) before the latch,
  and a single LOW sample re-clears it.

### Status LED (GPIO8)

The status LED is a real runtime indicator (single owner: the service
container). Pattern: off in `Idle`, 1 Hz blink in `Preheating`, solid in
`Heating`/`Stable`, 4 Hz blink on `Error` or any fault. The safe-shutdown
path (init failure) takes GPIO8 via `Peripherals::steal()` and blinks it
3×400 ms — by then all application tasks are dead, so the steal is the final
owner.

### Strapping pins

The project documentation must always keep these points visible:

- avoid GPIO2 for SPI MISO in this design,
- treat GPIO9 carefully because it is a strap pin used for fan PWM,
- avoid external circuitry that forces invalid boot levels.

### High-voltage separation

The firmware docs assume the heater power stage is externally isolated and properly designed. LibreRoaster does not make unsafe hardware safe by software alone.

## 9. Expected supporting hardware

Typical build assumptions are:

- ESP32-C3 development board
- two MAX31856 boards
- two type-K thermocouples
- SSR for heater switching
- fan stage capable of PWM control
- appropriate low-voltage supply for logic and fan stage
- safe isolation and mains-rated heater wiring where applicable

The exact part choices are flexible, but the signal model is not.

## 10. Practical integration notes

### USB-first development

For most development work, native USB CDC is the simplest path because it avoids a separate UART adapter and matches the recommended Artisan setup.

### UART as a secondary transport

UART remains useful for bring-up, debug, and scenarios where USB CDC is unavailable or intentionally isolated.

### Thermal sensor sanity

Before trusting PID behavior, verify that BT and ET are wired to the intended channels. A swapped pair is electrically valid but behaviorally misleading.

## 11. Hardware-facing risks to remember

The hardware layer is stable enough to run, but these are the main points engineers should keep in mind:

1. strap-pin sensitivity on GPIO9,
2. slow sensor-read timing relative to nominal control cadence,
3. the fact that the heat-detection line is safety-relevant, not optional fluff.

## 12. Related documents

- `ARCHITECTURE.md` for how hardware feeds the runtime model
- `PROTOCOL.md` for how hardware state appears over serial
- `ARTISAN_CONNECTION.md` for the host-side connection workflow
