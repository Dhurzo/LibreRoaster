# 🔌 Hardware Model

**Last updated:** 2026-10-22

This page is the README-level hardware overview. [`HARDWARE.md`](HARDWARE.md) owns the firmware pin
map, subsystem topology, timing values, and electrical limits; [`pinout.md`](pinout.md) owns
physical wiring, assembly order, and board-damage prevention; [`CONNECTION_TYPES.md`](CONNECTION_TYPES.md)
owns the USB/UART trade-offs.

LibreRoaster expects a simple two-sensor / two-actuator topology:

- an ESP32-C3 for compute, native USB CDC, and UART0;
- two MAX31856 thermocouple front ends on a shared SPI bus;
- an SSR-controlled heater output;
- a PWM-driven fan stage.

## Pin map

| Signal | GPIO | Notes |
|---|---:|---|
| Heat detection input | 1 | Input with internal pull-up; LOW means the SSR appears to be conducting. Interpreted only with the opt-in `heat-sense` feature. |
| ET thermocouple chip select | 3 | Output, shared SPI bus |
| BT thermocouple chip select | 4 | Output, shared SPI bus |
| SPI MISO | 5 | Input, routed through the GPIO matrix to avoid the GPIO2 strap |
| SPI SCLK | 6 | Output, FSPI clock |
| SPI MOSI | 7 | Output, FSPI data out |
| Status LED | 8 | Output, push-pull, active-high; not a strap for normal SPI boot |
| Fan PWM | 9 | Output, LEDC 25 kHz; strapping pin |
| SSR control PWM | 10 | Output, LEDC 5 Hz zero-cross, 14-bit |
| UART RX | 20 | Input, 3.3 V only, 115200 baud |
| UART TX | 21 | Output, 3.3 V only, 115200 baud |

USB D+/D− is internal to the ESP32-C3 and has no external GPIO.

## Constraints that matter operationally

1. **GPIO9 is a strapping pin.** Official ESP32-C3 development boards already include the required
   pull-up. Custom boards need a **10 kΩ pull-up to 3.3 V**, and external fan circuitry must not
   force an invalid boot level. The safe failure direction is airflow with no heat.
2. **SPI MISO uses GPIO5, not GPIO2.** GPIO2 is a strapping pin; routing MISO through GPIO5 avoids a
   boot-reliability conflict.
3. **GPIO10 needs a 10 kΩ pull-down to GND.** The pin has no defined level between reset and
   firmware initialization, so the pull-down is mandatory to keep the SSR off during boot.
4. **Keep 5 V away from ESP32-C3 pins.** All GPIOs are 3.3 V logic. Use proper level shifting and
   mains isolation for the heater side.

The SPI bus runs at 1 MHz in mode 1. Thermocouple conversion is one-shot with a 210 ms wait. The SSR
uses a 5 Hz zero-cross LEDC channel; the fan uses a 25 kHz LEDC PWM channel.

Before first power-on with real mains-adjacent hardware, complete
[`preflight-check.md`](preflight-check.md). For connector-level wiring diagrams and assembly notes,
use [`pinout.md`](pinout.md).
