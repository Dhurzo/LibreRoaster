# Instrumentation and Telemetry Guide

**Last Updated:** 2026-03-11 (v5.1 documentation update)
**Purpose:** Complete reference for LibreRoaster's instrumentation, telemetry, and safety monitoring capabilities.

---

## Overview

LibreRoaster provides comprehensive instrumentation for safety monitoring, watchdog health, guard timeouts, regression testing, and PID controller internals. This telemetry is exposed through the `STATUS` command (18-field CSV) to enable automation and auditing of the roaster's operational health.

**Key Documents:**
- [PROTOCOL.md](PROTOCOL.md) - Complete Artisan command reference including STATUS format
- [ARCHITECTURE.md](ARCHITECTURE.md) - System architecture and control loop details

---

## STATUS Command Telemetry

The `STATUS` command returns 18 CSV fields that provide a complete snapshot of the roaster's state:

```
ET,BT,Heater,Fan,WatchdogOK,WatchdogFailures,LastWatchdogReason,LEDCGuardTimeouts,RegressionActive,PV,MV,IntegratorValue,DerivativeValue,SaturationFlag,IntegratorClampFlag,DerivativeAvailableFlag,CommandLatency,MaxCommandLatency
```

### Field Descriptions

| Position | Field | Type | Description |
|----------|-------|------|-------------|
| 1 | ET | f32 (°C) | Environment/Exhaust Temperature |
| 2 | BT | f32 (°C) | Bean Temperature |
| 3 | Heater | f32 (%) | Heater SSR PWM output (0-100) |
| 4 | Fan | f32 (%) | Fan PWM output (0-100) |
| 5 | WatchdogOK | bool (0/1) | Watchdog feed success status |
| 6 | WatchdogFailures | u8 | Consecutive watchdog failure count |
| 7 | LastWatchdogReason | string | Failure reason token (or "none") |
| 8 | LEDCGuardTimeouts | u16 | Total LEDC guard timeout counter |
| 9 | RegressionActive | bool (0/1) | Over‑temperature regression test active |
| 10 | PV | f32 (°C) | Process Variable - current temperature used by PID |
| 11 | MV | f32 (%) | Manipulated Variable - controller output before clamping |
| 12 | IntegratorValue | f32 | PID integrator accumulation term |
| 13 | DerivativeValue | f32 (°C/s) | PID derivative rate term |
| 14 | SaturationFlag | bool (0/1) | PID output saturation active (wind‑up protection) |
| 15 | IntegratorClampFlag | bool (0/1) | Integrator clamping is active |
| 16 | DerivativeAvailableFlag | bool (0/1) | Derivative term is available (not inhibited) |
| 17 | CommandLatency | u32 (µs) | Most recent command processing latency |
| 18 | MaxCommandLatency | u32 (µs) | Maximum observed command latency |

---

## Safety Telemetry Categories

### 1. Watchdog Telemetry

The watchdog ensures the control loop runs reliably. If the watchdog doesn't get fed within the expected interval, it indicates a serious failure and may trigger a reset.

**Fields:**

#### WatchdogOK (Position 5, Type: bool)

Indicates whether the watchdog feed succeeded during the last control loop tick.

- **Value `1`**: Watchdog was successfully fed in the last 100ms tick
- **Value `0`**: Watchdog feed failed or timed out

**Implementation:** The control loop task calls `watchdog::feed()` every 100ms. If feed succeeds, `watchdog_feed_ok` is set to `true`. If feeding fails or times out, it's set to `false`.

**Code Reference:** `src/config/constants.rs` (SystemStatus::watchdog_feed_ok)

---

#### WatchdogFailures (Position 6, Type: u8)

Consecutive watchdog failure count. This counter increments each time the watchdog feed fails or times out.

- **Value `0`**: No consecutive failures (healthy)
- **Value `1-255`**: Number of consecutive failures

**Use Cases:**
- Automation monitors this field to detect firmware hangs or control loop starvation
- Rising values indicate problems with timing, blocking operations, or CPU starvation
- Threshold-based alerts can trigger remediation actions

