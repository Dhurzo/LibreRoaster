# Fault Injection Scenario Matrix

This document defines the watchdog/guard/comms fault-injection scenarios for SOLID-02 verification.

## Scenario Categories

### Watchdog Fault Scenarios

| Scenario ID | Description | Expected STATUS Flags |
|-------------|-------------|----------------------|
| WD-01 | Watchdog feed succeeds normally | `watchdog_feed_ok=1`, `watchdog_consecutive_failures=0`, `fault_condition=0` |
| WD-02 | Single watchdog feed failure | `watchdog_feed_ok=0`, `watchdog_consecutive_failures=1`, `fault_condition=0` |
| WD-03 | Multiple consecutive watchdog feed failures | `watchdog_feed_ok=0`, `watchdog_consecutive_failures>=3`, `fault_condition=1` |
| WD-04 | Watchdog feed recovers after failure | `watchdog_feed_ok=1`, `watchdog_consecutive_failures=0`, `fault_condition=0` |

### LEDC Guard Timeout Scenarios

| Scenario ID | Description | Expected STATUS Flags |
|-------------|-------------|----------------------|
| GD-01 | No LEDC guard timeouts | `ledc_guard_timeouts=0`, `fault_condition=0` |
| GD-02 | Single LEDC guard timeout | `ledc_guard_timeouts=1`, `fault_condition=0` |
| GD-03 | Multiple LEDC guard timeouts | `ledc_guard_timeouts>=3`, `fault_condition=1` |
| GD-04 | LEDC guard timeout with watchdog healthy | `ledc_guard_timeouts>=1`, `watchdog_feed_ok=1`, `fault_condition=0` |

### Communication Fault Scenarios

| Scenario ID | Description | Expected STATUS Flags |
|-------------|-------------|----------------------|
| CM-01 | Normal communication, no faults | `fault_condition=0`, all channels responding |
| CM-02 | USB CDC channel fails, UART responds | `fault_condition=0`, multiplexer fallback active |
| CM-03 | Both channels fail (command timeout) | `fault_condition=1`, no responding channels |
| CM-04 | Partial command received, buffer timeout | `fault_condition=0`, error logged |

## STATUS Telemetry Format

The `STATUS` command returns a CSV tail with 16 columns:

```
env_temp,bean_temp,ssr_output,fan_output,watchdog_flag,failure_count,failure_reason,guard_timeouts,regression_flag,pv,mv,integrator,derivative,saturation,integrator_clamp,derivative_available
```

### Column Mapping

| Index | Field | Description |
|-------|-------|-------------|
| 0 | env_temp | Environmental temperature (Celsius) |
| 1 | bean_temp | Bean temperature (Celsius) |
| 2 | ssr_output | SSR heater output (0-100%) |
| 3 | fan_output | Fan output (0-100%) |
| 4 | watchdog_flag | 1 if watchdog feed OK, 0 if failed |
| 5 | failure_count | Consecutive watchdog failures |
| 6 | failure_reason | "none", "feed_failed", "guard_timeout", etc. |
| 7 | guard_timeouts | LEDC guard timeout counter |
| 8 | regression_flag | 1 if overtemp regression active |
| 9 | pv | Process variable (filtered temperature) |
| 10 | mv | Manipulated variable (heater output) |
| 11 | integrator | PID integrator value |
| 12 | derivative | PID derivative rate |
| 13 | saturation | 1 if actuator saturated |
| 14 | integrator_clamp | 1 if integrator clamped |
| 15 | derivative_available | 1 if derivative calculated |

## Evidence Output

Each scenario execution produces a CSV row with:

```
timestamp,scenario_id,env_temp,bean_temp,ssr_output,fan_output,watchdog_feed_ok,failure_count,failure_reason,guard_timeouts,fault_condition
```

## Running Scenarios

```bash
# Run all fault injection scenarios
cargo test --test fault_injection_scenarios

# Run specific category
cargo test --test fault_injection_scenarios watchdog
cargo test --test fault_injection_scenarios guard
cargo test --test fault_injection_scenarios comms
```
