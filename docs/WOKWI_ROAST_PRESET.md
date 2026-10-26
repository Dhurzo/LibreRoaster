# Wokwi Roast Preset: Typical Thermocouple Temperatures

**Last updated:** 2026-10-26

This runbook covers the default MAX31856 values used by Wokwi F2 and the GPIO1
heat-sense switch procedure. It changes simulation inputs only; it does not
change firmware behavior.

For the full simulation guide, see [`SIMULATION.md`](SIMULATION.md).

## 1. Default preset

`sim/wokwi/diagram.json` starts both custom MAX31856 chips at a
first-crack-like roast point instead of ambient:

| Channel | Chip id | Chip select | `temperature` | `coldJunction` | `fault` |
|---|---|---:|---:|---:|---:|
| BT (bean) | `bt` | GPIO4 | 200 °C | 35 °C | 0 |
| ET (environment) | `et` | GPIO3 | 220 °C | 35 °C | 0 |

With PID disabled, `READ` returns approximately:

```text
35.0,220.0,200.0,0.0,0.0
```

The field order is `AMB,ET,BT,unused,unused`. `AMB` is the cold-junction mean
of the healthy channels, so it reports 35 °C here, not 200 °C.

## 2. Why these values

- **BT = 200 °C** is inside the usual first-crack window (roughly 196–205 °C)
  and gives a recognizable “roast in progress” value.
- **ET = 220 °C** is a typical drum-air temperature above the bean at first
  crack. ET legitimately runs hotter than BT.
- **Cold junction = 35 °C** models a warm connector/enclosure, not a roast
  temperature. It affects `AMB`, not the thermocouple value.
- **Fault = 0** keeps both channels healthy.

These values are below the firmware's emergency cutoffs: BT 260 °C and
ET 300 °C (`OVERTEMP_THRESHOLD` / `ET_OVERTEMP_THRESHOLD`). The absolute
validity ceiling is 350 °C.

This is a static snapshot, not a roast curve. Use the F1 `simulated-sensors`
build (or F3 for a guard-safe `START` test), F5 `simulated-plant` for closed-loop
heater/fan dynamics, or move the sliders manually, to exercise a temperature
trajectory.

## 3. Changing the preset

Edit the `attrs` of the `bt` and `et` parts in `sim/wokwi/diagram.json`:

```json
{
  "type": "chip-max31856",
  "id": "bt",
  "attrs": {
    "temperature": "200",
    "coldJunction": "35",
    "fault": "0"
  }
}
```

Reload the Wokwi project to apply diagram values. Temperature changes do not
require a firmware rebuild: the chip reads the slider attributes on the next
one-shot conversion. Rebuild only when you change the firmware feature set
(F1, F2, or `embedded` without `heat-sense`).

To return to a cold-boot setup, set both `temperature` values to `25` and both
`coldJunction` values to `25`.

## 4. The boot self-test warning is expected

On F2 the firmware runs the real MAX31856 self-test at boot. With the hot
defaults and the release `Warn` log level, both channels log:

```text
WARN - MAX31856 self-test: Temperature out of expected range: 200.0°C
WARN - MAX31856 self-test: Temperature out of expected range: 220.0°C
```

The self-test uses the ambient plausibility window `-50..=100 °C`. A roast
temperature is outside that window by design. The warning is informational:
it does not degrade the channel and does not fail the self-test. The
`info!` lines with CR0, MASK, and raw conversion values are suppressed in the
release image; add `instrumentation` to `FEATURES` if you need them.

If you want a clean boot log, set both sliders to 25 °C before resetting.

## 5. Heat-sense switch runbook (F2 only)

The GPIO1 slide switch is the heat-sense input. It is interpreted only by the
`embedded,heat-sense` build without `simulated-sensors` (F2). In F1/F3/F5 the
detection code is compiled out and the switch has no effect.

| Switch position | Contacts | GPIO1 | Firmware interpretation |
|---|---|---|---|
| RIGHT (`"value": "1"`) | 3-2 | pulled up | no current through the SSR |
| LEFT (`"value": "0"`) | 1-2 | GND | current through the SSR |

The diagram starts in **RIGHT** (no current). With the heater off, RIGHT is the
only valid state. If the switch is LEFT at boot, the firmware correctly
interprets current with zero duty and latches `SSR stuck-on` after
`HEAT_PRESENT_MISMATCH_MAX` (10) consecutive samples, about 3.3 s at the
control-loop cadence.

