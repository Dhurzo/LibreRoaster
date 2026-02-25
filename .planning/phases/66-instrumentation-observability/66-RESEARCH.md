# Research: Instrumentation Observability (Phase 66)

**Domain:** LibreRoaster instrumentation telemetry
**Researched:** 2026-02-23
**Confidence:** MEDIUM

## Observability Scope

- The project now records watchdog feed health, guard timeouts, and over-temperature regression activity inside `SystemStatus` (`src/config/constants.rs`). Each field is updated inside the control loop (`src/application/tasks.rs`), so the snapshot already exists in RAM.
- The current `READ` command remains the production telemetry channel and must continue returning four values (ET, BT, heater, fan). Extending `READ` would break existing clients, so a new Artisan command is needed for richer instrumentation data.
- Automation and auditors need deterministic, parseable telemetry that highlights watchdog failures, guard hits, and whether a regression ran. CSV with constant column positions (booleans as `0/1`, counts, and reason strings without commas) keeps the parser simple.

## Format Design

- Instrumentation payload should include the usual ET/BT/heater/fan for continuity plus the new fields: watchdog feed success flag, consecutive failure count, last failure reason (stable tokens per `WatchdogError::reason()`), LEDC guard timeout count, and regression active flag.
- The response must remain small enough for the Artisan serial channel but structured enough for automation (6‑7 values). Using string quotes is not necessary; keep values unquoted and comma-separated.

## Documentation + Automation

- Update `internalDoc/INSTRUMENTATION_README.MD` so the harnesses and auditors understand how the new `STATUS` payload maps to watchdog/guard/regression checks. Include an example response and describe how automation should interpret each column.

## Next Steps

- Implement the `STATUS` command inside the parser, RoasterControl, and output task so automation can push the formatted string to the output channel.
- Add targeted tests for the parser (`src/input/parser.rs`) and formatter (`src/output/artisan.rs`) to prevent regressions in the instrumentation payload.
