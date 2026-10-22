# 📡 Serial Protocol Surface

**Last updated:** 2026-10-22

This page is the README-level protocol overview. [`PROTOCOL.md`](PROTOCOL.md) owns the exact
grammar, field ordering, validation rules, and compatibility boundary. [`INSTRUMENTATION.md`](INSTRUMENTATION.md)
owns STATUS field semantics and diagnostics.

LibreRoaster implements a TC4-compatible serial interface over native USB CDC or UART0
(115200 8N1). The protocol is ASCII text framed as lines; the same command set works on both
transports.

## Implemented command families

- **Polling:** `READ`, `STATUS`, `STAT`
- **Manual actuation:** `OT1` (including `up` / `down`), `OT2`, `IO3`, `DCFAN`, `UP`, `DOWN`,
  `START`, `STOP`
- **PID and roast control:** `SETTARGET`, `PIDGAIN`, `PID;ON`, `PID;OFF`, `PID;SV`, `PID;T`,
  `PID;CHAN`, `PID;CT`, `PID;LIMIT`
- **Profiles:** `PROFILE`, `FANPROFILE`, `RORPROFILE` (plus `RORPROFILE;OFF`), `PREHEAT`, `CHARGE`,
  `DROP`
- **Handshake and setup:** `CHAN`, `UNITS`, `FILT`
- **Autotune:** `TUNE` (step test, `ABORT`, `UNLOCK`, `STATUS`)
- **Diagnostics:** `REG`, `#DUMP`, `STREAM;ON`, `STREAM;OFF`

`OT1`, `OT2`, and `IO3` accept the four TC4 delimiter forms: space, semicolon, comma, and equals
sign. Artisan slider configurations normally use the comma form.

## Core response shapes

- **`READ` with PID off** returns the TC4-style line `AMB,ET,BT,0.0,0.0` (5 fields).
- **`READ` with PID on** appends heater, fan, and setpoint: `AMB,ET,BT,0.0,0.0,HEATER,FAN,SV`
  (8 fields).
- **`STATUS` / `STAT`** returns a 20-field diagnostic line covering temperature, actuators,
  watchdog state, PID internals, command latency, fault state, and temperature scale.
- **Continuous telemetry** is opt-in with `STREAM;ON`. It emits `#<time>,ET,BT,ROR,Gas` once per
  second; the leading `#` distinguishes it from synchronous command responses. `#CHARGE dt=NN.N`
  is a one-shot charge event.
- **Errors** are line-oriented `ERR ...` responses, for example
  `ERR unknown_command unknown_command` or `ERR handler_failed <token>:<source>`.

The spontaneous `#` stream is off by default because a stream line can occupy the response slot of a
`READ` on the shared wire. Artisan does not need it and polls `READ`; custom clients opt in.

For exact commands, examples, and edge cases, use [`PROTOCOL.md`](PROTOCOL.md). For Artisan setup,
use [`ARTISAN_CONNECTION.md`](ARTISAN_CONNECTION.md); for roast workflows, use
[`ARTISAN_MODES.md`](ARTISAN_MODES.md) and [`ARTISAN_ADVANCED.md`](ARTISAN_ADVANCED.md).
