# Fault Injection Scenario Matrix

This document defines the watchdog/guard/comms fault-injection scenarios used for safety verification.

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

The `STATUS` command returns a 20-field CSV line (`ArtisanFormatter::format_status_response` in `src/output/artisan.rs`), each field feeding either telemetry, watchdog/guard diagnostics, or fault indicators.

```
et,bt,heater,fan,watchdog_flag,failure_count,failure_reason,guard_timeouts,regression_flag,pv,mv,integrator_value,derivative_value,saturation_flag,integrator_clamp_flag,derivative_available_flag,command_latency_us,max_command_latency_us,temp_scale_indicator,fault_flag
```

`validation_runner.py` persists STATUS fields 1–18 (zero-based 0–17) after a leading `timestamp`
column, so `telemetry.csv` has 19 columns. The last two STATUS fields, `TempScale` and `FaultFlag`,
are **not persisted** by the current runner; check them on live `STATUS` and record the result in
the report.

> The manifest's `expected_columns` and scenario metadata use descriptive names:
> `env_temp` = `et`, `bean_temp` = `bt`, `ssr_output` = `heater`, `fan_output` = `fan`,
> `watchdog_feed_ok` = `watchdog_flag`, `watchdog_consecutive_failures` = `failure_count`,
> `ledc_guard_timeouts` = `guard_timeouts`, and `fault_condition` = `fault_flag`. The last field
> is live-STATUS only.

### Column Mapping & Golden CSV Linkage

Indices below are **STATUS field indices**. `telemetry.csv` prepends a `timestamp` column and
contains fields 0–17.

| Index | Field | Description | Golden CSV Use |
|-------|-------|-------------|----------------|
| 0 | et | Environmental temperature (display scale) | Telemetry snapshot for audits |
| 1 | bt | Bean temperature (display scale) | Telemetry snapshot for audits |
| 2 | heater | SSR heater output (0-100%) | Telemetry snapshot for audits |
| 3 | fan | Fan output (0-100%) | Telemetry snapshot for audits |
| 4 | watchdog_flag | 1 if watchdog feed OK, 0 if failed | Watchdog health; copied into golden CSV to prove latency thresholds |
| 5 | failure_count | Consecutive watchdog failures | Watchdog latency indicator; auditors expect 0 for golden runs |
| 6 | failure_reason | "none", "feed_failed", "guard_timeout", etc. | Fault flag detail captured in golden CSV |
| 7 | guard_timeouts | LEDC guard timeout counter | Guard metric copied into golden CSV |
| 8 | regression_flag | 1 if overtemp regression active | Fault flag; alerts auditors if non-zero (not golden) |
| 9 | pv | Process variable (filtered temperature) | Telemetry snapshot for trend analysis |
| 10 | mv | Manipulated variable (heater output) | Telemetry snapshot for trend analysis |
| 11 | integrator_value | PID integrator value | Telemetry snapshot (supporting diagnostics) |
| 12 | derivative_value | PID derivative rate (display units per minute) | Telemetry snapshot (supporting diagnostics) |
| 13 | saturation_flag | 1 if actuator saturated | Telemetry snapshot (supporting diagnostics) |
| 14 | integrator_clamp_flag | 1 if integrator clamped | Telemetry snapshot (supporting diagnostics) |
| 15 | derivative_available_flag | 1 if derivative calculated | Telemetry snapshot (supporting diagnostics) |
| 16 | command_latency_us | Last command latency in microseconds | Latency evidence copied into golden CSV |
| 17 | max_command_latency_us | Maximum observed command latency in microseconds | Latency evidence copied into golden CSV |
| 18 | temp_scale_indicator | 0 = Celsius, 1 = Fahrenheit | Live STATUS only; not in `telemetry.csv` |
| 19 | fault_flag | 1 if the emergency/fault latch is active | Live STATUS only; not in `telemetry.csv` |

## Evidence Output

Each scenario execution produces telemetry rows with the STATUS fields (written by `validation_runner.py`):

```
timestamp,et,bt,heater,fan,watchdog_flag,failure_count,failure_reason,guard_timeouts,regression_flag,pv,mv,integrator_value,derivative_value,saturation_flag,integrator_clamp_flag,derivative_available_flag,command_latency_us,max_command_latency_us
```

### Golden Outputs

- **Capture runs:** `validation_runner.py` writes each run to `tests/hardware/runs/{scenario_id}/{timestamp}/telemetry.csv` (one row per STATUS poll or per executed STATUS command), plus `read_telemetry.csv` for READ rows and `telemetry.csv.json` metadata (scenario id, manifest entry, run id, max latencies, run directory).
- **Golden path:** the manifest declares `golden_output: tests/hardware/goldens/{scenario_id}.csv` for the approved golden copy. `tests/hardware/goldens/` is a promotion target — it is created when a validated run is copied there, not by the capture runner.
- **Manifest pointer:** Every scenario ID in this matrix links to `tests/hardware/scenario_manifest.json` for the command_sequence, expected_columns, golden_output path, and retention window. Use the manifest to translate a scenario into the sequence automation must execute and the golden artifact it must produce.
- Every golden output referenced here must exist under `tests/hardware/goldens/{scenario_id}.csv` before the manifest entry is marked as audit-ready. The repository does not ship these files; create them by promoting validated runs.

### Golden Run Criteria

- **Stable telemetry:** `et`, `bt`, `heater`, and `fan` remain within 2°C/2% of their pre-fault values for the duration of the run.
- **Watchdog behavior:** `watchdog_flag=1`, `failure_count=0`, and `failure_reason="none"` (latency recorded but tolerated) for golden runs unless the scenario deliberately triggers a watchdog fault; the manifest metadata clarifies the expected `fault_condition`.
- **Guard counts:** `guard_timeouts` stays below 3; values greater than that must be documented in the golden CSV with rationale before promotion.
- **Retention:** Promoted golden artifacts must remain available for **60 days** as defined in the methodology (`metadata.retention_days` in the manifest records the window so operators know how long auditors may request the artifact).
- **Manifest correlation:** Every WD/GD/CM entry here must have a manifest record so automation and auditors can correlate `scenario_id` with command sequences and expected telemetry columns.

## Running Scenarios

```bash
# Run the fault-injection regression harness (requires both features: `test`
# provides the host Embassy time driver, `regression` compiles the test file —
# without it the command executes 0 tests)
cargo test --target x86_64-unknown-linux-gnu --features test,regression --test fault_injection_scenarios
```

The Rust harness covers a small set of safety regressions (overtemp shutdown,
latch persistence, STOP recovery, cooldown fan). The full WD-/GD-/CM- matrix
is exercised through the manifest with the HIL runner:

```bash
python tests/hardware/validation_runner.py --port /dev/ttyACM0 --scenario WD-01
```
