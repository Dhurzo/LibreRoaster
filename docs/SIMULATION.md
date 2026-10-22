# No-Hardware Simulation

This guide explains how to run LibreRoaster without a physical ESP32-C3, what
the Wokwi simulation can and cannot prove, and how to move between the
supported test builds.

The simulation project lives in `sim/wokwi/`. It is deliberately isolated from
the firmware source tree: the firmware is built for the simulator with the
same `embedded` code path, plus simulation-only Cargo features
(`simulated-sensors`, `simulated-typical-roast`, and optionally `heat-sense`).

> **Scope:** this is a development aid, not a hardware safety sign-off. Wokwi
> cannot validate mains isolation, SSR thermal behavior, thermocouple accuracy,
> watchdog silicon behavior, or real power-stage failure modes.

## 1. What the simulation validates

### Build variants

| Build | Cargo features | Sensor source | GPIO1 heat-sense | Main use |
|---|---|---|---|---|
| F1 | `embedded,simulated-sensors` | Synthetic medium curve; SPI idle | Compiled out | Board, UART, LED, PWM, and protocol bring-up |
| F2 | `embedded,heat-sense` | Real MAX31856 over SPI | Active; manual switch | Full pinout, SPI, and heat-sense check |
| F2 without interlock | `embedded` | Real MAX31856 over SPI | Compiled out | SPI and actuators without the GPIO1 interlock |
| F3 | `embedded,simulated-typical-roast` | Synthetic typical curve; SPI idle | Compiled out | Guard-safe `START`/`Heating` test |
| F0 | `embedded` + `esp-emulator` | Host emulator | Not modeled | Bootloader/app-load reproduction only (blocked) |

### Coverage matrix

| Area | Wokwi (F1/F2/F3) | esp-emu (F0) | Host tests |
|---|---|---|---|
| Boot, UART0, Artisan protocol | Yes | Bootloader/app load only | Protocol unit tests |
| Status LED (GPIO8) | Yes | No (blocked) | Pattern logic |
| SSR PWM (GPIO10) | Yes (LED + logic analyzer) | No (blocked) | State machine |
| Fan PWM (GPIO9) | Logic analyzer only | No (blocked) | State machine |
| MAX31856 SPI (CS/SCK/MOSI/MISO) | Yes in F2 | No (no external SPI model) | Scripted SPI |
| Heat-sense input (GPIO1) | Yes in F2 | No (blocked) | State machine |
| Native USB CDC | Not exposed on the board model | Blocked | Protocol unit tests |
| Mains isolation, SSR thermal behavior, real thermocouple physics | No | No | No |

### What each Wokwi build changes

- **F1** bypasses the MAX31856 driver. It boots the normal application with
  synthetic temperatures, so it is the right build for UART, LED, PWM, and
  Artisan-session bring-up. The SPI pins stay idle and the heat-sense switch is
  ignored.
- **F2** uses the real MAX31856 driver and the real GPIO1 input. The custom
  chips in `diagram.json` provide the temperatures. This is the complete pinout
  check.
- **F3** is F1 with a charge-dip curve whose positive BT slopes stay at or below
  about 0.33 °C/s. It starts at a 200 °C charge, drops to a ~100 °C turning
  point, then rises slowly enough that `START` can remain in `Heating` without
  tripping the rate-of-rise (RoR) guard.
- **F0** (`scripts/sim-esp-emu.sh`) is a reproduction harness for the blocked
  esp-emulator path. It is not a working firmware run; see §7.

## 2. Prerequisites

- Rust target `riscv32imc-unknown-none-elf`:

  ```bash
  rustup target add riscv32imc-unknown-none-elf
  ```

- `espflash` on `PATH`:

  ```bash
  cargo install espflash --locked
  ```

- VS Code with the Wokwi extension. The extension offers a free local license
  via `Wokwi: Request a new License`; a paid plan is not required for the GUI
  workflow.
- `wokwi-cli` to compile the custom MAX31856 chip and, optionally, to run
  scripted scenarios. The prepare script uses it, and
  `sim/wokwi/chips/max31856.chip.wasm` is not committed, so a fresh clone needs
  this step. The first chip compile downloads WASI-SDK and `wokwi-api.h`; both
  are cached locally.

