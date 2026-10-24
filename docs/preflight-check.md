# LibreRoaster Pre-Flight Checklist

**Purpose:** mandatory verification before first power-on with real hardware. Complete every item
in order and do not skip steps.

**Last updated:** 2026-10-24

This checklist owns the physical bring-up sequence. Build, flash, and test commands are owned by
[`DEVELOPMENT.md`](DEVELOPMENT.md); pin definitions and electrical limits are owned by
[`HARDWARE.md`](HARDWARE.md) and [`pinout.md`](pinout.md). Command grammar is owned by
[`PROTOCOL.md`](PROTOCOL.md).

---

## Phase 0 — Build Verification (Computer)

Run the canonical verification commands from [`DEVELOPMENT.md`](DEVELOPMENT.md) §4–§7 before
touching hardware.

- [ ] Embedded build completes with zero errors and zero warnings.
- [ ] Host test suite passes with no failures.
- [ ] `cargo fmt --check` and the Clippy gate pass.
- [ ] The firmware artifact was built with the intended feature set (`embedded`, plus
  `heat-sense` only if the current-sense circuit is installed and validated).

---

## Phase 1 — Visual Inspection (Unpowered)

Use a multimeter in continuity/resistance mode with the board unpowered.

### Power rails

- [ ] No short between the 3.3 V rail and GND.
- [ ] No 5 V connection reaches any ESP32-C3 pin.
- [ ] 100 µF electrolytic plus 0.1 µF ceramic decoupling are fitted near the ESP32-C3.

### SPI bus (MAX31856 thermocouples)

Verify against the pin map in [`HARDWARE.md`](HARDWARE.md) §2:

| Signal | GPIO |
|---|---:|
| SCLK | 6 |
| MOSI (SDI) | 7 |
| MISO (SDO) | 5 |
| CS ET | 3 |
| CS BT | 4 |

- [ ] SCLK → GPIO6, MOSI → GPIO7, MISO → GPIO5.
- [ ] CS ET → GPIO3; CS BT → GPIO4.
- [ ] MAX31856 VCC = **3.3 V**, not 5 V.
- [ ] Type-K polarity: yellow (+), red (−).
- [ ] Neither CS line has an external pull-up or pull-down; firmware drives both.

### Actuators

| Signal | GPIO | Frequency |
|---|---:|---|
| SSR heater | 10 | 5 Hz zero-cross |
| Fan MOSFET | 9 | 25 kHz |
| Status LED | 8 | — |

- [ ] SSR control line → GPIO10.
- [ ] Fan PWM line → GPIO9.
- [ ] Status LED → GPIO8 with a 330 Ω series resistor (if fitted).

### Strapping and protection

- [ ] GPIO9 has a **10 kΩ pull-up to 3.3 V** on custom boards and bare modules; official dev boards
  may already include it.
- [ ] The fan gate driver is high-impedance at reset and cannot pull GPIO9 LOW during boot.
- [ ] GPIO9 additionally has the 1 kΩ series resistor and 100 kΩ weak gate pull-down from
  [`pinout.md`](pinout.md).
- [ ] GPIO10 has a **10 kΩ pull-down to GND**. This is mandatory; the SSR line must not be HIGH
  before firmware drives it.
- [ ] GPIO8 does not form the invalid download-mode combination (GPIO8=HIGH + GPIO9=LOW).
- [ ] GPIO2 is unconnected.

### Communication

- [ ] UART RX ← adapter TX on GPIO20; UART TX → adapter RX on GPIO21.
- [ ] The UART adapter is **3.3 V**, or its 5 V signals are level-shifted.
- [ ] The USB cable is connected to the ESP32-C3 native USB port, not to a UART adapter.

---

## Phase 2 — First Power-On (Heater Disconnected)

**Do not connect the heater element yet.**

1. Flash the firmware with the `embedded` feature using the command in
   [`DEVELOPMENT.md`](DEVELOPMENT.md) §5.
