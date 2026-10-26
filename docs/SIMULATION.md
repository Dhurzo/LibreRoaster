# No-Hardware Simulation

**Last updated:** 2026-10-26

This guide explains how to run LibreRoaster without a physical ESP32-C3, what
the Wokwi simulation can and cannot prove, and how to move between the
supported test builds.

The simulation project lives in `sim/wokwi/`. It is deliberately isolated from
the firmware source tree: the firmware is built for the simulator with the
same `embedded` code path, plus simulation-only Cargo features
(`simulated-sensors`, `simulated-plant`, `simulated-typical-roast`, or
`simulated-web-medium-roast`), or the `heat-sense` feature for the real-SPI F2 build.
`simulated-plant` implies `simulated-sensors` and replaces the time curve with a heater/fan-driven
`ThermalPlant`. The two roast-profile features are mutually exclusive, and `heat-sense` cannot be
combined with `simulated-sensors`.

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
| F4 | `embedded,simulated-web-medium-roast` | Published 696 s bean curve; ET = BT + 20 °C | Compiled out | Artisan end-to-end roast with a public profile |
| F5 | `embedded,simulated-plant` | Closed-loop `ThermalPlant` driven by control-loop heater/fan output | Compiled out | PID/actuator and `SIM` closed-loop checks |
| F0 | `embedded` + `esp-emulator` | Host emulator | Not modeled | Bootloader/app-load reproduction only (blocked) |

### Coverage matrix

| Area | Wokwi (F1-F5) | esp-emu (F0) | Host tests |
|---|---|---|---|
| Boot, UART0, Artisan protocol | Yes | Bootloader/app load only | Protocol unit tests |
| Status LED (GPIO8) | Yes | No (blocked) | Pattern logic |
| SSR PWM (GPIO10) | Yes (LED + logic analyzer) | No (blocked) | State machine |
| Fan PWM (GPIO9) | Logic analyzer only | No (blocked) | State machine |
| MAX31856 SPI (CS/SCK/MOSI/MISO) | Yes in F2 | No (no external SPI model) | Scripted SPI |
| Closed-loop heater/fan -> temperature | Yes in F5 (`simulated-plant`) | No | `tests/thermal_closed_loop.rs` |
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
  esp-emulator path. It is not a working firmware run; see §6.

## 2. Prerequisites

- Rust target and `espflash`: follow [`DEVELOPMENT.md`](DEVELOPMENT.md) §2.
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
   A single failed channel degrades to single-channel operation; boot aborts
   only when both channels fail. Set both sliders to 25 °C before reset for a
   clean cold-boot log.

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
   `Heat source not detected`), and boot with the switch LEFT while the heater is
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

### 3.3.1 F4 test checklist (published medium-roast curve)

