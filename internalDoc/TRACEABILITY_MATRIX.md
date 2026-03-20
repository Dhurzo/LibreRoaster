# Traceability Matrix Reference

This checklist gives regression triage leads the commands, artifacts, and column meanings they need to recreate the command→queue→actuator→telemetry→guard matrix without reading the parser code.

## Trace capture recipe

1. Build and run the embedded control loop with tracing enabled:
   - `cargo run --features embedded --package libre-roaster --bin artisan-controller` (or flash the binary and start it the same way).
   - Connect Artisan to the device over USB/serial and capture the full output stream.
2. Include the `TRACE` stream by enabling Artisan tracing (hosts now emit `TRACE` lines alongside `STATUS` and `DEBUG`).
3. Dump the captured stream into `logs/traceability/<your-run>.log`. Keep the `logs/traceability/sample-trace.log` file as the regression reference for formatting and timing expectations.

## Parser command

Run the host-side parser to squash TRACE lines into a triage table:

```
python scripts/traceability_matrix.py logs/traceability/<your-run>.log
```

Use `logs/traceability/sample-trace.log` when validating the extractor or teaching auditors, then substitute your own run when investigating regressions.

## Matrix columns

| Column | Meaning | Notes |
|--------|---------|-------|
| `TraceId` | Unique identifier assigned at enqueue time. Use it to follow a command through the pipeline. | TraceIds increment per command and never mix between channels. |
| `Command` | Artisan command name (`STATUS`, `GUARD_CHECK`, etc.). | Matches the `command` field in `command_enqueue`. |
| `QueueDepth` | Human-readable queue depth + channel info. | Combines `queue_depth` and `channel` when available; depth increases before dequeue, drops to zero once the control loop grabs the command. |
| `Actuator` | SSR/fan outputs (`ssr`, `heater_pwm`, `fan_pwm`). | Emitted during `actuator_output` and shows what the control loop commanded for that TraceId. |
| `Telemetry` | PID telemetry (`ET`, `BT`, `PV`, `MV`). | Captured during `telemetry_emit`; compare against STATUS logging to ensure instrumentation alignment. |
| `Guard` | Guard and watchdog health for the TraceId (fields like `guard_state`, `watchdog`, `guard_timeout`). | Empty guard rows usually mean the command crashed before guard_report ran; guard_timeout > 0 or watchdog ≠ `ok` signals protective action. |

## Triage checklist

1. Confirm a fresh log was saved under `logs/traceability/` before running the parser so regression evidence can be archived.
2. Run `scripts/traceability_matrix.py` against the log and capture the resulting CLI table (copy/paste into the regression ticket if needed).
3. Look for TraceIds whose `QueueDepth` never drops to zero, whose `Actuator` output never shows the expected SSR command, or whose `Guard` column signals timeouts.
4. Correlate poor `Telemetry` values (ET/BT/PV/MV) with the guard output to understand whether watchdogs fired because the PID hung or because actuator writes blocked.

## Guard and watchdog interpretation

- `guard_state=armed` (or similar) means the guard is monitoring the operation; `guard_state=idle` means no guard action was needed for that TraceId.
- `watchdog=ok` is the expected healthy state; any other token (e.g., `reset`, `feed_failed`) notes why the watchdog did not accept the feed.
- `guard_timeout` increments when LEDC guard or other watchdog mechanisms abort the command (typically due to blocked SSR writes). If the matrix shows guard_timeout > 0, inspect the matching command’s actuator outputs and queue depth to spot what hung.
- If `Guard` is empty while `QueueDepth` remains high, the command never reached `guard_report` – hunt for blocking operations earlier in the control loop.

## Troubleshooting: stuck TraceIds

- A stuck TraceId is any identifier that reappears without completing all five steps or that keeps `QueueDepth` > 0 across multiple minutes.
- To resolve:
  1. Check the actuator output and telemetry to see if the control loop wrote to SSR/fan; if not, a hardware lockup likely prevented progress.
  2. Inspect `guard_report` — if the guard is still armed with a previous watchdog reason, there may be a driver-level timeout.
 3. Re-run the parser after clearing logs; compare against `logs/traceability/sample-trace.log` to ensure repeated commands look similar. If they diverge, the regression is likely a queue starvation or a guard/watchdog regression.

## Next steps for regression triage

- Archive the `.log` and the parser output together to demonstrate end-to-end traceability for SOLID-03 compliance.
- When problems repeat, share the parsed matrix with the responsive team and highlight which column first diverged from the healthy sample.
