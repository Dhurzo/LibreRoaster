# Phase 103 Research: Safe-Shutdown Artifact Replay

## Context
- Phase 102 delivered guard TRACE events with AppError metadata, a `logs/traceability/sample-safe-shutdown.log` fixture, and instrumentation documentation that describes how to capture/replay InitError traces.
- Auditors now need deterministic evidence bundles (log + metadata + matrix) so they can verify the failure path without running hardware.

## Observations
- `scripts/traceability_matrix.py` already parses TRACE logs into the queue/actuator/telemetry/guard matrix and is capable of summarizing guard failures.
- `scripts/test_traceability_matrix.py:test_safe_shutdown_log_replays_guard_failure` already ensures the sample log contains `watchdog_failure=init_error_failure` plus AppError fields.
- The instrumentation doc currently describes capturing the log but lacks procedural guidance for packaging and archiving the evidence or rerunning the parser against an artifact.

## Focus
1. Provide a host CLI that bundles the safe-shutdown log, a metadata summary, and the parsed matrix into a `safe-shutdown-replay.zip` artifact for auditors.
2. Make the CLI reusable for any TRACE log so the artifact always includes `TraceId`, `watchdog_failure`, `error_category`, and `error_source` details along with the parser output.
3. Update regression coverage and documentation so testers/auditors know how to run the CLI, inspect the artifact, and rerun the parser/tests that prove the guard row is preserved.

## References
- `.planning/STATE.md`
- `.planning/ROADMAP.md`
- `.planning/phases/102-diagnostics-verification/102-VERIFICATION.md`
- `scripts/traceability_matrix.py`
- `scripts/test_traceability_matrix.py`
- `logs/traceability/sample-safe-shutdown.log`
- `internalDoc/INSTRUMENTATION_README.MD`
