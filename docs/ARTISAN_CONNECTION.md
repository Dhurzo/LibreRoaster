# Connecting Artisan to LibreRoaster

This guide covers the serial connection, the Artisan device and driver settings, the startup
handshake, and the compatibility boundary. It does not restate the command grammar; use
[`PROTOCOL.md`](PROTOCOL.md) for that, and use [`ARTISAN_MODES.md`](ARTISAN_MODES.md) and
[`ARTISAN_ADVANCED.md`](ARTISAN_ADVANCED.md) for roast workflows.

LibreRoaster behaves like a serial roasting controller that Artisan can drive. It is not a
reimplementation of the full Artisan desktop stack.

---

## 1. Choose a Transport

LibreRoaster accepts the same protocol over both transports:

| Transport | Port | Baud | Use |
|---|---|---|---|
| **Native USB CDC** (recommended) | `/dev/ttyACM0` on Linux, `/dev/cu.usbmodem-*` on macOS, `COMx` on Windows | 115200 8N1 | Normal operation; no adapter or extra wiring |
| **UART0** | Adapter on GPIO20/GPIO21 | 115200 8N1 | USB CDC unavailable or a separate physical path |

UART wiring:

| LibreRoaster signal | Adapter signal |
|---|---|
| GPIO20 RX | Adapter TX |
| GPIO21 TX | Adapter RX |
| GND | GND |

The UART path is 3.3 V. Do not use a 5 V adapter without level shifting. USB CDC uses the
ESP32-C3 USB-Serial-JTAG peripheral; on Windows it may appear as a generic CDC device or need the
`esp32_usb_jtag` driver.

For board-level boot behavior and the GPIO9 pull-up requirement, see
[`CONNECTION_TYPES.md`](CONNECTION_TYPES.md).

---

## 2. Configure Artisan

### 2.1 Device tab

Open **Config → Device** and set:

```text
Device Type:   Arduino
Port:          [detected port, e.g. /dev/ttyACM0 or COM3]
Baud Rate:     115200
Data Bits:     8
Parity:        None
Stop Bits:     1
Flow Control:  None
```

In the same tab, verify:

| Setting | Value | Why |
|---|---|---|
| **DTR on connect** | Disabled | LibreRoaster ignores DTR/RTS; leaving them disabled keeps the session clean. |
| **RTS on connect** | Disabled | Same; the firmware does not use these lines for reset or boot control. |
| **Timeout (ms)** | 2000 | Covers several real control ticks (≈310–330 ms each). |
| **Inter-byte timeout (ms)** | 100 | The parser expects complete lines. |

### 2.2 Driver tab

LibreRoaster speaks the TC4 serial protocol, compatible with Artisan's ArduinoTC4 driver.

```text
Driver: ArduinoTC4
Port:   [same port as above]
```

Click **Configure** next to the driver and set:

| Option | Value | Notes |
|---|---|---|
| **Channels** | 1200 | Sent as `CHAN;1200`; acknowledged with `#1200`. |
| **Temperature Unit** | Celsius or Fahrenheit | Sent as `UNITS;C` or `UNITS;F`. |
| **Filter** | 70 | Sent as `FILT;70`; the firmware stores the first value but does not apply the filter. |
| **Poll interval** | ~1 s | Artisan polls `READ`; do not exceed 15 s while heating (see §5). |

Optional extra devices:

- `+ArduinoTC4_34` exposes CH3/CH4 as RoR target and measured RoR. It sends `CHAN;1234`.
- `+ArduinoTC4_56` and `+ArduinoTC4_78` expose heater/fan/SV curves, but the READ layout shifts.
  Add `+ArduinoTC4_34` together with them or read the raw serial log instead.

---

## 3. Handshake

Artisan normally sends this sequence on connect:

```text
CHAN;1200
UNITS;C
FILT;70,70,70,70
```

Expected responses:

