# Hardware Validation Methodology

## Command Latency
Latency is measured as the time delta (in microseconds) between the arrival/dequeue of an Artisan command and the completion of its respective handler. This measures the internal firmware processing overhead and ensures the control loop remains responsive.

## Thermal Envelope
The thermal envelope represents the stability of the heating control. It is calculated as the absolute difference between the actual Bean Temperature (BT) and the target temperature during a stable heating phase (post-preheat, during a soak or constant rate of rise).

## Safety Metrics
- **Watchdog Fails**: Any consecutive watchdog failure is considered a critical safety breach.
- **LEDC Guard Timeouts**: Occasional timeouts are acceptable if the system recovers, but excessive timeouts indicate potential hardware or scheduling contention.

## Scenario Manifest–Driven Runs
- **Locate scenarios:** The single source of truth for scenario IDs, command_sequence, expected telemetry columns, golden outputs, and retention windows is `tests/hardware/scenario_manifest.json`. Each entry mirrors the table in `SCENARIO_MATRIX.md` and includes the same WD/GD/CM prefixes so automation and auditors can crosswalk between prose and metadata.
- **Executing scenarios:** Run `validation_runner` or the fault_injection test harness with the `--manifest tests/hardware/scenario_manifest.json` flag. The manifest's `command_sequence` string describes the CLI/API steps to reproduce the scenario (e.g., `prepare_watchdog; drop_watchdog_feed; check_status`), and `expected_columns` enumerates the `STATUS` columns that every golden CSV must include for that scenario.
- **Reference artifacts:** The manifest's `golden_output` path points to the approved CSV under `tests/hardware/goldens/{scenario_id}.csv`. Automation compares new runs against this golden artifact before a run can be promoted for audit evidence.

## Artifact Tracking & Retention
- **Telemetry deposits:** Validation runner deposits raw sensor rows at `tests/hardware/runs/{scenario_id}/{timestamp}.csv` and a companion `metadata.json` that repeats the scenario_id, the manifest entry id, the executed command_sequence, and the checksum of the CSV. Auditors use these directories to verify each golden run, independently of the golden CSV.
- **Naming guidance:** Use `{scenario_id}` exactly as listed in `SCENARIO_MATRIX.md` and `scenario_manifest.json`; append an ISO8601 timestamp for archival runs. For example, `tests/hardware/runs/WD-01/2026-03-20T12-00-00Z.csv` and `tests/hardware/goldens/WD-01.csv`.
- **Retention policy:** This evidence retention policy keeps every golden CSV/metadata pair for **60 days** as documented in the manifest's `retention_days` field, and ensures `tests/hardware/runs/{scenario_id}` retains audit artifacts for at least the same window. After 60 days, rotate files into the `archive/` subtree while keeping a pointer in `scenario_manifest.json`.
- **Golden run checklist:** Before promoting a run to golden status, verify:
  - Stable telemetry: `env_temp`, `bean_temp`, `ssr_output`, and `fan_output` stay within 2°C/2% of their pre-fault baselines.
  - Guard/watchdog counts: `guard_timeouts < 3` and `failure_count`/`watchdog_feed_ok` match the scenario's `metadata` expectations.
  - Checksum present: The CSV metadata JSON contains both CSV and manifest checksums, and the guard/watchdog columns match the scenario's `expected_columns` list.
  - Retention readiness: The `evidence_owner` (`libreroaster/audits`) has been notified, and the files are stored under the `tests/hardware/goldens/` and `tests/hardware/runs/` hierarchies for 60 days.
