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

The `STATUS` command returns a CSV tail with 16 columns, each of which feeds either telemetry, watchdog/guard diagnostics, or fault indicators. The golden CSV that documents every run copies the telemetry fields that auditors validate.

```
env_temp,bean_temp,ssr_output,fan_output,watchdog_flag,failure_count,failure_reason,guard_timeouts,regression_flag,pv,mv,integrator,derivative,saturation,integrator_clamp,derivative_available
```

### Column Mapping & Golden CSV Linkage

| Index | Field | Description | Golden CSV Use |
|-------|-------|-------------|----------------|
| 0 | env_temp | Environmental temperature (Celsius) | Telemetry snapshot for audits |
| 1 | bean_temp | Bean temperature (Celsius) | Telemetry snapshot for audits |
| 2 | ssr_output | SSR heater output (0-100%) | Telemetry snapshot for audits |
| 3 | fan_output | Fan output (0-100%) | Telemetry snapshot for audits |
| 4 | watchdog_flag | 1 if watchdog feed OK, 0 if failed | Watchdog health; copied into golden CSV to prove latency thresholds |
| 5 | failure_count | Consecutive watchdog failures | Watchdog latency indicator; auditors expect 0 for golden runs |
| 6 | failure_reason | "none", "feed_failed", "guard_timeout", etc. | Fault flag detail captured in golden CSV |
| 7 | guard_timeouts | LEDC guard timeout counter | Guard metric copied into golden CSV |
| 8 | regression_flag | 1 if overtemp regression active | Fault flag; alerts auditors if non-zero (not golden) |
| 9 | pv | Process variable (filtered temperature) | Telemetry snapshot for trend analysis |
| 10 | mv | Manipulated variable (heater output) | Telemetry snapshot for trend analysis |
| 11 | integrator | PID integrator value | Telemetry snapshot (supporting diagnostics) |
| 12 | derivative | PID derivative rate | Telemetry snapshot (supporting diagnostics) |
| 13 | saturation | 1 if actuator saturated | Telemetry snapshot (supporting diagnostics) |
| 14 | integrator_clamp | 1 if integrator clamped | Telemetry snapshot (supporting diagnostics) |
| 15 | derivative_available | 1 if derivative calculated | Telemetry snapshot (supporting diagnostics) |

## Evidence Output

Each scenario execution produces a golden CSV row with:

```
timestamp,scenario_id,env_temp,bean_temp,ssr_output,fan_output,watchdog_feed_ok,failure_count,failure_reason,guard_timeouts,fault_condition
```

### Golden Outputs

- **Naming convention:** `tests/hardware/goldens/{scenario_id}.csv` for the latest approved golden output, `tests/hardware/goldens/{scenario_id}/{timestamp}.csv` for archival runs, and `tests/hardware/goldens/{scenario_id}/{timestamp}.json` for metadata. The manifest describes the corresponding CSV path for each scenario so automation knows where to compare columns.
- **Run directory:** `tests/hardware/runs/{scenario_id}/{timestamp}.csv` stores raw telemetry rows in the same schema, and an accompanying `metadata.json` contains command status, command_sequence, and the manifest entry id so auditors can reconcile steps with evidence.
- **Manifest pointer:** Every scenario ID in this matrix links to `tests/hardware/scenario_manifest.json` for the command_sequence, expected_columns, golden_output path, and retention window. Use the manifest to translate a scenario into the sequence automation must execute and the golden artifact it must produce.
- Every Golden output referenced here must exist under `tests/hardware/goldens/{scenario_id}.csv` before the manifest entry is marked as audit-ready.

### Golden Run Criteria

- **Stable telemetry:** `env_temp`, `bean_temp`, `ssr_output`, and `fan_output` remain within 2°C/2% of their pre-fault values for the duration of the run.
- **Watchdog behavior:** `watchdog_feed_ok=1`, `failure_count=0`, and `failure_reason="none"` (latency recorded but tolerated) for golden runs unless the scenario deliberately triggers a watchdog fault; the manifest metadata clarifies the expected `fault_condition`.
- **Guard counts:** `guard_timeouts` stays below 3; values greater than that must be documented in the golden CSV with rationale before promotion.
- **Checksum and retention:** Each golden CSV and metadata file must include a checksum field inside its JSON companion and remain available for **60 days** as defined in the methodology. The manifest records the retention window so operators know how long auditors may request the artifact.
- **Manifest correlation:** Every WD/GD/CM entry here must have a manifest record so automation and auditors can correlate `scenario_id` with command sequences and expected telemetry columns.

## Running Scenarios

```bash
# Run all fault injection scenarios
cargo test --test fault_injection_scenarios

# Run specific category
cargo test --test fault_injection_scenarios watchdog
cargo test --test fault_injection_scenarios guard
cargo test --test fault_injection_scenarios comms
```