`WOKWI_CLI_TOKEN` is **not** required for the VS Code workflow. It is only
needed by the scripted scenario runner in §3.4.

## 3. Quick start

### Build and open a simulation

```bash
# F1: board/UART/PWM bring-up with simulated temperatures
./scripts/sim-wokwi-prepare.sh

# F2: real MAX31856 driver + heat-sense over GPIO1
FEATURES=embedded,heat-sense ./scripts/sim-wokwi-prepare.sh

# F3: guard-safe typical roast for START/Heating
FEATURES=embedded,simulated-typical-roast ./scripts/sim-wokwi-prepare.sh
```

The script builds the firmware, creates a merged image with
`espflash save-image --chip esp32c3 --merge`, copies the ELF, and compiles the
custom chip when `wokwi-cli` is available. It writes:

```text
sim/wokwi/libreroaster.bin              merged flash image
sim/wokwi/libreroaster.elf              symbols for the debugger
sim/wokwi/chips/max31856.chip.wasm      compiled custom chip
```

Open `sim/wokwi/` in VS Code and run `Wokwi: Start Simulator`, then open the
serial monitor at **115200 baud**. The firmware logs to UART0 and uses the same
UART as the Artisan transport, so the monitor both shows logs and sends
commands.

### First commands

```text
CHAN;0
UNITS;C
READ
```

`READ` returns a TC4-style `AMB,ET,BT,unused,unused` line. In F1/F3 the values
come from the synthetic curve; in F2 they come from the MAX31856 sliders.

To see continuous `#` telemetry, send `STREAM;ON` (off by default). To exercise
the actuators without arming the RoR guard:

```text
PREHEAT 200
OT1;100
OT1;0
PID;OFF
```

`PREHEAT 200` shows the 1 Hz green LED (Preheating) and energizes the heater.
`OT1;100` drives the SSR; the red LED and LA D0 follow the output. Do **not**
use `START` with the default F1 medium curve just to test LEDs: its early ramp
exceeds the RoR guard and the firmware correctly latches
`Bean temperature rate-of-rise exceeded`. Use F3 for a guard-safe
`START`/`Heating` test.

> **Keepalive:** once the heater is energized (`PREHEAT`, `START`, `OT1`, or
> `PID;ON`), send `READ` or `STATUS` every 5–10 s. Fifteen seconds without an
> inbound command trips the comms-idle safety latch and turns the heater off.
> `STREAM;ON` is output-only and does not refresh the timer. See §3.5.

### 3.1 F1 test checklist (simulated sensors)

1. Build and open F1:

   ```bash
   ./scripts/sim-wokwi-prepare.sh
   ```

   Start `sim/wokwi/` in VS Code and open the serial monitor at 115200.

2. The release image logs at `Warn`, so application `info!` lines are hidden.
   The ESP-IDF bootloader still ends with `Loaded app from partition`. Send
   `READ` to confirm the application is alive. There must be no MAX31856
   self-test lines and no SPI initialization failure.

3. Send `CHAN;0`, `UNITS;C`, and `READ`. With PID disabled the reply is
   `0.0,<ET>,<BT>,0.0,0.0`; `AMB` is `0.0` because simulated mode has no
   cold-junction hardware.

4. The medium curve starts at 25 °C/25 °C, but it is time-based from source
   construction. By the time you send `READ` after boot, it will already be a
   few degrees higher. At about 30 s it is near BT 80 °C / ET 100 °C, and at
   about 60 s near BT 120 °C / ET 150 °C; the EMA filter may lag slightly.
   `START` does not restart the curve. Reset the simulator to restart at
   25 °C/25 °C.

5. Optional continuous trace: send `STREAM;ON` (`#OK`) and watch the `#` lines.
   While the heater is energized, keep sending `READ` or `STATUS` every 5–10 s.
   In Idle (heater off) the multiplexer releases the transport only after 60 s
   without commands.

6. LED and actuator check without arming the RoR guard:

   - **Green LED:** `PREHEAT 200` gives the 1 Hz Preheating blink; `PID;OFF`
     returns to Idle and turns it off.
   - **SSR LED:** `OT1;100` activates the red LED and LA D0; `OT1;0` turns them
     off.
   - Do **not** use `START` with the default medium curve: it enters Heating,
     arms the RoR guard, and the ramp correctly latches
     `Bean temperature rate-of-rise exceeded` (4 Hz green LED). Use F3 for the
     solid Heating/Stable pattern.
   - The heat-sense switch and the MAX31856 chips are ignored; SPI pins stay
     idle.

