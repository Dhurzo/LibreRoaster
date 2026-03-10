# Modernization Automation

`./scripts/run-modernization.sh` orchestrates Rust modernization runs while capturing the evidence auditors need.

## Running the script
1. Launch via `./scripts/run-modernization.sh` (no args required). The script uses `RUN_ID` derived from UTC timestamp + random suffix and writes all logs to `logs/modernization/$RUN_ID/`.
2. Each step (`fmt`, `fix`, `clippy`) writes stdout/stderr to `logs/modernization/$RUN_ID/step-XX-<name>.log` while also streaming to the console for real-time feedback.
3. A `summary.txt` file is generated in the same directory with the following entries:
   - `run_id` and `log_path`
   - `unsafe_register_changes` (empty placeholder; updated manually per run)
   - `skip_reason` (optional, see next section)

## Recording skip reasons
When modernization intentionally skips behavior-critical modules, set `SKIP_REASON` as an environment variable (e.g., `SKIP_REASON="hardware stabilization" ./scripts/run-modernization.sh`). The script appends `skip_reason = "<reason>"` to the summary so auditors immediately know why a module was deferred.

## Summary artifacts
- Auditors should start at `logs/modernization/$RUN_ID/summary.txt` to see the run ID, log directory, and any skip rationale.
- The `unsafe_register_changes` entry is the input for `quality/modernization/modernization-summary.md`, which highlights register deltas and references the log directory for every recorded run.

## Related references
- Modernization summaries (see `quality/modernization/modernization-summary.md`) aggregate these reports before milestone reviews.
- Run logs live under `logs/modernization/`—do not delete them until verification is complete.