**Code Reference:** `src/config/constants.rs` (SystemStatus::watchdog_consecutive_failures)

---

#### LastWatchdogReason (Position 7, Type: string)

The reason token explaining why the last watchdog feed failed. Returns `"none"` when the watchdog is healthy.

**Possible Values:**

| Reason Token | Description |
|--------------|-------------|
| `"none"` | Watchdog is healthy, no failures occurred |
| `"watchdog_init"` | Watchdog hardware initialization failed |
| `"watchdog_unavailable"` | Watchdog service not yet registered |
| `"watchdog_invalid_state"` | Feed failed due to invalid ESP-IDF state |
| `"watchdog_invalid_arg"` | Feed failed due to invalid argument |
| `"watchdog_feed_failed"` | Generic feed failure from ESP-IDF |
| `"watchdog_feed_error"` | Fallback error for unknown feed failures |

**Implementation:** When `watchdog_feed_ok` is `false`, this field is set to a descriptive token from the `WatchdogError::reason()` method. This enables auditors to understand *why* failures occurred, not just *that* they occurred.

**Use Cases:**
- Root cause analysis for watchdog failures
- Correlate with system logs to identify blocking operations
- Historical trending of failure types

---

### 2. LEDC Guard Telemetry

The LEDC (LED Controller) guard prevents PWM runaway by ensuring that LEDC write operations complete within a timeout window. If a write operation takes too long, the guard aborts it to prevent indefinite blocking.

**Fields:**

#### LEDCGuardTimeouts (Position 8, Type: u16)

Total count of LEDC guard timeout events. This counter increments whenever the `LedcGuard` detects a stalled LEDC operation.

- **Value `0`**: No LEDC timeouts have occurred
- **Value `1-65535`**: Total number of timeout events observed

**Implementation:** The `LedcGuard::try_acquire()` method attempts to acquire the LEDC lock. If the lock cannot be acquired within `LEDC_GUARD_TIMEOUT_MS` (40ms), the guard records a timeout via the `TIMEOUT_COUNTER` atomic counter.

**Code Reference:** `src/hardware/ledc_guard.rs` (record_timeout function)

**Safety Role:**
- Prevents the control loop from blocking indefinitely on a hung LEDC operation
- Enables early detection of hardware faults or peripheral lockups
- Timeout counter serves as an audit trail for stability issues

**Use Cases:**
- Detect LEDC peripheral faults
- Monitor PWM write latency trends
- Identify hardware that may be marginal or failing

---

### 3. Regression Telemetry

Over‑temperature regression testing is a safety validation feature that drives the system to 100% heater/fan output while monitoring temperature limits. This tests whether the safety systems (watchdog, emergency shutdown) operate correctly under stress.

**Fields:**

#### RegressionActive (Position 9, Type: bool)

Indicates whether an over‑temperature regression test sequence is currently active.

- **Value `1`**: Regression test is running (heater/fan at 100%, monitoring temperatures)
- **Value `0`**: Normal operation, no regression test in progress

**Implementation:** The `REG` command triggers `request_regression()` which sets `overtemp_regression_active` to `true`. The regression task monitors temperatures and will emergency‑stop if limits are exceeded.

**Code Reference:** `src/control/roaster_refactored.rs` (request_regression method)

**Safety Implications:**
- Regression sequences should **only** be triggered in controlled test environments
- System monitors temperature and will emergency‑stop at 260°C
- The `STATUS` command allows automation to verify `RegressionActive` is not stuck at `1`
- Safety logging emits `SAFETY OT-REGRESSION` records for external monitoring

**Use Cases:**
- Automated safety validation in test suites
- Verification that emergency shutdown works under thermal stress
- Audit trail of when regression tests were executed

**Warning:** Do not trigger regression tests in production roasting environments. The 100% heater/fan output can cause over‑temperature conditions that require manual intervention.

---

## PID Controller Internal State