2. For log verification, flash an `embedded,instrumentation` build and connect a **3.3 V UART
   adapter** to GPIO20/21. Logs are emitted on UART0 at `info!`/`debug!`; the default `embedded`
   build filters them to `Warn`, and native USB carries only the Artisan protocol.
3. Open the UART0 serial monitor at 115200 8N1. Do not connect Artisan to UART0 at the same time.

- [ ] With the instrumentation build, the boot banner and `Hardware initialized` appear.
- [ ] With the instrumentation build, `Sensors initialized (BT: GPIO4, ET: GPIO3)` appears on a
  non-simulated build.
- [ ] With the instrumentation build, `Hardware watchdog initialized (RTC WDT)` appears.
- [ ] With the instrumentation build, `USB CDC initialized` appears.
- [ ] No panic or fatal error appears.
- [ ] The status LED follows the state pattern: off in `Idle`, 1 Hz in `Preheating`, solid in
  `Heating`/`Stable`, 4 Hz on fault.

If no UART adapter is available, the log assertions cannot be verified in this phase. Do not sign
off Phase 2 as fully passed on electrical checks alone.

With a multimeter:

- [ ] The 3.3 V rail measures **3.3 V ±5 %** (3.15–3.45 V).
- [ ] GPIO10 (SSR) < 0.1 V (LOW; heater off).
- [ ] GPIO9 (fan) < 0.1 V (LOW; fan off).
- [ ] GPIO8 (LED) follows the status pattern; in `Idle` it is LOW (≈0 V, LED off), not stuck HIGH.
- [ ] GPIO3 (CS ET) ≈3.3 V (HIGH; inactive).
- [ ] GPIO4 (CS BT) ≈3.3 V (HIGH; inactive).

After log verification, reflash the production `embedded` build (without `instrumentation`) before
configuring Artisan or connecting the heater.

---

## Phase 3 — Sensor Verification

Connect over native USB (`/dev/ttyACM0`) or UART at 115200 8N1.

### Temperature reading

Send `READ` and verify:

```text
Expected: AMB,ET,BT,0.0,0.0   (PID off; 5 fields)
Example:  0.0,23.5,24.1,0.0,0.0
```

- [ ] `READ` returns five comma-separated values.
- [ ] `AMB` is the MAX31856 cold-junction mean and is near ambient, not a placeholder.
- [ ] ET and BT are reasonable ambient temperatures (18–30 °C).
- [ ] Heater and fan fields are `0.0`.

### Sensor self-test

- [ ] ET is not `0.0` or `-999` (either indicates SPI failure).
- [ ] BT is not `0.0` or `-999`.
- [ ] ET and BT are within ~2 °C of each other at ambient.
- [ ] Touching the BT probe raises BT within 2 seconds.
- [ ] Touching the ET probe raises ET within 2 seconds.

**If any sensor reads 0.0 or an absurd value, stop. Do not proceed.** Check SPI wiring first.

### Status report

Send `STATUS` and verify against [`INSTRUMENTATION.md`](INSTRUMENTATION.md):

- [ ] The response has exactly 20 CSV fields. Field 7 `LastWatchdogReason` is a token such as
  `none`; the other fields are numeric.
- [ ] Field 5 `WatchdogOK` = `1`.
- [ ] Field 3 `Heater` = `0.0` and field 4 `Fan` = `0.0`.
- [ ] Field 19 `TempScale` = `0` and field 20 `FaultFlag` = `0`.

---

## Phase 4 — Actuator Verification (Heater Still Disconnected)

### Fan test

```text
IO3 50
```

- [ ] Fan spins at ~50 % speed.
- [ ] GPIO9 shows PWM (≈1.6 V average at 50 % duty on a DMM).

```text
IO3 100
IO3 0
```

- [ ] Full speed, then a complete stop.

### SSR test (heater disconnected, GPIO only)

```text
OT1 50
```

- [ ] GPIO10 shows PWM activity (≈1.6 V average at 50 % duty).
- [ ] Keep the session alive with `READ` or `STATUS` every 5–10 s while the heater is energized.

```text
OT1 0
```

- [ ] GPIO10 returns below 0.1 V.

### Emergency stop test

