# Phase 102 Research: Safe-Shutdown Diagnostics

## Context
- v5.2 milestone audit (2026-03-20) still calls out gaps in the diagnostics chain: AppError metadata never left `src/error/app_error.rs`, `enter_safe_shutdown()` previously awaited timers synchronously, and the TRACE parser was not consuming the runtime event names emitted by the guard/telemetry helpers.
- Phases 95-101 now cover the build fix, error taxonomy, trace parser alignment, and documentation updates; the release still needs a traceable failure flow so auditors can tie `InitError` events back to the TRACE matrix.

## Observations
- `traceability.rs` already formats guard/telemetry events with AppError metadata but there is no exposure point for `InitError` failures that happen before the control loop begins.
- `enter_safe_shutdown()` in `src/main.rs` builds an Artisan error line and a blinking LED loop but never emits TRACE events that downstream tooling depends on.
- `scripts/traceability_matrix.py` consumes logs that include queue/actuation steps but has no sample covering guard events that arise from initialization failures.
- `internalDoc/INSTRUMENTATION_README.MD` and `logs/traceability/sample-trace.log` describe the happy path but not the safe-shutdown failure flow.

## Focus
1. Surface `InitError` diagnostics as AppError metadata in guard events before the LED blink loop takes over.
2. Extend the parser/test/docs/log artifacts so gsd-plan-checker can verify the safe-shutdown trace path.
3. Produce a reproducible sample log and reproduction guidance that auditors can run with the trace parser.

## References
- `@.planning/v5.2-MILESTONE-AUDIT.md`
- `@.planning/v5.2-v5.2-MILESTONE-AUDIT.md`
- `@.planning/STATE.md`
- `src/logging/traceability.rs`
- `src/main.rs`
- `scripts/traceability_matrix.py`
- `scripts/test_traceability_matrix.py`
- `internalDoc/INSTRUMENTATION_README.MD`