### Exercise the input

1. Start with the switch **RIGHT**.
2. Energize the heater, for example with `OT1;100`.
3. Move the switch **LEFT** within `HEAT_MISMATCH_WINDOW_MS` (1.5 s) and hold
   it there long enough to prove there is no mismatch latch (>1.5 s).
   Otherwise the firmware latches “Heat source not detected” because the commanded
   duty is observable (≥ 50 %) and no heat has been seen for 1.5 s.
4. Turn the heater off (`OT1;0` or `PID;OFF`) and move the switch **RIGHT**
   again.

The switch is a manual stimulus, not automatic electrical feedback. The
simulator does not couple it to the heater command.

To run the negative check automatically:

```bash
./scripts/sim-wokwi-check.sh --scenario f2-heatsense
```

### Recover from a heat-sense latch

1. Move the switch **RIGHT** first.
2. Clear the latch with `PID;OFF`, `START`, or `PREHEAT`, or reset the
   simulator.
3. The internal safety trap emits `ERR safety_fault ...` once when it arms;
   the operator `STOP` path does not emit it.

If you do not need to test GPIO1, build `embedded` instead of
`embedded,heat-sense`. The real SPI path and the actuators still run, and the
switch is ignored.

## 6. Build and Scenario Selection

This preset is used by F2 (`embedded,heat-sense`). F1 uses
`embedded,simulated-sensors` for synthetic temperatures; F3 uses
`embedded,simulated-typical-roast` for the guard-safe `START`/`Heating` path;
F5 uses `embedded,simulated-plant` for closed-loop heater/fan dynamics. The
build variants, prepare commands, scenario matrix, and `SIM` controls are owned
by [`SIMULATION.md`](SIMULATION.md) §3.4.

## 7. Troubleshooting

| Symptom | Cause and action |
|---|---|
| `Temperature out of expected range` at boot | Expected with BT 200 °C / ET 220 °C; the self-test window is −50…100 °C. Not a fault. Set the sliders to 25 °C for a cold-boot log. |
| `SSR stuck-on` at boot or with the heater off | The switch is LEFT. Move it RIGHT, then recover with `PID;OFF`/`START`/`PREHEAT` or reset. |
| `Heat source not detected` after energizing | The switch stayed RIGHT for more than 1.5 s at observable duty. Move it LEFT within the window, or leave the interlock out with `FEATURES=embedded`. |
| `ERR safety_fault Comms idle timeout` while the heater is on | No command arrived for 15 s (`COMMS_IDLE_TIMEOUT_MS`). Send `READ` or `STATUS` every 5–10 s, or keep Artisan connected (it polls `READ`). Recover with `PID;OFF` or resend `PREHEAT`/`START`. Full explanation: [`SIMULATION.md`](SIMULATION.md) §3.5. |
| `READ` still shows old values | The control loop updates once per tick (~310–330 ms), and the EMA filter smooths changes. Large slider steps can take several seconds to settle. |
| No temperatures in F2 | Check the SPI wiring (CS GPIO4/3, SCK GPIO6, MOSI GPIO7, MISO GPIO5) and the serial monitor at 115200 on UART0. |

## 8. What this preset does not model

- A roast curve, thermal mass, or heater/bean dynamics in F2. F5 is the
  closed-loop exception; its `ThermalPlant` is still synthetic and uncalibrated.
- The MAX31856 conversion time, 50/60 Hz notch filter, or LTC delay.
- Cold-junction compensation or automatic open-circuit detection.
- Real current sensing: the heat-sense switch is manual.

Use the `simulated-sensors` curve for trajectories and the custom chip sliders
for static value/SPI checks.

## 9. Related documentation

- [`SIMULATION.md`](SIMULATION.md) — full no-hardware simulation guide.
- [`HARDWARE.md`](HARDWARE.md) — pins, SPI bus, and the real heat-sense circuit
  requirements.
- [`PROTOCOL.md`](PROTOCOL.md) — `READ`, `OT1`, `PID;OFF`, and `START`
  semantics.
- [`simulated-curve-test.md`](simulated-curve-test.md) — synthetic curve
  architecture and curve selection.