LibreRoaster uses a PID (Proportional-Integral-Derivative) controller for temperature regulation. The `STATUS` command exposes internal PID state for debugging and tuning.

**Fields:**

#### PV - Process Variable (Position 10, Type: f32)

The current temperature measurement used as input to the PID controller (Process Variable).

- **Units:** Celsius (°C)
- **Source:** `SystemStatus::bean_temp` (BT) or `env_temp` (ET) depending on control mode

**Purpose:** The PV is what the controller is trying to regulate. The PID calculates error as `error = setpoint - PV`, then adjusts the MV (Manipulated Variable) accordingly.

**Code Reference:** `src/config/constants.rs` (SystemStatus::pv), `src/control/pid.rs` (compute method)

---

#### MV - Manipulated Variable (Position 11, Type: f32)

The controller output value before clamping to actuator limits.

- **Units:** Percentage (0-100)
- **Range:** May temporarily exceed 0-100 before clamping, but output is clamped to safe range

**Purpose:** The MV represents the PID's desired output. Before being sent to the SSR, the MV is:
1. Clamped to the actuator's physical limits (0-100%)
2. Applied with saturation detection
3. Used as the final command value

**Code Reference:** `src/control/pid.rs` (compute method, line ~154)

---

#### IntegratorValue (Position 12, Type: f32)

The accumulated integral term of the PID controller.

- **Units:** Error·time (error integrated over time)
- **Behavior:** Accumulates error over time to eliminate steady-state error

**Purpose:** The integrator term corrects for long‑term error between setpoint and actual temperature. A high `IntegratorValue` indicates persistent error that the integrator is actively correcting.

**Clamping:** The integrator is clamped to prevent wind‑up (unbounded growth) when the actuator is saturated. See `IntegratorClampFlag` for clamping status.

**Code Reference:** `src/control/pid.rs` (integrator field), line ~101

---

#### DerivativeValue (Position 13, Type: f32)

The rate of change of the temperature (derivative term).

- **Units:** °C/second
- **Calculation:** Difference between current and previous temperature samples

**Purpose:** The derivative term provides predictive control. If the temperature is rising quickly, the derivative term anticipates the need to reduce heating, improving responsiveness.

**Availability:** The `DerivativeAvailableFlag` indicates whether a valid derivative value is available. Derivative may be inhibited during startup or when data is insufficient.

**Code Reference:** `src/control/pid.rs` (derivative_rate field), line ~107

---

#### SaturationFlag (Position 14, Type: bool)

Indicates whether the PID output is currently at the actuator limit (saturation).

- **Value `1`**: PID output is clamped to 0% or 100% (actuator at limit)
- **Value `0`**: PID output is within actuator range (unsaturated)

**Purpose:** Saturation detection is part of the anti‑windup strategy. When saturated, the integrator stops accumulating to prevent wind‑up (unbounded integrator growth that causes overshoot when the setpoint changes).

**Implementation:** The PID controller sets `saturation_active` to `true` when the MV is clamped to the output limit.

**Code Reference:** `src/control/pid.rs` (saturation_active field), line ~149

---

#### IntegratorClampFlag (Position 15, Type: bool)

Indicates whether the integrator is currently being clamped to prevent wind‑up.

- **Value `1`**: Integrator is clamped (not accumulating)
- **Value `0`**: Integrator is accumulating normally

**Purpose:** This flag works with `SaturationFlag` to implement anti‑windup protection. When the output is saturated, the integrator is frozen to prevent it from growing unbounded while the actuator can't respond.

**Implementation:** When the output is saturated and the integrator would increase the error, the PID sets `integrator_clamped` to `true` instead of adding to the accumulator.

**Code Reference:** `src/control/pid.rs` (integrator_clamped field), line ~137

---

#### DerivativeAvailableFlag (Position 16, Type: bool)

Indicates whether a valid derivative value is available.

- **Value `1`**: Derivative term is computed and valid
- **Value `0`**: Derivative is inhibited (insufficient data or startup)

