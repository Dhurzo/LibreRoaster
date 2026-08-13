# LibreRoaster Protocol Reference

**Last updated:** 2026-08-12

This is the implementation-facing serial protocol reference for LibreRoaster. It describes what the firmware currently accepts and emits, how the session behaves, and where compatibility with the official Artisan application starts and stops.

## 1. Transport model

LibreRoaster exposes the same protocol over two physical transports:

- **native USB CDC** on the ESP32-C3
- **UART0** at **115200 8N1**

The protocol is ASCII text. Commands are framed as lines. Responses are emitted as lines as well.

The codebase treats transport and protocol separately: transport readers own byte accumulation, the parser owns command decoding, and the formatter owns response shape.

## 2. Session model

The expected operator is the official Artisan application in Arduino/RPi-style serial mode.

In a normal session, Artisan:

1. opens the serial port,
2. optionally sends handshake commands,
3. polls `READ` repeatedly,
4. may request `STATUS` for deeper diagnostics,
5. sends actuation or PID/profile commands during the roast.

LibreRoaster does not require a complex session-establishment phase. It is intentionally permissive so a terminal, a script, or Artisan can all drive the device.

## 3. Handshake commands

The firmware accepts the standard startup-oriented commands Artisan commonly emits.

### `CHAN;<rate>`

- Purpose: channel-map acknowledgement for Artisan startup
- Response: `#<rate>`

Example:

```text
CHAN;1200
#1200
```

### `UNITS;C` / `UNITS;F`

- Purpose: select display scale for temperature output
- Response: `#OK`

This updates the display conversion used by `READ`, `STATUS`, and temperature-bearing PID/setpoint output.

> The acknowledgement is `#`-prefixed (not `OK`) because Artisan's ArduinoTC4
> driver only accepts empty or `#`-prefixed lines during its initialisation
> handshake (`Arduino could not set temperature unit` otherwise, followed by
> an infinite re-init where `READ` never works). The reference TC4 firmware
> answers `# Changed units to C`/`# Changed units to F`.

### `FILT;<values>`

- Purpose: compatibility acknowledgement for Artisan startup configuration
- Response: `#OK`

Current behavior is intentionally shallow: the firmware acknowledges the command and stores only the first parsed value as protocol state. It does not implement the richer semantics that a desktop application might assume from the filter name.

> Same `#`-prefix handshake rationale as `UNITS` (`Arduino could not set
> filters` otherwise). The reference TC4 firmware stays silent here; `#OK`
> satisfies the same empty-or-`#` contract.

## 4. Polling responses

### `READ`

`READ` is the main temperature polling command.

#### PID disabled

Response shape:

```text
AMB,ET,BT,0.0,0.0
```

Field meaning:

1. ambient temperature
2. environment temperature
3. bean temperature
4. unused channel placeholder
5. unused channel placeholder

> Note: the ambient field is a structural placeholder. There is no ambient
> sensor in the hardware model, so the firmware always emits `0.0` on a live
> device (the field is only populated in host test code).

#### PID enabled

When PID is enabled, the formatter appends actuator and setpoint information:

```text
AMB,ET,BT,0.0,0.0,HEATER,FAN,SV
```

Additional fields:

6. heater output percentage
7. fan output percentage
8. active setpoint temperature

### Temperature-scale behavior

If `UNITS;F` is active, only temperature-bearing values are converted. Percent outputs remain percent outputs.

That is important for client code: heater and fan should never be interpreted as Fahrenheit-bearing fields.

## 5. Diagnostic telemetry

### `STATUS` / `STAT`

These commands return the deep telemetry line used for automation, debugging, and internal health monitoring.

Response shape:

```text
ET,BT,Heater,Fan,WatchdogOK,WatchdogFailures,LastWatchdogReason,LEDCGuardTimeouts,RegressionActive,PV,MV,IntegratorValue,DerivativeValue,SaturationFlag,IntegratorClampFlag,DerivativeAvailableFlag,CommandLatency,MaxCommandLatency,TempScale,FaultFlag
```

Field map:

1. ET
2. BT
3. Heater
4. Fan
5. Watchdog feed success flag
6. Consecutive watchdog failure count
7. Last watchdog failure reason token
8. LEDC guard timeout count
9. Regression-active flag
10. PID process variable
11. PID manipulated variable
12. PID integrator value
13. PID derivative value (°C/min of the active display scale)
14. Saturation flag
15. Integrator clamp flag
16. Derivative-available flag
17. Last command latency in microseconds
18. Maximum observed command latency in microseconds
19. Temperature-scale flag (`0` = Celsius, `1` = Fahrenheit)
20. Fault condition flag (`0` = normal, `1` = emergency fault active)

