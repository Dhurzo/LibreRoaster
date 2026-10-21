# No-Hardware Simulation

This document covers running LibreRoaster without a physical ESP32-C3. It is a
development aid, not a substitute for hardware-in-the-loop (HIL) validation.

The simulation project lives in `sim/wokwi/`. It is deliberately outside the
firmware source tree: no production code is compiled differently for the
simulator, and the only firmware differences are the simulation-only
`simulated-sensors` / `simulated-typical-roast` and `heat-sense` cargo
features.

## 1. What the simulation validates

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

F1 (`embedded,simulated-sensors`) bypasses the MAX31856 driver entirely. It is
the bring-up configuration: boot, UART, LEDs, PWM and the Artisan session.

F2 (`embedded,heat-sense`, without `simulated-sensors`) uses the real MAX31856
driver over the shared SPI bus and the real GPIO1 heat-sense input. The custom
chips in `diagram.json` supply the temperatures. This is the complete pinout
check.

F3 (`embedded,simulated-typical-roast`) is F1 with a guard-safe typical roast
curve. It starts at a 200 C charge, drops to a ~100 C turning point, then
rises with positive BT slopes at or below ~0.33 C/s. `START` can stay in
`Heating` (solid green LED) without tripping the RoR guard. The MAX31856
chips and the heat-sense switch are ignored, exactly as in F1.

## 2. Prerequisites

- Rust target `riscv32imc-unknown-none-elf` (`rustup target add ...`).
- `espflash` on `PATH` (`cargo install espflash --locked`).
- VS Code with the Wokwi extension. The extension offers a free license via
  `Wokwi: Request a new License`; a paid plan is not required for local use.
- `wokwi-cli` to compile the custom chip (`max31856.chip.wasm`) and, optionally,
  to run the scripted scenario in §3.4. The prepare script uses it; the wasm is
  not committed, so a fresh clone needs this step. The first compile downloads
  WASI-SDK and `wokwi-api.h`; both are cached locally.

Wokwi CI (`WOKWI_CLI_TOKEN`) is not required for the VS Code workflow. It is
optional and only used by the scripted scenario runner in §3.4.

## 3. Quick start

```bash
# F1: bring-up with simulated temperatures
./scripts/sim-wokwi-prepare.sh

# or F2: real MAX31856 driver + heat-sense over GPIO1
FEATURES=embedded,heat-sense ./scripts/sim-wokwi-prepare.sh

# or F3: synthetic "typical roast" that does not trip the RoR guard
FEATURES=embedded,simulated-typical-roast ./scripts/sim-wokwi-prepare.sh
```

The script builds the firmware, creates a merged flash image with
`espflash save-image --chip esp32c3 --merge`, copies the ELF, and compiles the
custom chip when `wokwi-cli` is available. It writes:

```
sim/wokwi/libreroaster.bin      merged image (firmware)
sim/wokwi/libreroaster.elf      symbols for the debugger
sim/wokwi/chips/max31856.chip.wasm
```

Open `sim/wokwi/` in VS Code and run `Wokwi: Start Simulator`. Open the serial
monitor at **115200 baud**. The firmware logs to UART0 and the same UART is the
Artisan transport, so the monitor both shows logs and sends commands.

First commands:

```
CHAN;0
UNITS;C
READ
```

`READ` returns a TC4-style line with BT/ET temperatures. In F1 the values come
from the `simulated-sensors` curve; in F2 they come from the MAX31856 sliders.

To see continuous `#` telemetry, send `STREAM;ON` (it is off by default). To
exercise the actuators without tripping the rate-of-rise guard:

```
PREHEAT 200
OT1;100
OT1;0
PID;OFF
```

`PREHEAT 200` shows the 1 Hz green LED (Preheating); `OT1;100` drives the SSR
LED. Do **not** use `START` with the default F1 curve just to test the LEDs:
the curve's ramp exceeds the RoR guard and the firmware correctly latches
`Bean temperature rate-of-rise exceeded`. Use the F3 build (§3.3) when you
want a `START`/`Heating` test without the latch.

