# LibreRoaster Protocol Reference

**Last updated:** 2026-05-02

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
- Response: `OK`

This updates the display conversion used by `READ`, `STATUS`, and temperature-bearing PID/setpoint output.

### `FILT;<values>`

- Purpose: compatibility acknowledgement for Artisan startup configuration
- Response: `OK`

Current behavior is intentionally shallow: the firmware acknowledges the command and stores only the first parsed value as protocol state. It does not implement the richer semantics that a desktop application might assume from the filter name.

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
13. PID derivative value
14. Saturation flag
15. Integrator clamp flag
16. Derivative-available flag
17. Last command latency in microseconds
18. Maximum observed command latency in microseconds
19. Temperature-scale flag (`0` = Celsius, `1` = Fahrenheit)
20. Fault condition flag (`0` = normal, `1` = emergency fault active)

`STATUS` is the right interface for anything that needs runtime health, not just roast temperatures.

## 6. Manual actuator commands

### `OT1 <0-100>` / `OT1;<0-100>`

Sets heater power percentage. Accepts both space and semicolon delimiter (Artisan default).

### `OT2 <0-100>` / `OT2;<0-100>`

Sets fan speed with decimal input support. Accepts both delimiters.

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

### `IO3 <0-100>` / `IO3;<0-100>`

Sets fan speed as an integer-oriented command path. Accepts both delimiters.

### `UP` / `DOWN`

Increment or decrement heater output in 5% steps.

### `STOP`

Emergency stop path. Heater is cut and fan is forced to 100%.

## 7. PID and roast-control commands

LibreRoaster accepts both Artisan-standard semicolon-delimited PID commands and a limited set of legacy comma variants.

### Supported forms

- `PID;ON`
- `PID;OFF`
- `PID;SV;<temp>`
- `PID;T;<kp>;<ki>;<kd>`
- `PID;CHAN;<1-4>`
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
- PID channel accepts `1..=4`

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

### `#DUMP`

Requests the roast ring-buffer dump.

This is also a LibreRoaster-specific extension and should be treated as a debugging/forensics surface rather than a guaranteed Artisan integration feature.

## 10. Error responses and acknowledgements

The formatter emits simple line-oriented acknowledgements and errors:

- `OK`
- `#<value>` for `CHAN`
- `ERR ...` for failures

Handler-level failures are emitted as `ERR handler_failed <token>:<source>`
(e.g. `ERR handler_failed invalid_state:profile_temp_out_of_range`, `ERR
handler_failed invalid_state:fault_condition_active`). Valid tokens:
`temperature_out_of_range`, `sensor_fault`, `invalid_state`, `pid_error`,
`hardware_error`, `emergency_shutdown`. The `:source` suffix is a
diagnostic discriminator, not a stable contract — client code should not
assume a rich structured error taxonomy.

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
OK
FILT;70,70,70,70
OK
READ
0.0,185.3,201.4,0.0,0.0
```

### PID-driven session

```text
PID;ON
OK
PID;SV;210
OK
READ
0.0,185.3,201.4,0.0,0.0,75.0,45.0,210.0
```

### Deep diagnostics

```text
STATUS
120.3,150.5,75.0,50.0,1,0,none,0,0,150.5,88.5,37.1,-0.42,1,1,1,1250,5000,0
```

## 14. Related documents

- `ARCHITECTURE.md` for the task and ownership model behind the protocol
- `ARTISAN_CONNECTION.md` for official Artisan configuration guidance
- `INSTRUMENTATION_README.MD` for deep status-field interpretation
- `ARTISAN_COMPATIBILITY_REPORT.md` for a broader compatibility assessment