**Purpose:** The derivative term requires at least two temperature samples to compute a rate. During startup or after a reset, the derivative is temporarily unavailable.

**Use Cases:**
- Filter out spurious derivative values during initialization
- Signal to tuning tools when derivative data is not reliable
- Prevent control instability from derivative noise during early operation

**Code Reference:** `src/config/constants.rs` (SystemStatus::derivative_available)

---

## Command Latency Telemetry

Latency monitoring measures the time taken to process Artisan commands, enabling detection of performance regressions or blocking operations.

**Fields:**

#### CommandLatency (Position 17, Type: u32)

Most recent command processing latency in microseconds.

- **Units:** Microseconds (µs)
- **Measurement:** Time from command receipt to completion

**Purpose:** Tracks real‑time performance of the command processing pipeline. Low latency ensures responsive control and timely safety actions.

**Implementation:** Latency is measured in the application layer (control_loop_task) by capturing timestamps before and after command processing. This avoids circular dependencies with hardware drivers.

**Typical Values:**
- **< 1000 µs**: Excellent performance
- **1000-5000 µs**: Normal operation
- **> 5000 µs**: Potential blocking or performance issue

**Code Reference:** `src/config/constants.rs` (SystemStatus::command_latency_us)

---

#### MaxCommandLatency (Position 18, Type: u32)

Maximum command latency observed since system startup.

- **Units:** Microseconds (µs)
- **Reset On:** System reset or initialization

**Purpose:** Provides a peak latency metric for identifying worst‑case performance. Spikes in `MaxCommandLatency` may indicate:
- Blocking operations in the control loop
- Interrupt conflicts
- Peripheral contention

**Use Cases:**
- Detect performance regressions after code changes
- Identify worst‑case command processing time for timing analysis
- Validate that latency thresholds are met in acceptance testing

**Code Reference:** `src/config/constants.rs` (SystemStatus::max_command_latency_us)

---

## Using STATUS for Automation

The `STATUS` command is designed for automation and auditing use cases:

### Safety Monitoring

Automation scripts can poll `STATUS` at regular intervals (e.g., every 1-5 seconds) to monitor:

1. **Watchdog Health:** `WatchdogOK` should be `1`, `WatchdogFailures` should be `0`
2. **Guard Stability:** `LEDCGuardTimeouts` should be low or not increasing
3. **Regression State:** `RegressionActive` should be `0` during normal operation

### Performance Monitoring

Track command latency over time:

```bash
# Extract latency from STATUS response
# Response: 120.3,150.5,75.0,50.0,1,0,none,0,0,150.5,88.5,37.1,-0.42,1,1,1,1250,5000
# CommandLatency is position 17 (1250)
# MaxCommandLatency is position 18 (5000)
```

Alert on:
- `CommandLatency` consistently exceeding threshold (e.g., > 5000 µs)
- `MaxCommandLatency` increases over time (performance regression)

### PID Tuning

PID internal state fields enable data‑driven tuning:

1. **Set PID gains** and monitor response curves
2. **Check `IntegratorClampFlag`** during tuning – frequent clamping may indicate gains are too high
3. **Verify `DerivativeAvailableFlag`** is consistently `1` – derivative should contribute to control
4. **Observe `SaturationFlag`** patterns – saturation indicates actuator limits are being hit

---

## Telemetry Integration

### Example STATUS Response

```
120.3,150.5,75.0,50.0,1,0,none,0,0,150.5,88.5,37.1,-0.42,1,1,1,1250,5000
```

### Field Mapping