> Keepalive: once the heater is energized (`PREHEAT`, `START`, `OT1` or
> `PID;ON`), send `READ` or `STATUS` every 5-10 s. Fifteen seconds without an
> inbound command trips the comms-idle safety latch and turns the heater off;
> `STREAM;ON` does not count. See §3.5.

In F2, `START`/`PREHEAT` need a profile only if you want profile following;
manual `OT1` and `UP` are enough for a pinout check. `PID;OFF` returns to
manual control. `STOP` stops the roast and the heater.

> UART session rule: the first valid command on either transport claims the
> Artisan session, and output goes to that transport. If another transport
> claimed it, wait 60 s of command inactivity (the multiplexer resets to
> `None`) or reset the board. Keep sending commands during a session; while
> the heater is energized, the 15 s `COMMS_IDLE_TIMEOUT_MS` safety latch
> applies before the 60 s multiplexer timeout.

### 3.1 F1 test checklist (simulated sensors)

1. Build and open the default F1 image:

   ```bash
   ./scripts/sim-wokwi-prepare.sh
   ```

   Start `sim/wokwi/` in VS Code and open the serial monitor at 115200.
2. The release image logs at `Warn`, so the application's `info!` boot lines
   are not shown. The serial monitor still shows the ESP-IDF bootloader ending
   with `Loaded app from partition`; send `READ` to confirm the application is
   alive. There must be no MAX31856 self-test lines and no SPI initialisation
   failure.
3. Send `CHAN;0`, `UNITS;C` and `READ`. With PID disabled the reply is
   `0.0,<ET>,<BT>,0.0,0.0`; `AMB` is 0 because simulated mode has no
   cold-junction hardware. Near boot the values are around 25/25.
4. Wait about 30 s and send `READ` again. The default medium curve is
   time-based from boot: about BT 80 / ET 100 at 30 s and BT 120 / ET 150 at
   60 s (the EMA filter may lag slightly). `START` does not restart the curve
   (and it arms the RoR guard; see step 6); reset the simulator to restart at
   25/25.
5. Optional continuous trace: send `STREAM;ON` (`#OK`), then watch the `#`
   lines. While the heater is energized, send `READ` or `STATUS` every
   5–10 s: `COMMS_IDLE_TIMEOUT_MS` is 15 s and trips the comms-idle safety
   latch. In Idle (heater off) the multiplexer drops the transport only after
   60 s without commands.
6. LED and actuator check without arming the RoR guard:
   - Green LED: `PREHEAT 200` gives the 1 Hz Preheating blink; `PID;OFF`
     returns to Idle and turns it off.
   - SSR LED: `OT1;100` lights the red LED and LA D0; `OT1;0` turns it off.
   - Do **not** use `START` with the default curve: it enters Heating, arms
     the RoR guard, and the curve's ramp correctly latches
     `Bean temperature rate-of-rise exceeded` (green LED 4 Hz). Use a gentle
     custom curve (slope below 0.75 C/s) if you need the solid
     Heating/Stable pattern, or use the F3 build in §3.3.
   The heat-sense switch and the MAX31856 chips are ignored; the SPI pins
   stay idle.

### 3.2 F2 test checklist (real SPI and heat-sense)

1. Build F2:

   ```bash
   FEATURES=embedded,heat-sense ./scripts/sim-wokwi-prepare.sh
   ```

   Open the simulator and the serial monitor at 115200.
2. At boot expect one MAX31856 self-test block per channel, with CR0=0x11,
   MASK=0x00, converted temperature 200 C (BT) / 220 C (ET), and the
   `Temperature out of expected range` warning for both. That warning is
   expected with the hot preset. There must be no `boot self-test failed`,
   `channel degraded`, or both-channels-dead abort.
3. Send `READ`. Expect approximately `35.0,220.0,200.0,0.0,0.0`
   (`AMB,ET,BT`).