| Command | Response | Purpose |
|---|---|---|
| `CHAN;1200` | `#1200` | Channel-map acknowledgement |
| `UNITS;C` / `UNITS;F` | `#OK` | Display scale for temperature-bearing output |
| `FILT;...` | `#OK` | Compatibility acknowledgement |

The `#` prefix is required. Artisan's ArduinoTC4 driver accepts only empty or `#`-prefixed lines
during initialization; a plain `OK` produces “Arduino could not set temperature unit” or “...
filters” and sends Artisan into a re-initialization loop without ever reaching `READ`.

The handshake is accepted while the safety latch is armed. `CHAN`, `UNITS`, and `FILT` have no
actuator side effects, so reconnecting Artisan to a latched device completes normally. Every
re-energizing command remains rejected until the operator recovers the device.

`UNITS` changes output formatting only. Temperatures are stored internally in Celsius; setpoints
sent in Fahrenheit are converted before validation.

---

## 4. Normal Session

A normal session looks like this:

```text
1. Artisan opens the serial port.
2. Artisan sends the handshake.
3. Artisan polls READ at ~1 Hz.
4. The operator uses sliders, PID controls, or profile/event buttons.
```

`READ` returns the TC4-style temperature line. With PID disabled it has five fields:

```text
AMB,ET,BT,0.0,0.0
```

With PID enabled it appends heater, fan, and setpoint:

```text
AMB,ET,BT,0.0,0.0,HEATER,FAN,SV
```

`AMB` is the cold-junction mean of the healthy MAX31856 channels. `STATUS` is the separate
20-field diagnostic line; its field semantics are in [`INSTRUMENTATION.md`](INSTRUMENTATION.md).

### Comms-idle rule

Artisan polling refreshes the firmware's host-activity timer. The 15 s comms-idle guard is armed
whenever the heater is energized or a roast is active. If Artisan stops polling, closes, or loses
the link, LibreRoaster latches, turns the heater off, and runs the fan at 100 %.

Do not configure an Artisan sampling interval above 15 s. Custom clients that do not poll must send
`READ` or `STATUS` every 5–10 s while heating. `STREAM;ON` is output-only and does **not** refresh
the timer.

---

## 5. Operating Rules

- **Artisan is the session owner.** It polls temperatures, sends operator actions, and decides when
  a roast starts or ends. LibreRoaster owns the device-side control loop and safety interlocks.
- **Keep `STREAM` off for Artisan.** The spontaneous `#` telemetry stream is a LibreRoaster
  extension for custom clients. Artisan only needs `READ`; enabling the stream can cost a `READ`
  sample because both share the wire.
- **Use `PID;OFF` to recover from a latch.** `STOP` is an emergency stop that arms the safety latch.
  `PID;OFF` un-latches and returns to `Idle`; `START` and `PREHEAT` are deliberate re-energizing
  actions that also clear the latch. `PID;ON` never clears a latch.
- **Watch Artisan's PID handshake.** Pressing *PID ON* sends Artisan's own gains, `PID;CHAN;1`
  (feedback on ET, not BT), cycle time, limits, and `PID;ON`. If you want bean-temperature control,
  set Artisan's PID input to channel 2. Never enable “PID ON at CHARGE”; an automatic `PID;ON`
  cannot recover a latched device.
- **Do not assume profile-file transfer.** `PROFILE` and `FANPROFILE` send runtime setpoints over
  serial; LibreRoaster does not read Artisan `.alog` files. See
  [`ARTISAN_MODES.md`](ARTISAN_MODES.md) for manual and profile workflows.

---

## 6. Smoke Test

After connecting, verify the transport and protocol without heat:

```text
CHAN;1200
UNITS;C
READ
STATUS
```

Expected:

- `CHAN` returns `#1200`;
- `UNITS` returns `#OK`;
- `READ` returns a temperature CSV;
- `STATUS` returns the 20-field diagnostic line.

