# Instrumentation and Telemetry

This document is the semantic reference for the 20-field `STATUS` line, the optional `TRACE` stream,
and the diagnostic tooling around them. Wire grammar and command syntax belong to
[`PROTOCOL.md`](PROTOCOL.md); runtime ownership and timing belong to
[`ARCHITECTURE.md`](ARCHITECTURE.md) and [`CONTROL_LOOP.md`](CONTROL_LOOP.md).

---

## STATUS Command

`STATUS` and `STAT` return one CSV line with exactly 20 fields. The field order is a wire contract;
the implementation is `format_status_response()` in `src/output/artisan.rs`.

```text
ET,BT,Heater,Fan,WatchdogOK,WatchdogFailures,LastWatchdogReason,LEDCGuardTimeouts,RegressionActive,PV,MV,IntegratorValue,DerivativeValue,SaturationFlag,IntegratorClampFlag,DerivativeAvailableFlag,CommandLatency,MaxCommandLatency,TempScale,FaultFlag
```

| # | Field | Type | Meaning |
|---|---|---|---|
| 1 | `ET` | f32 | Environment/exhaust temperature in the active display scale. |
| 2 | `BT` | f32 | Bean temperature in the active display scale. |
| 3 | `Heater` | f32 % | SSR heater output, 0–100. |
| 4 | `Fan` | f32 % | Fan output, 0–100. |
| 5 | `WatchdogOK` | 0/1 | Software watchdog feed succeeded during the last control tick. |
| 6 | `WatchdogFailures` | u8 | Consecutive watchdog feed failures. |
| 7 | `LastWatchdogReason` | token | Last failure reason, or `none`. |
| 8 | `LEDCGuardTimeouts` | u16 | Total LEDC guard acquisition timeouts. |
| 9 | `RegressionActive` | 0/1 | Over-temperature regression workflow is running. |
| 10 | `PV` | f32 | PID process variable in the active display scale. |
| 11 | `MV` | f32 % | PID manipulated variable before actuator clamping. |
| 12 | `IntegratorValue` | f32 | PID integral accumulator. |
| 13 | `DerivativeValue` | f32 | PID derivative in °C/min (or °F/min), the Artisan RoR convention. |
| 14 | `SaturationFlag` | 0/1 | PID output is at an actuator limit. |
| 15 | `IntegratorClampFlag` | 0/1 | PID anti-windup is holding the integrator. |
| 16 | `DerivativeAvailableFlag` | 0/1 | A valid derivative is available. |
| 17 | `CommandLatency` | u32 µs | Most recent command-processing latency. |
| 18 | `MaxCommandLatency` | u32 µs | Peak command latency since startup. |
| 19 | `TempScale` | 0/1 | `0` = Celsius, `1` = Fahrenheit. |
| 20 | `FaultFlag` | 0/1 | Emergency/fault latch is active. |

Temperature-bearing fields follow `UNITS`; percentages and PID terms that are not temperatures do
not. `DerivativeValue` is always a per-minute rate, not a per-second rate.

### Example

```text
120.3,150.5,75.0,50.0,1,0,none,0,0,150.5,88.5,37.1,-25.20,1,1,1,1250,5000,0,0
```

This is a healthy Celsius session: ET 120.3 °C, BT 150.5 °C, 75 % heater, 50 % fan, watchdog
healthy, no regression, PID output at its limit, derivative −25.20 °C/min, 1.25 ms last command
latency, 5 ms peak, no fault.

---

## Watchdog and Guard Telemetry

`WatchdogOK` reflects the software watchdog feed performed once per control tick. The watchdog
timeout is 1000 ms, sized for the real tick cadence (≈310–330 ms because of the MAX31856
conversion wait), not the nominal 100 ms timer.

`LastWatchdogReason` is a diagnostic token, not a stable API:

| Token | Meaning |
|---|---|
| `none` | Healthy; no failure recorded. |
| `watchdog_init` | Watchdog initialization failed. |
| `watchdog_unavailable` | Watchdog service was not registered. |
| `watchdog_timeout` | Feed missed the software timeout window. |

`LEDCGuardTimeouts` counts failed attempts to acquire the LEDC guard. The guard prevents a stalled
PWM write from blocking the control loop indefinitely; the timeout is `LEDC_GUARD_TIMEOUT_MS`.
The counter is monotonic and is exposed for trend detection, not as a per-tick alarm.

For automation, the normal baseline is `WatchdogOK=1`, `WatchdogFailures=0`, and a
`LEDCGuardTimeouts` value that is stable. A rising failure count or repeated new timeout events
points to timing pressure, blocking work, or peripheral contention.

---

## Regression Telemetry

`REG` is a diagnostic-only workflow. On builds with the `regression` feature it drives heater and
fan to 100 %, requests an immediate emergency shutdown, then replays the HIL fixtures
(`warm_normal`, `cold_negative`, `bean_open_fault`) while feeding the watchdog. It emits
`SAFETY OT-REGRESSION` on success and `SAFETY OT-REGRESSION-ABORTED <reason>` if a foreign latch is
already armed at entry or the shutdown write fails.

`RegressionActive=1` means that workflow is running. It must return to `0` after the run. On builds
without the feature the regression task is a no-op and `REG` answers `ERR regression_disabled`; a
second request while one is pending answers `ERR regression_busy`.

Do not trigger `REG` during a production roast. The 100 % actuator output is a test condition, and
the over-temperature guard remains a separate always-on layer.