4. Move the BT/ET `temperature` sliders and send `READ` again; the values
   follow within one or two control ticks while `AMB` stays at the
   cold-junction 35 C. Set `fault` to 0x01 on one chip to inject an
   open-circuit fault, watch the log/`STATUS`, then restore 0 to recover; the
   other channel must stay healthy.
5. Check SPI with the logic analyzer: CS GPIO4/GPIO3 pulse once per control
   tick, SCK GPIO6 runs at 1 MHz, MOSI GPIO7 shows the register writes, and
   MISO GPIO5 returns data.
6. Heat-sense check: start with the switch RIGHT (no current). Send
   `OT1;100`, move the switch LEFT within 1.5 s, then turn the heater off and
   move it RIGHT again; there must be no latch. Negative checks: leave the
   switch RIGHT for more than 1.5 s with the heater on (expect `heater not
   detected`), and boot with the switch LEFT while the heater is off (expect
   `SSR stuck-on`). Recover by moving the switch RIGHT and using `PID;OFF`,
   `START`, `PREHEAT`, or a reset. Full runbook:
   [`WOKWI_ROAST_PRESET.md`](WOKWI_ROAST_PRESET.md).
7. Actuator check: the SSR LED brightness follows `OT1`; `STOP` cuts the
   heater. The fan has no LED by design; probe LA D1.

`FEATURES=embedded` runs the same SPI and actuator checks without the GPIO1
interlock.

### 3.3 F3 test checklist (guard-safe typical roast)

F3 is F1 with a charge-dip curve designed to stay below the RoR guard. Use it
when you want to test `START` → `Heating` (solid green LED) without editing
the curve.

1. Build F3:

   ```bash
   FEATURES=embedded,simulated-typical-roast ./scripts/sim-wokwi-prepare.sh
   ```

   Open the simulator and the serial monitor at 115200.
2. The release image logs at `Warn`, so the application's `info!` boot lines
   are not shown. The serial monitor still shows the ESP-IDF bootloader ending
   with `Loaded app from partition`; send `READ` to confirm the application is
   alive. There must be no MAX31856 self-test lines and no SPI initialisation
   failure. The exact charge is `0.0,220.0,200.0,0.0,0.0`
   (`AMB,ET,BT`) at t=0; because the curve starts when the source is built,
   the first `READ` after a multi-second boot already shows the charge dip
   (for example `0.0,218.1,184.4,0.0,0.0` after ~6 s).
3. Send `START`. The green LED must be **solid** (Heating). The log must not
   contain `BT RoR`, `safety_fault`, or `Emergency shutdown`. The first slew
   step can log `SAFETY FAN-FLOOR: heater at 5.0% with fan at 0.0% — raising
   fan to minimum 20%`; that is the normal fan-floor interlock (a warning,
   not a latch), not a fault.
4. Over the first minute `READ` drops toward ~100 C BT / 205 C ET (turning
   point), then rises slowly to ~214 C BT / 243 C ET at 600 s. BT keeps
   falling with the heater on: the F3 curve is time-based, not a closed-loop
   plant, and the heater does not move it. Every positive BT slope is at or
   below ~0.33 C/s, so the RoR guard stays armed but silent. Keep the Artisan
   session alive with `READ` or `STATUS` every 5–10 s while Heating;
   `COMMS_IDLE_TIMEOUT_MS` is 15 s. In Idle the multiplexer's 60 s timeout
   applies instead.
5. `PID;OFF` ends the test and returns to Idle; reset the simulator to restart
   at the 200/220 charge. `PREHEAT 200` is an alternative: green at 1 Hz plus
   the red SSR LED, with the same 15 s comms-idle keepalive. The MAX31856
   chips and the heat-sense switch are ignored.

### 3.4 Scripted keepalive scenario (Wokwi CLI)

The VS Code extension can start the simulator but cannot run scenario files.
Automated serial input requires `wokwi-cli` and a Wokwi CI token (the local
VS Code license is not a CI token):