| Field | Value | Interpretation |
|-------|--------|---------------|
| ET | 120.3 | Exhaust temp 120.3°C |
| BT | 150.5 | Bean temp 150.5°C |
| Heater | 75.0 | Heater at 75% |
| Fan | 50.0 | Fan at 50% |
| WatchdogOK | 1 | Watchdog healthy |
| WatchdogFailures | 0 | No consecutive failures |
| LastWatchdogReason | none | No recent failures |
| LEDCGuardTimeouts | 0 | No LEDC timeouts |
| RegressionActive | 0 | No regression test |
| PV | 150.5 | PID process variable (BT) |
| MV | 88.5 | PID output before clamping |
| IntegratorValue | 37.1 | Integral accumulator |
| DerivativeValue | -0.42 | Temperature falling at 0.42°C/s |
| SaturationFlag | 1 | Output at limit |
| IntegratorClampFlag | 1 | Integrator clamped |
| DerivativeAvailableFlag | 1 | Derivative valid |
| CommandLatency | 1250 | Last command took 1.25ms |
| MaxCommandLatency | 5000 | Worst‑case 5ms observed |

---

## Related Documentation

- **[PROTOCOL.md](PROTOCOL.md)** - Complete Artisan command reference with STATUS format
- **[ARCHITECTURE.md](ARCHITECTURE.md)** - System architecture, control loop, and task structure
- **[HARDWARE.md](HARDWARE.md)** - Hardware specifications, pinout, and wiring
- **[DEVELOPMENT.md](DEVELOPMENT.md)** - Build, flash, test, and debugging guides

---

*Last Updated: 2026-03-11*

## TRACE Stream & Parser

Every Artisan command that touches the queue is now tagged with a `TraceId` so engineers can follow the exact journey from the command entry point through the queue, actuator, telemetry emission, and guard/watchdog safety checks. Each TRACE line is emitted on the same host channel that services `STATUS`, so the `TraceId` stays attached to queue depth, actuator outputs, PID telemetry, and guard states for the entire lifecycle. The instrumentation sits in [`src/logging/traceability.rs`](src/logging/traceability.rs) and writes the strings through the shared `ServiceContainer` output channel.

### TRACE event series & fields

Lines follow the format `TRACE,<TraceId>,<event>,key=value,...`. The `cmd` field is produced with Rust's `Debug` formatter (e.g., `cmd=ReadStatus` or `cmd=ArtisanCommand::STATUS`), so the parser always knows which enum variant drove the event. The runtime emits the following events:

1. **`queue_enqueue`** – command entered the Artisan queue. Fields: `cmd`, `channel`, `depth`, `fallback`. When the queue overflow path reroutes a command the same data is emitted as `queue_fallback` with `fallback=1`; this makes retries immediately visible in the matrix.
2. **`queue_dequeue`** – command removed for execution. The event repeats `cmd`, `channel`, and `depth` so the matrix shows both enqueue and dequeue depths.
3. **`queue_fallback`** – emitted whenever the queue overflow handler resubmits a command. It uses the same fields as `queue_enqueue` but flags `fallback=1` so hosts can detect the fallback path without inspecting the control loop.
4. **`actuation`** – SSR/fan outputs written before actuating the traced command. Fields: `cmd`, `channel`, `ssr`, `fan`, `latency_us`, `saturation_active`.
5. **`telemetry`** – guard and watchdog telemetry snapshot taken while the command is running. Fields: `guard_timeout`, `guard_timeouts`, `watchdog`, plus optional `error_category`/`error_source` when an `AppError` is attached.
6. **`guard`** – guard/watchdog reporting tied to the same `TraceId`. Fields mirror the telemetry event: `guard_timeout`, `guard_timeouts`, `watchdog`, optional `watchdog_failure`, and optional `error_category`/`error_source` metadata.

The queue fields drive the `QueueDepth` column in the regression matrix so you can clearly see whether a command hit the fallback path before it actuated.

### Sample TRACE entries

Use this excerpt (taken from `logs/traceability/sample-trace.log`) to validate regression output formatting:

```
TRACE,1,queue_enqueue,cmd=ReadStatus,channel=Uart,depth=5,fallback=0
TRACE,1,actuation,cmd=ReadStatus,channel=Uart,ssr=33.5,fan=12,latency_us=150,saturation_active=1
TRACE,1,telemetry,guard_timeout=0,guard_timeouts=0,watchdog=ok
TRACE,1,guard,guard_timeout=0,guard_timeouts=0,watchdog=ok
```