F4 replaces the synthetic curve with a real bean-probe trace from the public
[`luispoveda93/coffee-roast-profiles`](https://huggingface.co/datasets/luispoveda93/coffee-roast-profiles)
dataset (row `real-zenodo-000`, source Zenodo record
[21383046](https://zenodo.org/records/21383046), CC-BY-4.0). The source publishes BT only,
so ET is derived as BT + 20 °C. It is a 25-point trace spanning 696 s.

1. Build F4 and open the simulator:

   ```bash
   FEATURES=embedded,simulated-web-medium-roast ./scripts/sim-wokwi-prepare.sh
   ```

2. The curve starts when the firmware boots, not on `START`. For a full-profile run, restart the
   simulator immediately before starting Artisan recording. The expected endpoints are:

   | Time | BT | ET |
   |---:|---:|---:|
   | 0 s | 183.8 °C | 203.8 °C |
   | 60 s (turning point) | 165.4 °C | 185.4 °C |
   | 696 s (drop) | 207.4 °C | 227.4 °C |

   The steepest positive BT slope is 8.77 °C/min, below the 45 °C/min soft RoR limit.

3. Connect the real Artisan app with §3.6. Use **ON** to confirm the handshake and live ET/BT,
   then **START** to record. Keep the simulation tab visible; Wokwi throttles the emulated clock
   when the window is hidden or rendered off-screen, which stretches the roast on Artisan's
   real-time axis.

4. Let the run reach 696 s, then stop recording. Artisan should show the turning point near
   1:00 and the drop near 11:36. Compare raw `READ` values with the bridge trace
   (`--trace /tmp/artisan-trace.log`) as well as the plotted curve; Artisan's filters and
   smoothing can hide sub-degree differences.

5. F4 is still open-loop: heater and fan commands do not change the published curve. ET is
   synthesized, not measured. Treat this as an Artisan/protocol/profile-plumbing test, not
   thermal validation.

### 3.4 Scripted scenarios (Wokwi CLI)

The VS Code extension can start the simulator but cannot run scenario files.
Automated serial input requires `wokwi-cli` and a Wokwi CI token (the local
VS Code license is not a CI token):

```bash
curl -fsSL https://wokwi.com/ci/install.sh | sh -s -- 0.29.0   # pinned wokwi-cli
export WOKWI_CLI_TOKEN=wok_...                 # https://wokwi.com/dashboard/ci
```

### 3.4.1 Scenario manifest and runner

`sim/wokwi/scenarios/manifest.json` is the source of truth for automated
scenarios. Each entry declares the scenario file, required Cargo features,
Wokwi timeout, kind (`happy` or `latch`), and the log patterns to expect or
forbid. The runner builds each feature set once, verifies that
`sim/wokwi/build-meta.json` matches the requested features and firmware hash,
runs Wokwi, then verifies the serial log.

| ID | Build | Scenario | What it checks |
|---|---|---|---|
| `f1-smoke` | F1 `embedded,simulated-sensors` | `scenarios/f1-smoke.yaml` | Handshake, `READ`, `PREHEAT 200`, and the 20 % fan floor. It does **not** send `START` (the default curve would latch the RoR guard). |
| `f2-spi` | F2 `embedded,heat-sense` | `scenarios/f2-spi.yaml` | Real MAX31856 SPI path: initial `35.0,220.0,200.0`, then slider changes to `35.0,230.0,250.0`. Does not energize the heater. |
| `f3-roast` | F3 `embedded,simulated-typical-roast` | `scenarios/f3-roast.yaml` | `PREHEAT`, 15 s keepalive, `START`/`Heating`, GPIO8 high, then `PID;OFF` and GPIO8 low. |
| `f4-short` | F4 `embedded,simulated-web-medium-roast` | `scenarios/f4-short.yaml` | Short smoke of the public 696 s curve. It is not a full F4 recording. |
| `f1-ror` | F1 `embedded,simulated-sensors` | `scenarios/f1-ror.yaml` | `START` on the default medium curve must latch `rate-of-rise exceeded`. |
| `f3-comms-idle` | F3 `embedded,simulated-typical-roast` | `scenarios/f3-comms-idle.yaml` | Heater armed, then no inbound command for more than 15 s; expects `Comms idle timeout`. |
| `f2-overtemp` | F2 `embedded,heat-sense` | `scenarios/f2-overtemp.yaml` | BT slider above 260 °C; expects `Temperature exceeds valid range`. |
| `f2-sensor-fault` | F2 `embedded,heat-sense` | `scenarios/f2-sensor-fault.yaml` | BT MAX31856 fault bit set to open-circuit; expects `Sensor fault`. |
| `f2-heatsense` | F2 `embedded,heat-sense` | `scenarios/f2-heatsense.yaml` | `OT1;100` with GPIO1 switch RIGHT (no current); expects `Heat source not detected` and forced SSR output 0 %. It is a heat-source interlock, not a full emergency latch. |
| `f5-plant` | F5 `embedded,simulated-plant` | `scenarios/f5-plant.yaml` | `OT1;90` drives `ThermalPlant`; `SIM;STATUS` shows `plant=1`, rising BT, and the applied heater/fan. Unlike F1/F3/F4, temperature follows actuator output. |
| `f5-et-overtemp` | F5 `embedded,simulated-plant` | `scenarios/f5-et-overtemp.yaml` | `SIM;TEMP;25;310` forces ET above the 300 °C ET threshold; expects `Temperature exceeds valid range`. |
| `f5-sensor-fault` | F5 `embedded,simulated-plant` | `scenarios/f5-sensor-fault.yaml` | `SIM;FAULT;1;0` injects an open-circuit bit after a successful `SIM;RESET`; expects `Sensor fault`. |

### 3.4.2 `SIM` control commands

Builds with `simulated-sensors` accept a `SIM` command family for negative
tests and closed-loop inspection. Without that feature the firmware replies
`ERR sim_unavailable`.

| Command | Effect |
|---|---|
| `SIM;STATUS` | Emit `#SIM;PLANT` with BT/ET, applied heater/fan, `plant=0/1`, `frozen=0/1`, and BT/ET fault masks as `0xNN`. Allowed while a safety latch is active. |
| `SIM;RESET` | Reset the source (curve time, plant, filters, overrides, fault masks). Only accepted from `Idle`; recover from a latch with `PID;OFF` first. |
| `SIM;FAULT;<bt_mask>;<et_mask>` | Inject MAX31856 fault-register bits; decimal or `0x` hex. Bit 0 is open-circuit. |
| `SIM;CHARGE;<drop_c>` | Inject a cold-bean drop into the plant (0…300 °C); returns `ERR sim_no_plant` without a plant. |
| `SIM;TEMP;<bt>;<et>` | Hard-override reported temperatures (0…600 °C), e.g. to exercise ET over-temp. |
| `SIM;TEMP;OFF` | Clear the temperature override. |
| `SIM;FREEZE;ON` / `SIM;FREEZE;OFF` | Freeze/unfreeze evolution while retaining the last reading. |

`SIM` commands are deliberate test controls, not an Artisan production
surface. They are included only in simulation features and must not be used
as a safety mechanism. `SIM;STATUS` is the only `SIM` command allowed while
`fault_condition` is latched.

```bash
./scripts/sim-wokwi-check.sh --list
./scripts/sim-wokwi-check.sh --build-only
./scripts/sim-wokwi-check.sh --scenario f1-smoke --scenario f4-short
./scripts/sim-wokwi-check.sh --all
```

Artifacts are written to `target/wokwi/`: `<id>.log`, `<id>.verify.json`,
`<id>.json`, and `summary.json`. `sim/wokwi/build-meta.json` is generated and
ignored by git.

The old wrapper still works for one scenario:

```bash
./scripts/sim-wokwi-scenario.sh scenarios/f1-smoke.yaml
./scripts/sim-wokwi-scenario.sh scenarios/f1-smoke.yaml /tmp/wokwi-logs
```

The optional second argument is a **log directory**; the file is named
`<scenario-id>.log` inside it.

`f3-roast` waits for `Loaded app from partition`, gives the application 6 s to
initialize, then sends `PREHEAT 200` and two `READ`s 8 s apart (16 s total,
crossing the 15 s window). It then sends `PID;OFF` and `START`, polls `READ`
twice 4 s apart, and finishes with `PID;OFF`. It asserts that GPIO8 is high in
`Heating` and low after the final `PID;OFF`.

The VS Code GUI checklists in §3.1–§3.3 remain the fallback when no CI token is
available. Wokwi CLI 0.29.0 emits an `invalid-attribute` warning for the
`wokwi-slide-switch` `value` attribute, but the observed default is RIGHT and
the heat-sense scenario passes; verify the switch position in the VS Code GUI
before using it manually.

CI coverage is split deliberately:

- `.github/workflows/ci.yml` builds F3/F4/F5 and rejects incompatible feature
  combinations without needing a Wokwi token.
- `.github/workflows/simulation.yml` runs the full manifest on manual dispatch
  or nightly when `WOKWI_CLI_TOKEN` is configured; otherwise it skips with a
  notice.

Local verification on 2026-10-26 passed F1-F4 (9/9) and the three F5
scenarios individually (3/3) with Wokwi CLI 0.29.0 before the MAX31856
chip-fidelity change. The subsequent combined 12-scenario sweep was
interrupted after 3/12 when the local Wokwi Free-plan monthly CI quota ran out;
re-run `./scripts/sim-wokwi-check.sh --all` with remaining quota before
treating the full matrix as green. `f4-short` remains a smoke check, not a
substitute for the full 696 s F4 recording.

> **Curve selection is compile-time.** There is no serial command or Wokwi
> setting that selects a roast curve. F1 uses `default_medium_roast()`; F3 uses
> `typical_roast()` via `embedded,simulated-typical-roast`; F4 uses the public
> `web_medium_roast()` trace via `embedded,simulated-web-medium-roast`; F5 uses
> `simulated-plant` (the plant starts at ambient and is actuator-driven); F2 does
> not use a curve at all. `simulated-typical-roast` and
> `simulated-web-medium-roast` are mutually exclusive. `light_roast()` has no
> Cargo feature and is only available from code or host tests. See
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

### 3.6 Connect the real Artisan application

Wokwi for VS Code can expose UART0 over RFC2217, but Artisan opens local serial devices.
`scripts/sim-wokwi-artisan-bridge.py` connects to the Wokwi RFC2217 server and creates a PTY at
`/tmp/libreroaster-wokwi`; Artisan opens that path as its Comm Port.

Prerequisites:

- **Wokwi for VS Code**. The RFC2217 server is not available in `wokwi-cli` or the web editor.
- **pyserial**: `python3 -m pip install pyserial`.
- **Linux or macOS**. The bridge creates a POSIX PTY; Windows needs a virtual COM-port pair
  instead.
- Artisan on the same machine as VS Code.

1. Build and start a simulation. Use F1 for a connection/telemetry check, F3 for
   `START`/`Heating`, F4 for a full public-curve roast, or F2 for the real SPI path:

   ```bash
   ./scripts/sim-wokwi-prepare.sh
   # F3: FEATURES=embedded,simulated-typical-roast ./scripts/sim-wokwi-prepare.sh
   # F4: FEATURES=embedded,simulated-web-medium-roast ./scripts/sim-wokwi-prepare.sh
   # F5: FEATURES=embedded,simulated-plant ./scripts/sim-wokwi-prepare.sh
   # F2: FEATURES=embedded,heat-sense ./scripts/sim-wokwi-prepare.sh
   ```

   Run `Wokwi: Start Simulator` in VS Code and keep the simulator tab visible. Close the Wokwi
   serial monitor; the bridge must be the only serial client.

2. Start the bridge in a terminal:

   ```bash
   ./scripts/sim-wokwi-artisan-bridge.py
   ```

   It prints the Artisan port immediately and waits for the RFC2217 server if Wokwi is still
   starting:

   ```text
   Artisan port: /tmp/libreroaster-wokwi -> /dev/pts/N
   ```

   Add `--trace /tmp/artisan-trace.log` to record timestamped raw traffic in both directions.
Verify a trace without Artisan or Wokwi:

```bash
./scripts/verify-artisan-trace.py --trace /tmp/artisan-trace.log \
    --expect '^#OK' --min-read-responses 10 --forbid 'ERR safety_fault'
```

The verifier decodes the bridge's `S` (device) and `A` (Artisan) byte streams,
counts TC4-style temperature responses, applies the same regex checks used by
the Wokwi log verifier, and writes an optional `--json-out` artifact.

3. Configure Artisan under **Config → Device**:

   | Setting | Value |
   |---|---|
   | Device Type | Arduino |
   | Port | `/tmp/libreroaster-wokwi` |
   | Baud Rate | 115200 |
   | Data Bits / Parity / Stop Bits | 8 / None / 1 |
   | Flow Control | None |
   | DTR / RTS on connect | Disabled |
   | Timeout | 2000 ms |
   | Inter-byte timeout | 100 ms |

   The Comm Port list does not enumerate PTYs. Type the path into the editable port field.

4. Under the Artisan driver configuration, select `ArduinoTC4`, set channels to `1200`, and keep
   the polling interval at about 1 s. Click **ON**. The expected handshake is `CHAN`, `UNITS`,
   `FILT`, followed by `READ` polling.

5. Use F1 for the handshake and `READ`/`STATUS`; F3 for `START`/`Heating`; F4 for a full
   696 s public-curve recording; or F5 for closed-loop heater/fan dynamics. While the heater is energized, Artisan's polling keeps the 15 s
   comms-idle guard satisfied. If Artisan disconnects, the firmware can latch; reconnect and
   recover with `PID;OFF`, `START`, or `PREHEAT`.

6. Stop the bridge with Ctrl-C. It removes the symlink. Restarting the bridge recreates the same
   port.

Validated on 2026-10-22 with Artisan 4.0.2 (Linux, Wokwi VS Code 4.0.1, F1 build): **ON** reported
`TC4 initialized`, ET/BT updated from live `READ` responses, and **START** entered recording.

F4 was validated on the same stack with the 696 s public profile: Artisan recorded the full curve,
starting at 183.8/203.8 °C BT/ET, passing the 165.4/185.4 °C minimum at 60 s, and ending at
207.4/227.4 °C. The bridge trace aligned 348 `READ` samples to the authored curve with RMSE
0.04 °C and maximum BT/ET error 0.09 °C. Artisan auto-marked `CHARGE` at 0:00; the final screen
showed ET 227.4 / BT 207.4. The 696 s plateau began near 11:36, and stopping at the chart edge placed
Artisan's `DROP` marker at 12:00. Wokwi ran at 100 % speed with the simulator tab visible; the clock
throttles when that tab is hidden.

The F1/F3/F4 curves are time-based and open-loop: Artisan sees temperatures and can drive commands,
but heater/fan changes do not move the simulated temperatures. F5 (`simulated-plant`) is the
exception: the control loop's heater/fan output advances a first-order `ThermalPlant`, so BT/ET
respond to `OT1`/`OT2` and PID. F4's ET is synthesized as BT + 20 °C; F2 reports the static
MAX31856 chip values. Unless you select F5, this setup exercises the Artisan session, protocol,
telemetry, and UI, not closed-loop thermal behavior.

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
| 0x00 | CR0 | Stored; writing bit 6 (1SHOT) updates the conversion block and self-clears bit 6 |
| 0x01 | CR1 | Stored; read-back verification works |
| 0x02 | MASK | Stored; set bits mask the corresponding SR fault bits on read |
| 0x0A | CJTH | High byte of the left-justified 14-bit CJ value |
| 0x0B | CJTL | Low byte |
| 0x0C | LTCBH | High byte of the left-justified 24-bit temperature |
| 0x0D | LTCBM | Middle byte |
| 0x0E | LTCBL | Low byte |
| 0x0F | SR | Current `fault` control value, masked by `MASK` |

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
written and clears the 1SHOT bit, matching the datasheet's self-clearing
behavior. It does **not** model the 210 ms conversion time, the 50/60 Hz notch
filter, the LTC delay, thermocouple physics, cold-junction compensation, or
automatic open-circuit detection. The firmware still waits its normal
conversion budget, and the fault register is whatever the slider says.

A permanently all-zero conversion frame is rejected by the firmware. The
default chip values are non-zero, so this only matters if you force both
temperature and cold junction to 0.

## 6. F0: esp-emulator (experimental, currently blocked)

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

## 7. Troubleshooting

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
| Bridge prints `Waiting for the Wokwi RFC2217 server` | Start or restart the Wokwi simulator; keep its tab visible; confirm `rfc2217ServerPort = 4000` in `sim/wokwi/wokwi.toml` |
| Wokwi reports `address already in use` | Another simulator or service owns port 4000; stop it or change `rfc2217ServerPort` and pass the matching `--rfc2217` to the bridge |
| Artisan does not list `/tmp/libreroaster-wokwi` | PTYs are not enumerated; type the path into the editable Comm Port field |
| Artisan cannot open the bridge port | Close the Wokwi serial monitor and any other bridge; only one client should own the serial session |
| Artisan connects but `READ` returns nothing | Confirm the bridge says `Connected`, the baud is 115200, and Artisan DTR/RTS are disabled; check the Wokwi serial monitor is closed |
| Artisan disconnects mid-roast | Reconnect Artisan; if the heater was on, the 15 s comms-idle latch may have fired. Recover with `PID;OFF`, `START`, or `PREHEAT` |
| `Permission denied` creating the bridge link | Use a path you own under `/tmp` or your home directory; do not create it under `/dev` |

## 8. Safety boundary

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