```bash
curl -L https://wokwi.com/ci/install.sh | sh   # installs wokwi-cli
export WOKWI_CLI_TOKEN=wok_...                 # https://wokwi.com/dashboard/ci
FEATURES=embedded,simulated-typical-roast ./scripts/sim-wokwi-prepare.sh
./scripts/sim-wokwi-scenario.sh
```

`sim/wokwi/scenarios/f3-roast.yaml` waits for the bootloader's
`Loaded app from partition` line, gives the application 6 s to initialize,
then sends `PREHEAT 200` and two `READ`s 8 s apart (16 s total, crossing the
15 s comms-idle window); it then sends `PID;OFF` and `START`, polls `READ`
twice 4 s apart to keep the roast alive, and finishes with `PID;OFF`. It
asserts that GPIO8 (the green status LED) is high in `Heating` and low after
the final `PID;OFF`. The runner fails on `safety_fault`, `Emergency shutdown`,
`BT RoR`, or `Control update error`. The scenario is sized to finish inside
Wokwi's 5-minute free API session. The scenario path is relative to
`sim/wokwi`; the default is `scenarios/f3-roast.yaml`.

This is the executable version of the 15 s comms-idle rule: every `READ`
refreshes `last_command_received_at_ms`, so the scenario must finish without a
latch. The scenario needs a Wokwi CI token and is not part of the default
local workflow; the VS Code GUI checklist in §3.3 remains the fallback.

### 3.4.1 F1/F2/F3 scenario matrix and curve selection

Each scripted scenario requires the firmware image built with the matching
feature set:

| Build | Feature set | Scenario | What it checks |
|---|---|---|---|
| F1 | `embedded,simulated-sensors` | `scenarios/f1-smoke.yaml` | Handshake, `READ`, `PREHEAT 200`, the 20 % fan floor, and the 15 s keepalive. It does **not** send `START` (the default medium curve would latch the RoR guard). |
| F2 | `embedded,heat-sense` | `scenarios/f2-spi.yaml` | Handshake, initial `35.0,220.0,200.0`, then slider changes to `35.0,230.0,250.0`. It does **not** energize the heater or move the heat-sense switch. |
| F3 | `embedded,simulated-typical-roast` | `scenarios/f3-roast.yaml` | `PREHEAT`, 15 s keepalive, `START`/`Heating`, GPIO8 high, then `PID;OFF` and GPIO8 low. |

```bash
# F1
FEATURES=embedded,simulated-sensors ./scripts/sim-wokwi-prepare.sh
./scripts/sim-wokwi-scenario.sh scenarios/f1-smoke.yaml

# F2
FEATURES=embedded,heat-sense ./scripts/sim-wokwi-prepare.sh
./scripts/sim-wokwi-scenario.sh scenarios/f2-spi.yaml

# F3 (default scenario/log)
FEATURES=embedded,simulated-typical-roast ./scripts/sim-wokwi-prepare.sh
./scripts/sim-wokwi-scenario.sh
```

The runner writes `target/wokwi-<scenario-name>.log` unless a second argument
overrides it. It fails on `safety_fault`, `Emergency shutdown`, `BT RoR`, or
`Control update error`. The image must match the scenario: rebuilding is
required when switching feature set. Wokwi's free API session is limited to
5 minutes, so keep scenarios short.

**Switching curves is compile-time, not runtime.** There is no serial command
or Wokwi setting that selects a roast curve:

- F1 uses `RoastCurve::default_medium_roast()` (the default of
  `embedded,simulated-sensors`).
- F3 uses `RoastCurve::typical_roast()` via `embedded,simulated-typical-roast`.
- F2 does not use a curve; the MAX31856 slider values are the temperatures.
  Edit them in `sim/wokwi/diagram.json` or move them in the Wokwi UI.
- `RoastCurve::light_roast()` has no Cargo feature. It is available from code
  and host tests only; see [`simulated-curve-test.md`](simulated-curve-test.md)
  for custom curves and noise.