`STATUS` is the right interface for anything that needs runtime health, not just roast temperatures.

### Continuous telemetry

During an active session the control loop also emits a spontaneous telemetry
line once per second (`DEFAULT_OUTPUT_INTERVAL_MS`), not once per control
tick (the real tick is ~310–330 ms). It is always prefixed with `#` so
clients can distinguish it from synchronous responses:

```text
#<time>,ET,BT,ROR,Gas
```

- `time`: elapsed seconds since the roast start (or boot outside a session)
- `ET`, `BT`: environment and bean temperatures (display scale)
- `ROR`: rate of rise, in °C/min of the active display scale
- `Gas`: current heater output percentage

Clients that only poll `READ` can ignore these lines; clients that stream
must treat any line beginning with `#` as asynchronous.

### `#CHARGE` event

When the firmware detects the bean-charge event (a BT drop of more than
`6.0 °C` within a ~3 s window), it emits a spontaneous event line:

```text
#CHARGE dt=NN.N
```

`dt` is the observed temperature drop. This is a one-shot event emitted at
charge detection time, not a periodic line.

## 6. Manual actuator commands

### Delimiter forms

The TC4 serial spec (aArtisan "Serial Commands" note 2) allows the parameter
delimiter to be a comma, space, semicolon or equals sign for **every**
command. The firmware accepts **all four** for the manual actuator commands:

```text
OT1 75     OT1;75     OT1,75     OT1=75
IO3 50     IO3;50     IO3,50     IO3=50
```

Configurations documented for Artisan sliders/buttons classically use the
comma form (`OT1,{v}`, `IO3,{v}`); all are accepted.

### `OT1 <0-100>` / `OT1;<0-100>` / `OT1,<0-100>` / `OT1=<0-100>`

Sets heater power percentage. Accepts all four delimiter forms.

### `OT1,up` / `OT1,down`

TC4 step commands: move the heater duty by the internal step instead of
setting an absolute value. Maps to the same actuator path as `UP`/`DOWN`.

### `OT2 <0-100>` / `OT2;<0-100>` / `OT2,<0-100>` / `OT2=<0-100>`

Sets fan speed with decimal input support. Accepts all four delimiter forms.

Implementation behavior:

- parses as floating point,
- rounds to the nearest integer,
- clamps to the `0..100` range,
- marks whether clamping occurred.

If clamping occurs, the control layer emits an `ERR OT2_CLAMPED fan=<n> heater_unchanged`
notification so Artisan knows the value was sanitised. Per Spec F4.8 the `OT2` command
is intentionally fan-only: the heater and PID state are left untouched. (Bug L10:
earlier drafts of this document claimed clamping cut the heater — that diverged from
the implementation in `roaster_control.rs::handle_set_fan_speed`, which keeps the
heater unchanged by design.)

### `IO3 <0-100>` / `IO3;<0-100>` / `IO3,<0-100>` / `IO3=<0-100>`

Sets fan speed as an integer-oriented command path. Accepts all four delimiter forms.

### `DCFAN <0-100>` / `DCFAN,<0-100>`

TC4 fan command (added to the aArtisan spec 13-Apr-2014). Sets the fan duty
0-100 and maps to the same actuator path as `IO3`. No response is emitted.

> The reference TC4 firmware slews the duty at a maximum of 25 points/second
> to limit fan inrush on triac-driven roasters (Hottop). LibreRoaster drives
> the fan with a 25 kHz LEDC PWM (no triac inrush), so the duty is applied
> immediately and the slew is intentionally not implemented.

### `UP` / `DOWN`

Increment or decrement heater output in 5% steps.

### `STOP`

Emergency stop path. Heater is cut and fan is forced to 100%.

`STOP` arms the safety latch: while latched, only `READ`, `STATUS`, `STOP`,
`START`, `PREHEAT` and the handshake commands `CHAN`/`UNITS`/`FILT` are
accepted (other commands return
`ERR handler_failed:fault_condition_active`). `CHAN`/`UNITS`/`FILT` are
admitted deliberately: they have no actuator side effects, and rejecting
them would break Artisan reconnects (its ArduinoTC4 handshake fails on any
non-`#` line and re-initialises forever). Recovery:

1. `PID;OFF` — unconditional un-latch, returns to `Idle`; or
2. `START` / `PREHEAT` — treated as the operator's deliberate re-energize:
   the latch is cleared and the new roast/preheat proceeds directly (Bug P3).