### 3.2 F2 test checklist (real SPI and heat-sense)

1. Build F2 and open the simulator:

   ```bash
   FEATURES=embedded,heat-sense ./scripts/sim-wokwi-prepare.sh
   ```

   Open the serial monitor at 115200.

2. The release image logs at `Warn`, so the `info!` self-test block (CR0, MASK,
   conversion values) is suppressed. Expect the two
   `Temperature out of expected range` warnings, one per channel, because the
   default preset is BT 200 °C / ET 220 °C and the self-test plausibility window
   is −50…100 °C. The warning is informational. There must be no
   `boot self-test failed`, `channel degraded`, or both-channels-dead abort.
   If you need the full `info!` block, add `instrumentation` to `FEATURES`.

3. Send `READ`. Expect approximately `35.0,220.0,200.0,0.0,0.0`
   (`AMB,ET,BT`).

4. Move the BT/ET `temperature` sliders and send `READ` again. The values move
   toward the new settings; the EMA filter can take several seconds to settle
   after a large step. `AMB` stays at the cold-junction 35 °C. Set `fault` to
   `0x01` on one chip to inject an open-circuit fault, watch the log/`STATUS`,
   then restore `0`; the other channel must stay healthy.

5. Check SPI with the logic analyzer: CS GPIO4/GPIO3 pulse once per control
   tick, SCK GPIO6 runs at 1 MHz, MOSI GPIO7 shows register writes, and MISO
   GPIO5 returns data.

6. Heat-sense check: start with the switch **RIGHT** (no current). Send
   `OT1;100`, move the switch **LEFT** within 1.5 s, and hold it there long
   enough to prove there is no mismatch latch (>1.5 s). Turn the heater off
   (`OT1;0`) and move the switch **RIGHT** again. Negative checks: leave the
   switch RIGHT with the heater on for more than 1.5 s (expect
   `heater not detected`), and boot with the switch LEFT while the heater is
   off (expect `SSR stuck-on` after 10 samples). Recover by moving the switch
   RIGHT and using `PID;OFF`, `START`, `PREHEAT`, or a reset. Full runbook:
   [`WOKWI_ROAST_PRESET.md`](WOKWI_ROAST_PRESET.md).

7. Actuator check: the red LED and LA D0 follow the requested `OT1` duty;
   `STOP` cuts the heater. The fan has no LED by design; probe LA D1.

`FEATURES=embedded` runs the same SPI and actuator checks without the GPIO1
interlock.

### 3.3 F3 test checklist (guard-safe typical roast)

F3 is F1 with a charge-dip curve designed to stay below the RoR guard. Use it
to test `START` → `Heating` (solid green LED) without editing the curve.

1. Build F3 and open the simulator:

   ```bash
   FEATURES=embedded,simulated-typical-roast ./scripts/sim-wokwi-prepare.sh
   ```

   Open the serial monitor at 115200.

2. The release image logs at `Warn`; application `info!` lines are hidden, but
   the bootloader still ends with `Loaded app from partition`. Send `READ` to
   confirm the application is alive. There must be no MAX31856 self-test lines
   and no SPI initialization failure. The curve starts at
   `0.0,220.0,200.0,0.0,0.0` at t=0. Because the curve starts when the source is
   built, the first `READ` after a multi-second boot already shows the charge
   dip (typically around `0.0,218,185,0.0,0.0`).

3. Send `START`. The green LED must be **solid** (Heating). The log must not
   contain `BT RoR`, `safety_fault`, or `Emergency shutdown`. The first slew
   step can log `SAFETY FAN-FLOOR: heater at 5.0% with fan at 0.0% — raising
   fan to minimum 20%`; that is the normal fan-floor interlock (a warning, not
   a latch).

4. During the charge dip, BT falls even with the heater on: F3 is a time-based
   curve, not a closed-loop plant, so the heater does not move the simulated
   temperature. After the turning point (~60 s), BT rises slowly toward about
   214 °C BT / 243 °C ET at 600 s. Every positive BT slope stays at or below
   about 0.33 °C/s, so the RoR guard remains armed but silent. Keep the Artisan
   session alive with `READ` or `STATUS` every 5–10 s while Heating.