### 3.5 Why PREHEAT/START latches after 15 s

There are two different inactivity timers:

| Timer | Value | Armed when | Effect |
|---|---:|---|---|
| Safety `COMMS_IDLE_TIMEOUT_MS` | 15 s | heater energized or roast active | emergency latch: heater 0 %, fan 100 %, `ERR safety_fault Comms idle timeout` |
| Multiplexer `IDLE_TIMEOUT_SECS` | 60 s | session Idle (heater off) | output transport is released; no safety latch |

`PREHEAT 200`, `START`, `OT1` and `PID;ON` all energize the heater, so the
15 s safety timer is armed. The green LED shows 1 Hz (Preheating) and the red
SSR LED follows the heater while it is on. If no inbound command arrives for
more than 15 s, the log looks like this:

```text
WARN - SAFETY COMMS-IDLE: no command for 15050ms (>15000ms) — emergency shutdown
ERROR - Emergency shutdown: Comms idle timeout
ERR safety_fault Comms idle timeout
```

That is expected supervision, not a PREHEAT bug, a RoR trip or the FAN-FLOOR
warning. The FAN-FLOOR warning (`heater at 5.0% with fan at 0.0%`) is the
normal first slew step. After the comms-idle latch the heater is forced off
(red LED off); the green LED switches to the 4 Hz Error pattern. The roast
cannot be recovered without an explicit operator command.

Recovery and keepalive:

- Recover with `PID;OFF`, `PREHEAT` or `START`.
- While the heater is energized, send `READ` or `STATUS` every 5-10 s.
  `STREAM;ON` is output-only and does **not** refresh the timer.
- Artisan polls `READ` once per sampling interval (default 2 s), so a normal
  Artisan roast keeps the timer refreshed. If Artisan is closed, loses the
  serial link, or is configured above 15 s, the latch is intentional.
- The scripted scenarios in §3.4/§3.4.1 send `READ` within the 15 s window and
  fail if this latch appears.

## 4. Pin checklist

Wokwi board: `board-esp32-c3-devkitm-1`. The wiring is in
`sim/wokwi/diagram.json`.

| Signal | GPIO | Wokwi part / probe | Expected observation |
|---|---:|---|---|
| UART0 TX | 21 | serial monitor; LA D7 | Boot log and command responses |
| UART0 RX | 20 | serial monitor | Commands accepted |
| Status LED | 8 | green LED + 330 ohm | Off in Idle, 1 Hz in Preheating, solid in Heating/Stable, 4 Hz on Error |
| SSR control | 10 | red LED + 220 ohm; LA D0 | 5 Hz zero-cross bursts; brightness follows duty |
| Fan PWM | 9 | **LA D1 only** | 25 kHz PWM; no direct LED (strapping pin) |
| BT chip select | 4 | LA D2 | Pulses once per control tick in F2; idle in F1 |
| ET chip select | 3 | LA D3 | Pulses once per control tick in F2; idle in F1 |
| SPI SCLK | 6 | LA D4 | 1 MHz clock during transfers in F2 |
| SPI MOSI | 7 | LA D5 | Register writes (0x80/0x81/0x82, 0x51 one-shot) |
| SPI MISO | 5 | LA D6 | Register/conversion data in F2 |
| Heat-sense | 1 | slide switch to GND | F2 only: RIGHT = no current (start here), LEFT = current |
| 3V3 / GND | - | board pins | Power for both chips and the LED circuits |

GPIO9 is an ESP32-C3 strapping pin. A direct LED to GND would pull it low at
reset; the diagram therefore leaves it on the logic analyzer only. On real
hardware, any external fan driver must not pull GPIO9 low at reset, and a
10 kOhm pull-up to 3.3 V is mandatory (see `docs/HARDWARE.md`).

GPIO8 is ignored for normal SPI boot (only the download-mode combination
GPIO8=HIGH + GPIO9=LOW matters), so the status LED is safe in this diagram.

