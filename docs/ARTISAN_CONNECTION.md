# Artisan ↔ LibreRoaster Connection Guide

**Last updated:** 2026-08-12

This guide explains how to connect the official Artisan application to LibreRoaster and, more importantly, what kind of compatibility to expect from that connection.

LibreRoaster is designed to behave like a serial roasting controller that Artisan can drive. It is not a reimplementation of the full Artisan desktop stack.

## 1. Supported connection modes

LibreRoaster exposes two serial transports:

- **USB CDC** on the ESP32-C3, recommended for normal use
- **UART0** at 115200 baud through an external USB-to-UART adapter

The firmware accepts the same command surface over both.

## 2. Recommended path: native USB CDC

USB CDC is the preferred route because it matches the firmware’s main development and test path.

Typical port names:

- Linux: `/dev/ttyACM0`
- macOS: `/dev/cu.usbmodem-*`
- Windows: `COMx`

## 3. Alternative path: UART0

Use UART when USB CDC is unavailable or when you want a physically separate serial path.

Wiring model:

| LibreRoaster signal | Connect to adapter |
|---|---|
| GPIO20 RX | adapter TX |
| GPIO21 TX | adapter RX |
| GND | adapter GND |

This is a 3.3 V serial path. The external adapter must be compatible with that level.

## 4. Artisan settings

In the official Artisan app, use the serial-device workflow that matches Arduino/RPi-style devices.

Minimum settings:

| Setting | Value |
|---|---|
| Port | the detected USB CDC or UART port |
| Baud | 115200 |
| Data bits | 8 |
| Stop bits | 1 |
| Parity | none |
| Device mode | Arduino/RPi-style serial device |

The main requirement is that Artisan speaks plain serial commands over the selected port.

## 5. Handshake expectations

Artisan may send startup commands such as:

- `CHAN;1200`
- `UNITS;C` or `UNITS;F`
- `FILT;...`

LibreRoaster accepts these and responds with lightweight acknowledgements.

> Handshake responses are `#`-prefixed (`#1200` for `CHAN`, `#OK` for
> `UNITS`/`FILT`). This is deliberate: Artisan's ArduinoTC4 driver rejects
> any non-`#` initialisation response with "Arduino could not set
> temperature unit/filters" and never proceeds to `READ` polling.
>
> The handshake keeps working even while the device's safety latch is
> armed: `CHAN`/`UNITS`/`FILT` are accepted in every state (they have no
> actuator side effects), so reconnecting Artisan to a latched device
> completes normally instead of looping on "Arduino could not set
> channels/units/filters". Re-energizing commands remain rejected until
> the latch is cleared.

This means the connection model is forgiving: a terminal and a manually configured Artisan session can both talk to the device without a fragile startup contract.

## 6. Temperature scale synchronization

LibreRoaster stores temperatures internally in Celsius. `UNITS` only changes the output scale.

That makes `UNITS` one of the most important commands in the session.

### Operational rule

- if Artisan expects Celsius, send `UNITS;C`
- if Artisan expects Fahrenheit, send `UNITS;F`

### How to detect drift

`STATUS` field 19 reports the active display scale:

- `0` = Celsius
- `1` = Fahrenheit

If roast values look “wrong but plausible,” check the scale field before assuming a sensor or calibration failure.

## 7. What Artisan can do successfully with LibreRoaster

Using the official Artisan app in serial-controller mode, LibreRoaster is built to support:

- live temperature polling via `READ`,
- heater and fan control,
- PID enable/disable and setpoint control,
- profile command injection via `PROFILE` and `FANPROFILE`,
- startup handshake commands (including reconnects to a latched device),
- deeper runtime diagnostics via `STATUS`,
- immediate safety-fault visibility (`ERR safety_fault <reason>` when an internal trap latches the device).

This is the project’s core compatibility target.

## 8. What Artisan should not assume

The official Artisan ecosystem is broader than LibreRoaster’s firmware surface.

LibreRoaster does **not** currently present itself as:

- a Modbus device,
- a WebSocket device,
- a file-format endpoint for `.alog`,
- an artisan.plus synchronization peer,
- a full vendor-driver implementation.

If you stay inside the serial TC4-style workflow, compatibility is strong. If you expect the rest of the Artisan ecosystem, compatibility drops sharply.

## 9. Practical smoke test

Once connected, a good minimal verification sequence is:

```text
CHAN;1200
UNITS;C
READ
STATUS
```

Expected behaviors:

- `CHAN` returns `#1200`
- `UNITS` returns `#OK`
- `READ` returns temperature-oriented CSV
- `STATUS` returns the 20-field deep telemetry line

At that point you know:

1. the transport works,
2. the parser works,
3. the formatter works,
4. the firmware can publish both roast values and diagnostics.

## 10. Troubleshooting model

### No connection at all

Usually a cable, port, permissions, or wrong serial-mode issue.

### Port opens but values look nonsensical

Check:

- `UNITS` state,
- ET/BT wiring,
- whether the device has rebooted and reset scale state,
- whether you are looking at `READ` or `STATUS` output.

### Commands work over UART but not USB CDC

That points to the USB path, not the protocol core.

### `READ` works but automation fails

Usually means the client is using `READ` where it should use `STATUS`, or it assumes unsupported protocol features beyond the implemented TC4-style surface.

### `ERR safety_fault <reason>` appears and sliders stop responding

An internal trap (over-temperature, stale sensor, probe-stuck, comms-idle, …) armed the safety latch: the heater is at 0 % and the fan at 100 %. While latched, every re-energizing command is rejected with `ERR handler_failed … fault_condition_active`. Recovery: send `PID;OFF` (or `START`/`PREHEAT`). The handshake still works, so closing and reconnecting Artisan is also safe.

## 11. Compatibility summary

The right mental model is:

**Artisan as serial controller** → good compatibility

**Artisan as complete ecosystem** → partial compatibility

For a deeper, implementation-driven comparison against the official Artisan application and repository, see `PROTOCOL.md`.
