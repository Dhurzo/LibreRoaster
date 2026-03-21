# Phase 97: Traceability Matrix Tooling - Research

**Researched:** 2026-03-20
**Domain:** Embedded regression triage instrumentation
**Confidence:** MEDIUM

## Summary

Traceability is the missing piece that prevents regression triage from confidently mapping a failing command back through the queue, actuator, telemetry, and guard layers described in SOLID-03. The runtime already emits stage instrumentation (`STAGE` entries), queue metrics, and an 18-field STATUS snapshot, but those signals are disconnected: there is no stable command identifier that threads through the queue/timer/guard stack and nothing on the host can produce a matrix from the emitted strings. This research concludes that the fastest path to SOLID-03 is:

1. Add a lightweight `TRACE` stream that assigns every Artisan command a `TraceId` and records `command`, `queue depth`, `actuator output`, `telemetry` snapshot, and `guard/watchdog` state with that ID. The instrumentation lives in `src/logging/traceability.rs`, reuses the existing `ServiceContainer` output channel, and piggybacks on `CommandQueue` entries so the data stays with the command as it flows.
2. Introduce host tooling (`scripts/traceability_matrix.py`) that parses the `TRACE` log, groups events by `TraceId`, and produces the command → queue → actuator → telemetry → guard table required by regression triage auditors.
3. Document the new stream and tooling inside `INSTRUMENTATION_README.MD` so future triage sessions can reconstruct regressions from logged artifacts (sample trace data is stored under `logs/traceability`).

## Standard Stack

| Library | Version | Role |
|---------|---------|------|
| `heapless` | 0.9.x | String builder for deterministic trace records (already in use for `STAGE`). |
| `embassy` | existing runtime | Task scheduling and channels (already orchestrating `queue_processor_task`). |
| `python` | 3.13+ | Host-side trace parser script (matches other scripts under `scripts/`). |
| `csv` | 1.4.0 | Optional dependency for host script output formatting (already pinned). |

## Architecture Approach

- **Trace surrogate**: Introduce `TraceId` + `TracedCommand` for the hardware queues so command metadata is carried from `uart/usb` through `queue_processor_task` into the control loop. `TraceId` is a `u32` drawn from an atomic counter, so the trace stream is compact and deterministic.
- **Event series**: Emit `TRACE,<trace_id>,<step>,key=value,...` strings for `command_enqueue`, `queue_dequeue`, `actuator_output`, `telemetry_emit`, and `guard_report`. Each entry uses the existing Artisan output channel so no new transport is needed.
- **Host parser**: `scripts/traceability_matrix.py` reads the log, groups events by trace, and outputs a table (`TraceId`, command name, queue depth, SSR/Fan, telemetry snapshot, guard/watchdog state). This script can consume both regression logs and live telemetry captures.

## Pitfalls / Risks

1. **Channel type churn**: Introducing `TracedCommand` touches every place that interacts with `ServiceContainer::get_artisan_channel()`, so tests and utilities must adapt to the new struct.
2. **Queue overflow**: Trace metadata must stay aligned with commands, even if the queue bypass is triggered (e.g., queue full) or commands are dropped. The instrumentation must emit `TRACE` entries in those fallback paths as well.
3. **Host tooling drift**: The parser must tolerate mixed logs (`TRACE`, `STATUS`, warnings). The script should gracefully skip lines it doesn't understand and focus on lifecycle events.

## Evidence / Sources

- Runtime telemetry: `src/application/tasks.rs` (control loop, `MutableArtisanFormatter`, watchdog updates).
- Stage instrumentation: `src/application/stage_instrumentation.rs` (deterministic string builder pattern to copy for `TRACE`).
- Command queue: `src/hardware/uart/tasks.rs`, `src/hardware/usb_cdc/tasks.rs`, and `src/input/mod.rs` (queue structure and queue depth instrumentation already in place).
- Requirement: `.planning/REQUIREMENTS.md` SOLID-03 and `.planning/milestones/v5.2-ROADMAP.md` (traceability matrix goal).
- Documentation reference: `internalDoc/INSTRUMENTATION_README.MD` (current telemetry/automation section that needs supplementing).

## Open Questions

1. **Guard detail fidelity**: Should the guard event include both LEDC guard timeout count and watchdog failure reason, or can we rely on `STATUS` output? The plan currently duplicates the same data from `STATUS` so the matrix remains self-contained.
2. **Sample retention**: Where should regression traces live for audit? The plan writes sample logs to `logs/traceability`, but we should confirm if that directory should remain under git version control.

## Execution Notes

- The trace stream emits zero-alloc strings built with `heapless::String<128>` just like `stage_instrumentation`, so runtime impact stays minimal.
- The parser runs on the host (`python` script) and can be invoked after regression runs to produce the matrix for auditors.

**Research date:** 2026-03-20
**Ready for planning:** yes