> Note: a bare `OFF` token is **not** parsed by the firmware (only the
> `PID;OFF` form is, in the PID subcommand parser). Earlier drafts of this
> document listed `OFF` as a recovery command; the parser rejects it with
> `ERR unknown_command`, so clients must send `PID;OFF`.

## 7. PID and roast-control commands

LibreRoaster accepts both Artisan-standard semicolon-delimited PID commands and a limited set of legacy comma variants.

### Supported forms

- `PID;ON`
- `PID;OFF`
- `PID;SV;<temp>`
- `PID;T;<kp>;<ki>;<kd>`
- `PID;CHAN;<1-2>`
- `PID;CT;<ms>`
- `PID;LIMIT;<min>;<max>`
- `PID,ON`
- `PID,OFF`
- `PID,SV,<temp>`
- `PIDGAIN <kp> <ki> <kd>`
- `SETTARGET <temp>`
- `START`
- `PREHEAT <temp>`

### Key validation rules

- target temperature values must parse as finite floats at the parser layer
  (NaN/Inf rejected); **there is no raw-numeric range check in the parser**
  (Bug B9: the old `50.0..=300.0` check ran on the raw display-unit value
  and rejected legitimate °F setpoints such as `400` °F ≈ 204 °C). The
  `50–300 °C` range is enforced in the control layer **after** display-unit
  conversion to Celsius (`convert_from_display`), so `SETTARGET`, `PREHEAT`
  and `PROFILE` setpoints are validated in true °C.
- a `PROFILE` whose converted setpoint falls outside that range is rejected
  with `ERR handler_failed invalid_state:profile_temp_out_of_range`
- PID gains must parse as floats
- semicolon PID gains reject negative values
- PID cycle time rejects values below 10 ms
- PID channel accepts `1..=2` (channel 1 = ET, channel 2 = BT; the
  firmware has exactly two thermocouple channels, so `3`/`4` are rejected
  with `ERR out_of_range`)

The firmware stores temperatures internally in Celsius and converts only on output.

## 8. Profile commands

### `PROFILE;t1,T1;t2,T2;...`

Loads a roast temperature profile into fixed-capacity profile storage.

### `FANPROFILE;t1,s1;t2,s2;...`

Loads a fan profile into fixed-capacity fan-profile storage.

### Runtime behavior

The firmware interpolates linearly between setpoints and holds the final target after the last point.

This is a live-control feature, not a file-format import layer. LibreRoaster does not consume Artisan `.alog` profiles directly.

## 9. Diagnostic extensions

### `REG`

Triggers the over-temperature regression workflow. This is a firmware diagnostic feature, not a standard Artisan roasting feature.

Response:

- on builds with the `regression` feature: `OK regression_started`
- otherwise: `ERR regression_disabled`

### `#DUMP`

Requests the roast ring-buffer dump.

This is also a LibreRoaster-specific extension and should be treated as a debugging/forensics surface rather than a guaranteed Artisan integration feature.

## 10. Error responses and acknowledgements

The formatter emits simple line-oriented acknowledgements and errors:

- `#<value>` for `CHAN` (e.g. `#1200`)
- `#OK` for `UNITS` and `FILT` — `#`-prefixed because Artisan's ArduinoTC4
  initialisation only accepts empty or `#`-prefixed handshake lines
- `OK regression_started` for `REG` (on builds with the `regression` feature)
- `ERR ...` for failures

Handler-level failures are emitted as `ERR handler_failed <token>:<source>`
(e.g. `ERR handler_failed invalid_state:profile_temp_out_of_range`, `ERR
handler_failed invalid_state:fault_condition_active`). Valid tokens:
`temperature_out_of_range`, `sensor_fault`, `invalid_state`, `pid_error`,
`hardware_error`, `emergency_shutdown`. The `:source` suffix is a
diagnostic discriminator, not a stable contract — client code should not
assume a rich structured error taxonomy.

The wire can also carry these transport/scheduling-level `ERR` lines:

- `ERR channel_full command_dropped` — the shared command channel was full; the command was dropped
- `ERR rate_limited excess commands this tick` — more commands than the per-tick budget arrived
- `ERR status_too_long` — a formatted response exceeded the output buffer
- `ERR command_ignored_inactive_channel` — command arrived on a channel the firmware is not currently serving
- `ERR buffer_overflow` — the transport byte buffer overflowed
- `ERR regression_disabled` — `REG` sent on a build without the `regression` feature
- `ERR OT2_CLAMPED fan=<n> heater_unchanged` — `OT2` value was clamped (see §6)
- `ERR safety_fault <reason>` — an internal trap (overtemperature, stale
  sensor, NaN reading, rate-of-rise, probe-stuck, comms-idle, max roast
  time, watchdog or actuator-write failure) armed the emergency latch. The
  `<reason>` text is a human-readable diagnostic (may contain spaces), not
  a stable contract. The line is emitted once per latch event; the STOP
  command path (operator-initiated) does not emit it.