This confirms the serial path, parser, formatter, and both response paths. It does not validate
thermocouple accuracy or actuator wiring; use [`preflight-check.md`](preflight-check.md) for the
hardware bring-up sequence.

---

## 7. Troubleshooting

### Artisan says “Arduino could not set temperature unit” or “... filters”

The handshake response was not `#`-prefixed. Confirm the firmware is current and that `UNITS` and
`FILT` return `#OK`.

### `READ` returns `0.0,0.0,0.0,0.0,0.0`

The sensor path is not producing valid readings. Check MAX31856 power, SPI wiring, and chip selects;
the pin map is in [`HARDWARE.md`](HARDWARE.md). A simulated-sensors build is a useful control.

### Heater stays at 0 %

Check `STATUS` field 20 (`FaultFlag`). If it is `1`, the safety latch is armed: recover with
`PID;OFF`, or deliberately re-energize with `START`/`PREHEAT`. If the fault flag is `0`, check the
SSR output and the hardware status in the debug log; the `heat-sense` feature is opt-in and must
match the board's current-sense circuit.

### Fan stays at 0 % in PID mode

Send `OT2 50` or load a `FANPROFILE`. The firmware enforces a 20 % floor while the heater is on,
but it does not invent a fan setpoint when neither command has been sent.

### `ERR channel_full command_dropped`

A command burst exceeded the 16-entry shared command channel in one tick. Artisan does not retry;
the dropped command is lost. Reduce the startup burst or split the command sequence.

### Telemetry is garbled or `READ` occasionally returns `-1`

`esp_println` logs share UART0 with the protocol. Do not use UART0 for Artisan while verbose logging
is enabled; use native USB CDC instead. The `instrumentation` build increases log volume and must
not be used for production sessions.

### PID output stays at 0 % after `PID;SV`

`PID;SV` updates the target but does not enable the PID. Send `PID;ON` first.

### RoR fault on a light-roast turnaround

The RoR guard is two-tier: the soft band (0.75–1.0 °C/s) requires about 3.7 s of sustained
exceedance before latching, while the hard band (>1.0 °C/s) latches within a few ticks. If the guard
trips on a legitimate turnaround, check the roast state and thresholds in
[`SAFETY_CASE.md`](SAFETY_CASE.md) before changing tuning.

### Commands work over UART but not USB CDC

This points to the USB path or host driver, not the protocol core. Recheck the cable, port, and
USB-Serial-JTAG driver; see [`CONNECTION_TYPES.md`](CONNECTION_TYPES.md).

---

## 8. Compatibility Boundary

Good compatibility:

- Artisan as a serial controller speaking TC4-style commands;
- temperature polling with `READ`;
- manual heater/fan control;
- PID and profile commands;
- startup handshake and reconnects to a latched device;
- `STATUS` diagnostics.

Not implemented:

- Artisan `.alog` file interchange;
- WebSocket device mode;
- Modbus device mode;
- artisan.plus synchronization;
- vendor-specific driver behavior outside this serial surface.

The right mental model is **session compatibility, not ecosystem compatibility**.

Artisan 2.8+ is the recommended host version. Artisan 2.7 generally works but may require manual
driver configuration.

---

## Related Documents

- [`PROTOCOL.md`](PROTOCOL.md) — command grammar, responses, and error tokens.
- [`INSTRUMENTATION.md`](INSTRUMENTATION.md) — STATUS fields and diagnostics.
- [`ARTISAN_MODES.md`](ARTISAN_MODES.md) — manual and profile roast workflows.
- [`ARTISAN_ADVANCED.md`](ARTISAN_ADVANCED.md) — CHARGE/DROP, RoR-follow, and TUNE.
- [`CONNECTION_TYPES.md`](CONNECTION_TYPES.md) — USB/UART and GPIO9 boot behavior.
- [`HARDWARE.md`](HARDWARE.md) — pin map and electrical constraints.

---

*Last updated: 2026-10-22.*
