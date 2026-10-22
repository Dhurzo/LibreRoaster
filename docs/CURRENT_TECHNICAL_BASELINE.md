# 📋 Current Technical Baseline

**Last updated:** 2026-10-22

This page is the README-level technical baseline. It is a summary, not the source of truth:
current project state and constraints live in [`../CONTEXT.md`](../CONTEXT.md); build, flash, and
test commands plus the recorded verification snapshot live in [`DEVELOPMENT.md`](DEVELOPMENT.md);
the wire contract lives in [`PROTOCOL.md`](PROTOCOL.md).

- **Target MCU:** ESP32-C3 (`riscv32imc-unknown-none-elf`)
- **Runtime model:** `no_std` embedded firmware on the Embassy async executor, started through
  `esp-rtos`
- **Primary integration:** the official Artisan application over **USB CDC** on the ESP32-C3 native
  USB port, using a TC4-compatible command set. UART0 on GPIO20/21 is also supported.
- **Sensors:** two MAX31856 thermocouple channels (Type K), mapped to ET and BT, on a shared SPI bus
  at 1 MHz. Readings are one-shot with a 210 ms conversion wait and EMA filtering; boot-time
  self-tests are included.
- **Actuators:** SSR-controlled heater (5 Hz zero-cross LEDC, GPIO10) and PWM fan (25 kHz LEDC,
  GPIO9), with slew-rate limiting and a heat-source cross-check.
- **Safety layers:** software and hardware watchdogs, over-temperature cutoff (BT 260 °C, ET
  300 °C), rate-of-rise protection (45 °C/min soft and 60 °C/min hard in firmware-PID mode),
  stale-temperature guard (1 s), probe-stuck detection, comms-idle cutoff (15 s), heat-source and
  SSR stuck-on detection (GPIO1, opt-in `heat-sense` feature), and time budgets (30 min roast;
  60 min uncharged `PID;ON` session; 90 min manual session).
- **Control:** PID with anti-windup, selectable ET/BT channel, profile interpolation, and preheat
  behavior.
- **Simulated sensors:** synthetic roast curves for hardware-free runs on real ESP32-C3 hardware and
  in Wokwi.
- **Telemetry:** `READ` and `STATUS` are always available; spontaneous `#` telemetry is opt-in with
  `STREAM;ON`. The firmware also keeps a 256-sample in-memory roast ring buffer for `#DUMP`.
- **Status LED:** GPIO8 indicates state (off / 1 Hz / solid / 4 Hz; `Error` blinks at 4 Hz).
- **`heat-sense` (opt-in):** GPIO1 current-sense interpretation is off by default. Enable it only
  with a validated stretched-pulse circuit; see [`HARDWARE.md`](HARDWARE.md) §8.
- **Focused controllers:** `SensorController`, `ActuatorController` (heater and fan together),
  `SafetyController`, and `CommandDispatcher`.
- **Code coverage:** measured by `cargo-llvm-cov` in the CI coverage job. `codecov.yml` declares
  75 % project and 70 % patch targets; the upload itself is non-failing.

For the current verification counts and the exact feature matrix, use
[`DEVELOPMENT.md`](DEVELOPMENT.md). For the current milestone and pending hardware validation, use
[`../CONTEXT.md`](../CONTEXT.md).