5. `PID;OFF` ends the test and returns to Idle; reset the simulator to restart
   at the 200 °C/220 °C charge. `PREHEAT 200` is an alternative: 1 Hz green LED
   plus the red SSR LED, with the same 15 s keepalive. The MAX31856 chips and
   the heat-sense switch are ignored.

### 3.4 Scripted scenarios (Wokwi CLI)

The VS Code extension can start the simulator but cannot run scenario files.
Automated serial input requires `wokwi-cli` and a Wokwi CI token (the local
VS Code license is not a CI token):

```bash
curl -L https://wokwi.com/ci/install.sh | sh   # installs wokwi-cli
export WOKWI_CLI_TOKEN=wok_...                 # https://wokwi.com/dashboard/ci
```

### 3.4.1 F1/F2/F3 scenario matrix and curve selection

Each scenario requires the matching firmware image. The runner writes
`target/wokwi-<scenario-name>.log` unless you pass a second argument.

| Build | Features | Scenario | What it checks |
|---|---|---|---|
| F1 | `embedded,simulated-sensors` | `scenarios/f1-smoke.yaml` | Handshake, `READ`, `PREHEAT 200`, and the 20 % fan floor. It does **not** send `START` (the medium curve would latch the RoR guard). |
| F2 | `embedded,heat-sense` | `scenarios/f2-spi.yaml` | Handshake, initial `35.0,220.0,200.0`, then slider changes to `35.0,230.0,250.0`. It does **not** energize the heater or move the heat-sense switch. |
| F3 | `embedded,simulated-typical-roast` | `scenarios/f3-roast.yaml` | `PREHEAT`, 15 s keepalive, `START`/`Heating`, GPIO8 high, then `PID;OFF` and GPIO8 low. |

```bash
# F1
FEATURES=embedded,simulated-sensors ./scripts/sim-wokwi-prepare.sh
./scripts/sim-wokwi-scenario.sh scenarios/f1-smoke.yaml

# F2
FEATURES=embedded,heat-sense ./scripts/sim-wokwi-prepare.sh
./scripts/sim-wokwi-scenario.sh scenarios/f2-spi.yaml

# F3 (default scenario and log name)
FEATURES=embedded,simulated-typical-roast ./scripts/sim-wokwi-prepare.sh
./scripts/sim-wokwi-scenario.sh
```

The runner fails on `safety_fault`, `Emergency shutdown`, `BT RoR`, or
`Control update error`. The image must match the scenario; rebuilding is
required when you switch feature set. Wokwi's free API session is limited to
5 minutes, so the scenarios are intentionally short.

`scenarios/f3-roast.yaml` waits for `Loaded app from partition`, gives the
application 6 s to initialize, then sends `PREHEAT 200` and two `READ`s 8 s
apart (16 s total, crossing the 15 s window). It then sends `PID;OFF` and
`START`, polls `READ` twice 4 s apart, and finishes with `PID;OFF`. It asserts
that GPIO8 is high in `Heating` and low after the final `PID;OFF`.

The scenario path is relative to `sim/wokwi`; the default is
`scenarios/f3-roast.yaml`. The VS Code GUI checklists in §3.1–§3.3 remain the
fallback when no CI token is available.

> **Curve selection is compile-time.** There is no serial command or Wokwi
> setting that selects a roast curve. F1 uses `default_medium_roast()`; F3 uses
> `typical_roast()` via `embedded,simulated-typical-roast`; F2 does not use a
> curve at all. `light_roast()` has no Cargo feature and is only available from
> code or host tests. See
> [`simulated-curve-test.md`](simulated-curve-test.md) for the full matrix and
> custom curves.

### 3.5 Why PREHEAT/START latches after 15 s

There are two different inactivity timers:

| Timer | Value | Armed when | Effect |
|---|---:|---|---|
| Safety `COMMS_IDLE_TIMEOUT_MS` | 15 s | heater energized or roast active | emergency latch: heater 0 %, fan 100 %, `ERR safety_fault Comms idle timeout` |
| Multiplexer `IDLE_TIMEOUT_SECS` | 60 s | session Idle (heater off) | output transport is released; no safety latch |