## 5. Custom MAX31856 chip

The chip model is `sim/wokwi/chips/max31856.chip.c` with its pin/control
metadata in `max31856.chip.json`. Two instances are wired: BT on CS GPIO4 and
ET on CS GPIO3, sharing SCLK/MOSI/MISO.

### Controls

| Control | Range | Default | Meaning |
|---|---|---|---|
| `temperature` | -200..1350 C | BT 200, ET 220 | Thermocouple temperature reported in LTCBH/LTCBM/LTCBL |
| `coldJunction` | -20..85 C | 35 | Cold-junction temperature reported in CJTH/CJTL |
| `fault` | 0..255 | 0 | Fault register value; 0 = healthy, bit 0 = open circuit |

### Registers

| Address | Register | Behavior |
|---:|---|---|
| 0x00 | CR0 | Stored; bit 6 (1SHOT) updates the conversion block |
| 0x01 | CR1 | Stored; read-back verification works |
| 0x02 | MASK | Stored |
| 0x0A | CJTH | High byte of left-justified 14-bit CJ value |
| 0x0B | CJTL | Low byte |
| 0x0C | LTCBH | High byte of left-justified 24-bit temperature |
| 0x0D | LTCBM | Middle byte |
| 0x0E | LTCBL | Low byte |
| 0x0F | SR | Current `fault` control value |

The driver writes CR0=0x11, CR1=0x03 and MASK=0x00 at boot, then runs its
self-test (including the 0x13 write/read-back probe and a 0x51 one-shot). The
chip supports that sequence. Reads use the 6-byte conversion block starting at
0x0A; a single register read is also supported.

Encoding matches `convert_raw_temp` in
`src/hardware/sensors/conversion.rs`:

- Temperature: 19-bit signed value left-justified in 24 bits. For 25 C the
  register is `0x019000`; the driver shifts right by 5 and multiplies by
  0.0078125.
- Cold junction: 14-bit signed value left-justified in 16 bits, 1/64 C per LSB.
  For 25 C the pair is `0x19 0x00`.
- SPI mode 1, 1 MHz, matching `init.rs`.

The chip updates the conversion registers immediately when CR0 bit 6 is
written. It does **not** model the 210 ms conversion time, the 50/60 Hz notch
filter, the LTC delay, thermocouple physics, cold-junction compensation, or
automatic open-circuit detection. The firmware still waits its normal
conversion budget and the fault register is whatever the slider says.

A permanently all-zero conversion frame is rejected by the firmware (H13). The
default chip values are non-zero, so this only matters if you force both
temperature and cold junction to 0.

## 6. F2: real SPI and heat-sense

Build F2 without `simulated-sensors`:

```bash
FEATURES=embedded,heat-sense ./scripts/sim-wokwi-prepare.sh
```

To exercise the real SPI driver and the actuators without enabling the GPIO1
interlock, use `FEATURES=embedded` instead; the heat-sense switch is then
ignored. Use the default F1 (`embedded,simulated-sensors`) only for
board/UART/PWM bring-up, because it bypasses the SPI sensors entirely.

At boot the firmware runs the real MAX31856 self-test on both channels. The
serial log shows the CR0/MASK values, the conversion temperature and the
fault byte. The diagram starts at a first-crack-ish roast point: BT 200 C,
ET 220 C, cold junction 35 C. The self-test's plausibility window is the
ambient range -50..100 C, so a hot default logs "Temperature out of expected
range" for both channels; that is expected and does not fail the channel. Set
both sliders to 25 C before reset if you want a clean cold-boot log. If one
channel fails to respond at all, `init_spi_sensors` degrades that channel
(single-channel boot) and only aborts when both fail.

The preset rationale, the exact `READ` shape, and the switch recovery runbook
are in [`WOKWI_ROAST_PRESET.md`](WOKWI_ROAST_PRESET.md).

