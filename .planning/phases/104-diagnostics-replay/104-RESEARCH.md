# Phase 104 Research: Safe-Shutdown Artifact Replay Automation

## Context
- Phase 103 shipped `scripts/collect_safe_shutdown.py` plus regression coverage and docs so auditors can bundle a safe-shutdown TRACE log, matrix, metadata, and README into `safe-shutdown-replay.zip`.
- Auditors and QA reviewers now want a way to replay that artifact headlessly so they can confirm the guard metadata still matches the runtime trace without rerunning hardware.

## Observations
- The artifact contains `metadata.json` with `TraceId`, `watchdog_failure`, `error_category`, and `error_source`, but the archived metadata isn’t automatically revalidated after the artifact is built.
- The sample log replay `scripts/test_traceability_matrix.py` already parses the trace but doesn’t verify that a zipped artifact can be decompressed and reprocessed.
- Documentation describes how to build the artifact but lacks guidance on how to unpack it, rerun the matrix, and compare metadata values during audits.

## Focus
1. Automate artifact replay so zipped safe-shutdown bundles can be validated by CI/auditors without hardware: unzip, parse the log, regenerate the matrix, and compare to `metadata.json` (TraceId/watchdog/error fields).
2. Extend regression coverage so the replay automation runs against `safe-shutdown-replay.zip` (rebuilt via `scripts/collect_safe_shutdown.py`) and ensures metadata stays consistent.
3. Document the replay automation so auditors know how to run the CLI, inspect metadata, and trust the zipped artifact’s trace output.

## References
- `logs/traceability/sample-safe-shutdown.log`
- `logs/traceability/safe-shutdown-replay.zip`
- `.planning/phases/103-diagnostics-artifacts/103-VERIFICATION.md`
- `.planning/phases/103-diagnostics-artifacts/103-01-PLAN.md`
- `scripts/collect_safe_shutdown.py`
- `scripts/traceability_matrix.py`
- `scripts/test_traceability_matrix.py`
- `internalDoc/INSTRUMENTATION_README.MD`