`PREHEAT 200`, `START`, `OT1`, and `PID;ON` all energize the heater, so the
15 s safety timer is armed. The green LED depends on state: 1 Hz in
Preheating, solid in Heating/Stable, and 4 Hz after a safety latch. The red
SSR LED follows the heater while it is on. If no inbound command arrives for
more than 15 s, the log looks like this:

```text
WARN - SAFETY COMMS-IDLE: no command for 15050ms (>15000ms) — emergency shutdown
ERROR - Emergency shutdown: Comms idle timeout
ERR safety_fault Comms idle timeout
```

That is expected supervision, not a PREHEAT bug, an RoR trip, or the FAN-FLOOR
warning. The FAN-FLOOR warning (`heater at 5.0% with fan at 0.0%`) is the
normal first slew step. After the comms-idle latch the heater is forced off
(red LED off) and the green LED switches to the 4 Hz Error pattern.

Recovery and keepalive:

- Recover with `PID;OFF`, `PREHEAT`, or `START`.
- While the heater is energized, send `READ` or `STATUS` every 5–10 s.
  `STREAM;ON` is output-only and does **not** refresh the timer.
- Artisan polls `READ` once per sampling interval (default 2 s), so a normal
  Artisan roast keeps the timer refreshed. If Artisan is closed, loses the
  serial link, or is configured above 15 s, the latch is intentional.
- The scripted scenarios in §3.4/§3.4.1 send `READ` within the 15 s window and fail if
  this latch appears.

## 4. Pin checklist

Wokwi board: `board-esp32-c3-devkitm-1`. The wiring is in
`sim/wokwi/diagram.json`.

| Signal | GPIO | Wokwi part / probe | Expected observation |
|---|---:|---|---|
| UART0 TX | 21 | serial monitor; LA D7 | Boot log and command responses |
| UART0 RX | 20 | serial monitor | Commands accepted |
| Status LED | 8 | green LED + 330 Ω | Off in Idle, 1 Hz in Preheating, solid in Heating/Stable, 4 Hz on Error |
| SSR control | 10 | red LED + 220 Ω; LA D0 | 5 Hz zero-cross bursts; LED activity follows duty |
| Fan PWM | 9 | **LA D1 only** | 25 kHz PWM; no direct LED (strapping pin) |
| BT chip select | 4 | LA D2 | Pulses once per control tick in F2; idle in F1 |
| ET chip select | 3 | LA D3 | Pulses once per control tick in F2; idle in F1 |
| SPI SCLK | 6 | LA D4 | 1 MHz clock during transfers in F2 |
| SPI MOSI | 7 | LA D5 | Register writes (`0x80`/`0x81`/`0x82`, `0x51` one-shot) |
| SPI MISO | 5 | LA D6 | Register/conversion data in F2 |
| Heat-sense | 1 | slide switch to GND | F2 only: RIGHT = no current (start here), LEFT = current |
| 3V3 / GND | — | board pins | Power for both chips and the LED circuits |

GPIO9 is an ESP32-C3 strapping pin. A direct LED to GND would pull it low at
reset, so the diagram leaves it on the logic analyzer only. On real hardware,
any external fan driver must not pull GPIO9 low at reset, and a 10 kΩ pull-up
to 3.3 V is mandatory (see `docs/HARDWARE.md`).

GPIO8 is ignored for normal SPI boot; only the download-mode combination
GPIO8=HIGH + GPIO9=LOW matters, so the status LED is safe in this diagram.

## 5. Custom MAX31856 chip

The chip model is `sim/wokwi/chips/max31856.chip.c`, with pin/control metadata
in `max31856.chip.json`. Two instances are wired: BT on CS GPIO4 and ET on
CS GPIO3, sharing SCLK/MOSI/MISO.

### Controls

| Control | Range | Default | Meaning |
|---|---|---|---|
| `temperature` | −200…1350 °C | BT 200, ET 220 | Thermocouple temperature reported in LTCBH/LTCBM/LTCBL |
| `coldJunction` | −20…85 °C | 35 | Cold-junction temperature reported in CJTH/CJTL |
| `fault` | 0…255 | 0 | Fault register value; 0 = healthy, bit 0 = open circuit |

### Registers