- `ERR probe_stuck_warning` — manual/software-PID mode only: BT has been
  flat (< 1 °C variation) for 120 s with the heater on. Purely
  informational (a legitimately slow finish can hold BT flat at low duty);
  the latch lands at 300 s via `ERR safety_fault Probe stuck`. Emitted once
  per stuck episode. Firmware-PID mode does not use this stage (it latches
  directly at 120 s).

> RoR-guard tiering (A-TC4-D, 2026-08-12): the `rate_of_rise_exceeded` trap
> is two-tier. Rates in the soft band (0.5–1.0 °C/s) latch only after ~3.7 s
> of sustained exceedance (12 consecutive control ticks); rates above
> 1.0 °C/s keep the fast 3-tick latch. A healthy light-roast turnaround
> spike (~3 s at 0.6 °C/s) therefore does not trip, while a genuine runaway
> still aborts within ~1 s. Both thresholds are provisional pending HIL
> calibration.
- `ERR probe_stuck_warning` — manual / Artisan software-PID mode only
  (Audit A-TC4-C): the bean probe has been flat (≤ 1 °C movement) for 120 s
  with the heater on. This is a WARNING, not a latch: the roast keeps
  running (a legitimately slow finish can hold BT flat for 2 min at low
  duty). If the probe stays flat for 300 s total, the detector escalates to
  the real latch, announced by `ERR safety_fault Probe stuck`. The warning
  is emitted once per stuck episode (reset by probe movement or heater off).

## 11. Protocol edge cases that matter

### Display scale persistence

`UNITS` affects output formatting state, but that state is not persisted across power cycles. A reconnect without reboot may preserve the last scale until a new `UNITS` command is sent.

### `FILT` permissiveness

The parser currently tolerates malformed `FILT` payloads by coercing bad values to zero instead of rejecting the command. That is documented as a technical risk in the internal bug report.

### `READ` vs `STATUS`

`READ` is for curve polling. `STATUS` is for operational introspection. Mixing those roles in an external client usually leads to confusion.

## 12. Compatibility boundary with the official Artisan app

LibreRoaster is compatible with Artisan where Artisan behaves like a serial controller speaking a TC4-flavored command set.

LibreRoaster is not intended to implement:

- Artisan’s `.alog` profile format,
- WebSocket device integration mode,
- Modbus integration mode,
- artisan.plus synchronization,
- vendor-specific device-driver behavior outside this serial surface.

That is why the protocol should be understood as **session compatibility**, not **ecosystem compatibility**.

## 13. Practical examples

### Basic startup sequence

```text
CHAN;1200
#1200
UNITS;C
#OK
FILT;70,70,70,70
#OK
READ
0.0,185.3,201.4,0.0,0.0
```

> The `#OK` acknowledgements for `UNITS`/`FILT` are required by Artisan's
> handshake: it raises `Arduino could not set temperature unit`/`... filters`
> on any non-`#` response and then re-initialises forever without ever
> polling `READ`.

### PID-driven session

Note that `PID;ON` and `PID;SV;...` are acknowledged **silently** — the
firmware does not emit an acknowledgement for them (only `UNITS`, `FILT`
and `REG` produce acknowledgement lines: `#OK`, `#OK`, `OK regression_started`
respectively). The wire transcript is:

```text
PID;ON
PID;SV;210
READ
0.0,185.3,201.4,0.0,0.0,75.0,45.0,210.0
```

### Deep diagnostics

```text
STATUS
120.3,150.5,75.0,50.0,1,0,none,0,0,150.5,88.5,37.1,-25.20,1,1,1,1250,5000,0,0
```

The STATUS line always carries **20 fields**; the final field is the
`FaultFlag` (`0` = normal, `1` = emergency fault active). Field 13
(`DerivativeValue`) is expressed in °C/min (or °F/min in Fahrenheit mode) —
the Artisan RoR convention — not °C/s.

## 14. Related documents

- `ARCHITECTURE.md` for the task and ownership model behind the protocol
- `ARTISAN_CONNECTION.md` for official Artisan configuration guidance
- `INSTRUMENTATION.md` for deep status-field interpretation
- `TESTING.md` for the test layers that pin the wire format