---

## PID Internals

The PID fields expose the controller state used for tuning and diagnosis:

- `PV` is the temperature the PID is regulating: BT or ET depending on `PID;CHAN`.
- `MV` is the controller output before the actuator's final clamp.
- `IntegratorValue` is the accumulated error term; it is held by anti-windup when the output is
  saturated.
- `DerivativeValue` is the process rate in display units per minute.
- `SaturationFlag` means the output is at 0 % or 100 %.
- `IntegratorClampFlag` means the integrator is being held to prevent wind-up.
- `DerivativeAvailableFlag` is `0` while there are not enough samples for a valid derivative.

Frequent saturation with a clamped integrator usually means the gains or actuator range are
mismatched. A derivative flag that remains `0` during a steady roast means derivative data is not
being used.

---

## Command Latency

`CommandLatency` measures the most recent command path in microseconds; `MaxCommandLatency` keeps
the peak since startup. Both are measured in the application/control layer to avoid coupling the
formatter to hardware drivers.

Typical values are below 1000 µs for short commands and a few milliseconds under load. A sustained
rise or a growing peak is a signal to inspect blocking work, channel pressure, or peripheral
contention. Treat the thresholds as operational hints, not protocol guarantees.

---

## Automation Patterns

Poll `STATUS` every 1–5 seconds while a host is connected. Useful checks:

1. `WatchdogOK == 1` and `WatchdogFailures == 0`.
2. `LEDCGuardTimeouts` is not increasing.
3. `RegressionActive == 0` outside an explicit `REG` run.
4. `FaultFlag == 0`; if it is `1`, stop sending re-energizing commands and follow the recovery path
   in [`PROTOCOL.md`](PROTOCOL.md).
5. `TempScale` matches the host's expected unit before interpreting temperatures.

`READ` remains the curve-polling command. Use `STATUS` for health, diagnostics, and automation.

---

## TRACE Stream

Commands that touch the queue are tagged with a `TraceId`, allowing a capture to follow one command
from ingress through actuation, telemetry, and guard checks. TRACE lines are emitted on the host
channel that services `STATUS`; the implementation is `src/logging/traceability.rs`.

Format:

```text
TRACE,<TraceId>,<event>,key=value,...
```

`cmd` values use Rust's `Debug` formatter (for example `cmd=ReadStatus`). The live events are:

1. `queue_enqueue` — command entered the queue. Fields: `cmd`, `channel`, `depth`, `fallback`.
2. `actuation` — actuator write attempted. Fields: `cmd`, `channel`, `ssr`, `fan`, `latency_us`,
   `saturation_active`.
3. `telemetry` — guard/watchdog snapshot while the command runs. Fields: `guard_timeout`,
   `guard_timeouts`, `watchdog`, optional `error_category`/`error_source`.
4. `guard` — guard/watchdog report tied to the same `TraceId`, with the same fields and optional
   failure metadata.

`queue_dequeue` and `queue_fallback` are not emitted by the current firmware: there is no dequeue
event, and the overflow path drops rather than resubmits.

### Sample

```text
TRACE,1,queue_enqueue,cmd=ReadStatus,channel=Uart,depth=5,fallback=0
TRACE,1,actuation,cmd=ReadStatus,channel=Uart,ssr=33.5,fan=12,latency_us=150,saturation_active=1
TRACE,1,telemetry,guard_timeout=0,guard_timeouts=0,watchdog=ok
TRACE,1,guard,guard_timeout=0,guard_timeouts=0,watchdog=ok
```

Sample logs are not shipped in the repository. Generate a capture from a real run or from the
traceability tests before pointing an auditor at one.

### Regression triage

Build the command → queue → actuator → telemetry → guard matrix from a capture:

```bash
python scripts/traceability_matrix.py <trace.log>
```

The parser understands the four live events, normalizes the Debug-formatted command names, and
produces the stable matrix used by regression triage.

### Safe-shutdown replay

Initialization failures emit guard metadata (`watchdog_failure=init_error_failure`,
`error_category=initialization`, `error_source=hardware_init_failed`) before the LED blink loop.
Capture that trace, then package or replay it without hardware:

```bash
python scripts/collect_safe_shutdown.py \
  --log logs/traceability/sample-safe-shutdown.log \
  --output logs/traceability/safe-shutdown-replay.zip \
  --artifact-name safe-shutdown-replay \
  --force

python scripts/replay_safe_shutdown.py \
  --artifact logs/traceability/safe-shutdown-replay.zip \
  --report replay-report.json
```

`collect_safe_shutdown.py` bundles the raw log, the generated `traceability.csv`, guard metadata,
and replay instructions. `replay_safe_shutdown.py` regenerates the matrix and compares the
`TraceId`, `watchdog_failure`, `error_category`, and `error_source` values with the bundled
metadata; a mismatch exits non-zero. The artifact is intended for audit evidence, not normal
operation.

---

## Related Documents

- [`PROTOCOL.md`](PROTOCOL.md) — command grammar, response framing, and error tokens.
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — runtime ownership and timing.
- [`CONTROL_LOOP.md`](CONTROL_LOOP.md) — tick stages and telemetry emission.
- [`SAFETY_CASE.md`](SAFETY_CASE.md) — hazard evidence behind the guards.
- [`DEVELOPMENT.md`](DEVELOPMENT.md) — test and trace commands.

---

*Last updated: 2026-10-22.*