| Address | Register | Behavior |
|---:|---|---|
| 0x00 | CR0 | Stored; bit 6 (1SHOT) updates the conversion block |
| 0x01 | CR1 | Stored; read-back verification works |
| 0x02 | MASK | Stored |
| 0x0A | CJTH | High byte of the left-justified 14-bit CJ value |
| 0x0B | CJTL | Low byte |
| 0x0C | LTCBH | High byte of the left-justified 24-bit temperature |
| 0x0D | LTCBM | Middle byte |
| 0x0E | LTCBL | Low byte |
| 0x0F | SR | Current `fault` control value |

The driver writes CR0=0x11, CR1=0x03, and MASK=0x00 at boot, then runs its
self-test (including the 0x13 write/read-back probe and a 0x51 one-shot). The
chip supports that sequence. Reads use the 6-byte conversion block starting at
0x0A; single-register reads are also supported.

Encoding matches `convert_raw_temp` in
`src/hardware/sensors/conversion.rs`:

- **Temperature:** 19-bit signed value left-justified in 24 bits. For 25 °C the
  register is `0x019000`; the driver shifts right by 5 and multiplies by
  0.0078125.
- **Cold junction:** 14-bit signed value left-justified in 16 bits, 1/64 °C per
  LSB. For 25 °C the pair is `0x19 0x00`.
- **SPI:** mode 1 at 1 MHz, matching `init.rs`.

The chip updates the conversion registers immediately when CR0 bit 6 is
written. It does **not** model the 210 ms conversion time, the 50/60 Hz notch
filter, the LTC delay, thermocouple physics, cold-junction compensation, or
automatic open-circuit detection. The firmware still waits its normal
conversion budget, and the fault register is whatever the slider says.

A permanently all-zero conversion frame is rejected by the firmware (H13). The
default chip values are non-zero, so this only matters if you force both
temperature and cold junction to 0.

## 6. F2 details: real SPI and heat-sense

Build F2 without `simulated-sensors`:

```bash
FEATURES=embedded,heat-sense ./scripts/sim-wokwi-prepare.sh
```

Use `FEATURES=embedded` to exercise the real SPI driver and actuators without
the GPIO1 interlock; the heat-sense switch is then ignored. Use F1
(`embedded,simulated-sensors`) only for board/UART/PWM bring-up, because it
bypasses the SPI sensors entirely.

At boot the firmware runs the real MAX31856 self-test on both channels. The
diagram starts at a first-crack-like point: BT 200 °C, ET 220 °C, cold junction
35 °C. The self-test plausibility window is −50…100 °C, so a hot preset logs
`Temperature out of expected range` for both channels; that is expected and
does not fail the channel. Set both sliders to 25 °C before reset for a clean
cold-boot log. If one channel fails to respond, `init_spi_sensors` degrades
that channel (single-channel boot) and aborts only when both fail.

Move the `temperature` slider to see `READ` change. Large steps settle through
the EMA filter over several seconds. Move `fault` to `0x01` to inject an open
circuit; the control layer marks that channel faulted and the safety path
reacts. Restore it to `0` to recover on a subsequent tick.

The GPIO1 slide switch is the heat-sense input: LEFT connects GPIO1 to GND,
which the firmware interprets as current flowing through the SSR. The diagram
starts in the RIGHT position (no current). With the heater off, RIGHT is the
only valid state; LEFT at boot makes the firmware latch `SSR stuck-on` after
`HEAT_PRESENT_MISMATCH_MAX` samples.

When you energize the heater in F2, move the switch LEFT within
`HEAT_MISMATCH_WINDOW_MS` (1.5 s) and move it RIGHT again when the heater is
off. This is a manual current-sense stimulus for the pinout test, not automatic
electrical feedback. If the heater is energized with the switch RIGHT for
longer than 1.5 s, the cross-check latches `heater not detected`; that is
expected behavior, not a simulator bug. Move the switch RIGHT first, then use
the documented recovery path (or reset the simulator) to clear the latch.

In F1/F3 (`simulated-sensors`), `detect_heat_source` is compiled out, so the
switch has no effect and the SPI pins stay idle.

## 7. F0: esp-emulator (experimental, currently blocked)