Because the `cmd` value is Debug-formatted, the log above is consistent with the parser expectations and can be compared directly with coverage captures from debug builds.

### Regression triage workflow

Run the host parser after every regression capture to rebuild the command → queue → actuator → telemetry → guard matrix:

```
python scripts/traceability_matrix.py <trace.log>
```

The script now understands the runtime names `queue_enqueue`, `queue_dequeue`, `queue_fallback`, `actuation`, `telemetry`, and `guard`, so it always produces the same column layout auditors expect. It also normalizes the Debug-formatted `cmd` field (e.g., `ReadStatus`) and shows `fallback=1` in the matrix whenever the queue overflow path retried a command. Point auditors to `logs/traceability/sample-trace.log` if their TRACE stream diverges so they can trace the difference to the firmware event that led to the regression outcome.

### Safe-Shutdown Trace Replay

InitError failures now emit guard events with `watchdog_failure=init_error_failure` plus the `error_category`/`error_source` metadata before the LED blink loop begins. Auditors can replay the new safe-shutdown path with `logs/traceability/sample-safe-shutdown.log`, which strings together queue/actuation/telemetry/guard lines carrying `error_category=initialization` and `error_source=hardware_init_failed`.

To capture and replay the safe-shutdown trace:

1. Build the firmware with `--features embedded` and flash a device.
2. Force `init_hardware()` to produce an `Err` (for example, by unplugging a temperature sensor or mocking the initializer) so `enter_safe_shutdown()` runs.
3. Observe the host TRACE output—the guard line should include `watchdog_failure=init_error_failure` and the AppError metadata from `AppError::Initialization`.
4. Replay the capture with `python scripts/traceability_matrix.py logs/traceability/sample-safe-shutdown.log` to confirm the regression row highlights the failure path.

The regression test `scripts/test_traceability_matrix.py` now exercises this log so the parser continually proves the safe-shutdown scenario stays visible in the matrix.

### Safe-Shutdown Replay Artifact

Auditors need a single bundle that captures the log, the parsed matrix, and the guard metadata so the InitError trace can be shipped and replayed without hardware. The helper CLI collects these pieces into `safe-shutdown-replay.zip` under `logs/traceability`.

```bash
python scripts/collect_safe_shutdown.py \
  --log logs/traceability/sample-safe-shutdown.log \
  --output logs/traceability/safe-shutdown-replay.zip \
  --artifact-name safe-shutdown-replay \
  --force
```

Inside the artifact you will find:

- `sample-safe-shutdown.log` – the raw TRACE capture that emitted `watchdog_failure=init_error_failure` and the AppError metadata fields.
- `traceability.csv` – the queue → actuator → telemetry → guard matrix generated by `scripts/traceability_matrix.py` so inspectors can follow the failure path step-by-step.
- `metadata.json` – key diagnostics for the guard row, including `TraceId`, `watchdog_failure`, `error_category`, and `error_source` (the values at the time of this failure are `watchdog_failure=init_error_failure`, `error_category=initialization`, `error_source=hardware_init_failed`).
- `README.txt` – a snapshot of the guard failure details plus replay instructions.

To replay the guard matrix from the artifact, unzip it and run the parser against the extracted log:

```bash
unzip logs/traceability/safe-shutdown-replay.zip -d /tmp/safe-shutdown
python scripts/traceability_matrix.py /tmp/safe-shutdown/sample-safe-shutdown.log
```

The matrix output will highlight the guard row with the same `TraceId` and failure metadata that auditors need to prove the diagnostics path.

Inspect `metadata.json` directly (e.g., `jq '.' metadata.json`) if you need to cite the guard failure values in a report. The artifact README reinforces the same failure summary so reviewers can validate the packaged contents without running any commands.

### Related documentation

Whenever another section of this guide references TRACE instrumentation or regression triage, keep the event names and parser usage aligned with `src/logging/traceability.rs` and `scripts/traceability_matrix.py`. This keeps all cross-references in sync with runtime behavior.