```text
OT1 80
IO3 80
STOP
```

- [ ] GPIO10 goes LOW immediately.
- [ ] Fan goes to 100 % and stays there (cooldown latch, until BT < 60 °C).
- [ ] `STATUS` shows `Heater = 0.0`, `Fan = 100.0`, `FaultFlag = 1`.
- [ ] The run state is visible in the serial log; it is not a STATUS field. This requires the
  instrumentation build and the UART0 log connection from Phase 2.

`STOP` arms the emergency latch. Recover explicitly before proceeding:

```text
PID;OFF
```

- [ ] `STATUS` shows `FaultFlag = 0`; the state returns to `Idle`.
- [ ] The heater remains off. The fan may stay at 100 % until BT falls below 60 °C; wait for it to
  stop, or power-cycle, before starting Phase 5.

---

## Phase 5 — Artisan Connectivity

Configure Artisan using [`ARTISAN_CONNECTION.md`](ARTISAN_CONNECTION.md) §2. Use the native USB
port, 115200, and the ArduinoTC4 driver.

- [ ] Artisan connects without errors.
- [ ] Temperature readings appear in the plot.
- [ ] ET and BT update about once per second.
- [ ] `OT1 30` from Artisan changes the SSR output.
- [ ] `IO3 50` changes the fan output.
- [ ] The handshake (`CHAN`, `UNITS`, `FILT`) does not crash or reset the firmware.

---

## Phase 6 — Connect Heater (Final Step)

Proceed only if every previous phase passed.

- [ ] The heater element is mounted securely in the roaster.
- [ ] A hardware thermal fuse or cutoff rated for the heater is in series with the element.
- [ ] The SSR output is connected to the heater AC circuit with proper isolation.
- [ ] Protective earth is bonded to the metal chassis.
- [ ] A fire extinguisher is accessible and the area is ventilated.

### First heat test

```text
OT1 10
```

- [ ] The heater produces gentle warmth at 10 % duty.
- [ ] Artisan temperature readings rise.
- [ ] No smoke, arcing, or unusual smell.

```text
OT1 0
```

- [ ] The heater turns off and the temperature stabilizes, then falls.

### Safety shutdown test

```text
OT1 50
```

Wait 5 seconds, then:

```text
STOP
```

- [ ] The heater cuts immediately.
- [ ] The fan goes to 100 % and stays there.
- [ ] The system enters the `Error` state; the serial log confirms it.
- [ ] `STATUS` shows `Heater = 0.0`, `Fan = 100.0`, `FaultFlag = 1`.
- [ ] `PID;OFF` clears the latch (`FaultFlag = 0`) and leaves the heater off.

---

## Emergency Procedures

### Firmware panic or crash

1. The hardware watchdog resets the CPU in roughly 2 seconds.
2. Outputs initialize LOW after reset.
3. If the reset repeats (blinking LED pattern), disconnect the heater and investigate.

### Temperature reaches a cutoff

- The firmware commands an emergency shutdown at BT ≥ 260 °C or ET ≥ 300 °C: heater 0 %, fan
  100 %.
- If firmware cannot control the heater, the hardware thermal fuse is the last line of defense.

### SSR appears stuck ON

1. Send `STOP` immediately, or disconnect heater AC power first if there is any doubt.
2. Do not reconnect the heater until the SSR is verified functional.
3. Check for welded contacts with the power off.

---

## Sign-Off

| Phase | Status | Date | Notes |
|---|---|---|---|
| 0 — Build | ☐ Pass / ☐ Fail | | |
| 1 — Visual | ☐ Pass / ☐ Fail | | |
| 2 — Boot | ☐ Pass / ☐ Fail | | |
| 3 — Sensors | ☐ Pass / ☐ Fail | | |
| 4 — Actuators | ☐ Pass / ☐ Fail | | |
| 5 — Artisan | ☐ Pass / ☐ Fail | | |
| 6 — Heater | ☐ Pass / ☐ Fail | | |

**All phases must pass before regular operation.**

---

*Last updated: 2026-10-24.*