`scripts/sim-esp-emu.sh` builds the merged image and runs
[esp-emulator](https://github.com/esp-emulator/esp-emulator). The emulator
loads the bootloader and the application, but the firmware then hangs in the
esp-hal software-interrupt path used by esp-rtos:

```text
esp_hal::interrupt::software::SoftwareInterrupt::raise
  <- esp_rtos::run_queue::RunQueue::mark_task_ready
  <- esp_rtos::run_queue::SchedulerState::resume_task
```

`raise()` spins waiting for the interrupt pending bit, which esp-emulator
v0.50.0 does not set on the ESP32-C3. The scheduler never makes progress and
the RTC watchdog resets the chip in a loop. There is no CLI workaround, and the
emulator core is not public source that could be patched here. Replacing
esp-rtos with a bare Embassy executor was rejected: esp-rtos owns the time
driver and is too invasive for a simulation-only feature.

Treat F0 as a reproduction harness, not a working firmware run. Use Wokwi for
actual no-hardware validation.

```bash
ESP_EMU=/path/to/esp-emu ./scripts/sim-esp-emu.sh
CONSOLE=uart0 ESP_EMU=/path/to/esp-emu ./scripts/sim-esp-emu.sh
```

## 8. Troubleshooting

| Symptom | Likely cause / fix |
|---|---|
| Wokwi asks for a license | Use `Wokwi: Request a new License`; local use is free |
| `chip-max31856` is missing | Re-run the prepare script; check `sim/wokwi/chips/max31856.chip.wasm` and the `[[chip]]` paths in `wokwi.toml` |
| Firmware does not boot | Check `libreroaster.bin` exists and `wokwi.toml` points to it; rebuild with the prepare script. If Wokwi rejects the merged image, set `firmware = "libreroaster.elf"` (Wokwi also accepts ELF) |
| No boot log / no `READ` response | Serial monitor must be on UART0 at 115200; another transport may own the session (wait 60 s or reset) |
| `ERR safety_fault Comms idle timeout` after `PREHEAT`/`START`/`OT1` | No inbound command for 15 s while the heater is on. Send `READ` or `STATUS` every 5–10 s; recover with `PID;OFF`/`PREHEAT`/`START`. See §3.5 |
| Green 1 Hz plus red SSR LED after `PREHEAT 200` | Expected: Preheating blink plus heater output. It still needs the 15 s keepalive; see §3.5 |
| F2 self-test says "Temperature out of expected range" | Expected with the hot BT 200 °C / ET 220 °C defaults; the self-test window is −50…100 °C. Not a fault. Set the sliders to 25 °C for a cold-boot log |
| F2 aborts with both channels dead at boot | Check chip VCC/GND and CS/SCK/MOSI/MISO; use F1 to isolate the board/UART first |
| Heat-sense latches with the heater on | Move the switch LEFT within 1.5 s of energizing the heater; RIGHT when off. This is the manual current-sense stimulus |
| `SSR stuck-on` at boot or with the heater off | The switch is LEFT (current) while the heater is off. Move it RIGHT and reset/recover; the diagram starts RIGHT |
| SSR LED does not blink | The output is 5 Hz zero-cross; check LA D0 and the active `OT1`/PID state |
| Fan PWM not visible on an LED | There is no fan LED by design (GPIO9 strapping); use LA D1 |
| VCD file is huge | `vcdFile` is optional; remove it from `wokwi.toml` for long sessions |

## 9. Safety boundary

Simulation can prove that the firmware drives the intended pins, parses the
intended protocol, and follows its control/safety state machine. It cannot
prove mains isolation, SSR thermal behavior, thermocouple accuracy, watchdog
silicon behavior, or failure modes of the real power stage. Do not treat a
green Wokwi session as a hardware safety sign-off; the HIL procedure in
[`DEVELOPMENT.md`](DEVELOPMENT.md) is still required.

## Related documents

- [`DEVELOPMENT.md`](DEVELOPMENT.md) — build, flash, and test workflows.
- [`HARDWARE.md`](HARDWARE.md) — pin map and electrical constraints.
- [`PROTOCOL.md`](PROTOCOL.md) — Artisan command set.
- [`ARTISAN_CONNECTION.md`](ARTISAN_CONNECTION.md) — Artisan setup.
- [`simulated-curve-test.md`](simulated-curve-test.md) — simulated curve
  architecture and host-side tests.
- [`WOKWI_ROAST_PRESET.md`](WOKWI_ROAST_PRESET.md) — F2 temperature preset and
  heat-sense switch runbook.