Move the `temperature` slider to see `READ` change. Move the `fault` slider to
0x01 to inject an open-circuit fault; the control layer will mark that channel
faulted and the safety path will react. Set it back to 0 to recover on a
subsequent tick.

The GPIO1 slide switch is the heat-sense input: LEFT connects GPIO1 to GND,
which the firmware interprets as current flowing through the SSR. The diagram
starts it in the RIGHT position (no current). With the heater off, RIGHT is
the only valid state; LEFT at boot makes the firmware correctly latch
`SSR stuck-on` after `HEAT_PRESENT_MISMATCH_MAX` samples.

When you energize the heater in F2, move the switch LEFT within the
`HEAT_MISMATCH_WINDOW_MS` window (1.5 s) and move it RIGHT again when the
heater is off. This is the manual current-sense stimulus for the pinout test,
not automatic electrical feedback. If the heater is energized with the switch
RIGHT for longer than 1.5 s, the cross-check latches
`heater not detected`; that is expected behavior, not a simulator bug. Move
the switch RIGHT first, then use the documented recovery path (or reset the
simulator) to clear the latch.

In F1 (`simulated-sensors`), `detect_heat_source` is compiled out, so the
switch has no effect and the SPI pins stay idle.

## 7. F0: esp-emulator (experimental, currently blocked)

`scripts/sim-esp-emu.sh` builds the merged image and runs
[esp-emulator](https://github.com/esp-emulator/esp-emulator). The emulator
loads the bootloader and the application, but the firmware then hangs in the
esp-hal software-interrupt path used by esp-rtos:

```
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
| `ERR safety_fault Comms idle timeout` after `PREHEAT`/`START`/`OT1` | No inbound command for 15 s while the heater is on. Send `READ` or `STATUS` every 5-10 s; recover with `PID;OFF`/`PREHEAT`/`START`. See §3.5 |
| Green 1 Hz plus red SSR LED after `PREHEAT 200` | Expected: Preheating blink plus heater output. It still needs the 15 s keepalive; see §3.5 |
| F2 self-test says "Temperature out of expected range" | Expected with the hot BT 200 / ET 220 defaults; the self-test window is -50..100 C. Not a fault. Set the sliders to 25 C for a cold-boot log |
| F2 aborts with both channels dead at boot | Check chip VCC/GND and CS/SCK/MOSI/MISO; use F1 to isolate the board/UART first |
| Heat-sense latches with the heater on | Move the switch LEFT within 1.5 s of energizing the heater; RIGHT when off. This is the manual current-sense stimulus |
| `SSR stuck-on` at boot or with the heater off | The switch is LEFT (current) while the heater is off. Move it RIGHT and reset/recover; the diagram now starts RIGHT |
| SSR LED does not blink | The output is 5 Hz zero-cross; check LA D0 and the active `OT1`/PID state |
| Fan PWM not visible on an LED | There is no fan LED by design (GPIO9 strapping); use LA D1 |
| VCD file is huge | `vcdFile` is optional; remove it from `wokwi.toml` for long sessions |

## 9. Safety boundary

Simulation can prove that the firmware drives the intended pins, parses the
intended protocol and follows its control/safety state machine. It cannot prove
mains isolation, SSR thermal behavior, thermocouple accuracy, watchdog silicon
behavior or failure modes of the real power stage. Do not treat a green Wokwi
session as a hardware safety sign-off; the HIL procedure in
`docs/DEVELOPMENT.md` is still required.

## Related documents

- [`docs/DEVELOPMENT.md`](DEVELOPMENT.md) - build, flash and test workflows.
- [`docs/HARDWARE.md`](HARDWARE.md) - pin map and electrical constraints.
- [`docs/PROTOCOL.md`](PROTOCOL.md) - Artisan command set.
- [`docs/ARTISAN_CONNECTION.md`](ARTISAN_CONNECTION.md) - Artisan setup.
- [`docs/simulated-curve-test.md`](simulated-curve-test.md) - host-side
  simulated curve verification.
