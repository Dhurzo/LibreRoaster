# Logging & Telemetry — Current Design

**Last updated:** 2026-10-17
**Scope:** how the firmware emits diagnostics and telemetry today. Source is truth; no benchmark claims.

## Edge-gated warnings

Persistent conditions are logged once per activation episode via `EdgeLogGate`
(`src/logging/edge_log_gate.rs`): `warn!` on the inactive → active transition,
`debug!` while the condition stays active, re-armed on the falling edge. Gate sites:

- FAN-FLOOR interlock — command path (`src/control/roaster_control.rs:435`) and control-loop path (`:1115`)
- PID setpoint above the channel limit (`target_cap_gate`, `:2561`)
- SSR cycle busy (`cycle_busy_gate`, `src/control/controllers/actuator.rs:132`)

The LEDC spin guard (`src/hardware/ledc_guard.rs`) has no edge-gated warning: a timeout increments a counter exposed as the STATUS field `ledc_guard_timeouts` (u16). A per-tick `debug!` trace is emitted only when a timeout happened, and `debug!` is visible only in `instrumentation`/`test` builds (the logger defaults to `Warn`).

## TRACE events (opt-in)

With `instrumentation` (or the host `test` feature), `src/logging/traceability.rs` exposes the event API used by the control pipeline:

- `trace_command_enqueue` / `trace_queue_dequeue`
- `trace_actuation`
- `trace_telemetry`
- `trace_guard`

Each event is a `String<TRACE_EVENT_MAX_LEN = 256>` correlated by a monotonic
`TraceId` (`AtomicU32`) and routed through the shared output channel with
`try_send_output` (drops counted). Without `instrumentation`/`test`, the
formatting bodies compile to nothing — release builds pay no formatting cost.

## Roast ring buffer (`#DUMP`)

`src/logging/roast_logger.rs` keeps the last `LOG_CAPACITY = 256` CSV samples
(`time_s,bt,et,heater,fan,target,ror`) at 1 Hz (~4.3 min). `#DUMP` renders an
8 KiB aggregate buffer (`DUMP_BUFFER_SIZE = 8192`, oldest rows discarded if
needed to preserve the roast tail) drained at ≤4 rows per tick
(`MAX_DUMP_ROWS_PER_TICK = 4`), so a full ring drains in ~20 s. Rows are staged
in `RoasterControl.dump_pending` (`String<DUMP_ROW_CAPACITY = 128>`, sized
`LOG_CAPACITY + 1`); a row that cannot be sent is re-pushed to the front, so no
row is lost to output-channel backpressure. The dump feed follows
`roast_logger::is_logging_active()`, independent of the `STREAM;ON` flag.

## Output path

- `dual_output_task` drains at most `MAX_MESSAGES_PER_TICK = 4` messages per
  5 ms invocation, routing each to the active transport.
- Continuous `#` telemetry is gated to 1 Hz wall-clock
  (`DEFAULT_OUTPUT_INTERVAL_MS = 1000`) regardless of the control tick.
