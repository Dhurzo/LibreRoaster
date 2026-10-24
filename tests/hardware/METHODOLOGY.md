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
- **Executing scenarios:** Run `validation_runner.py` with the `--manifest tests/hardware/scenario_manifest.json` flag. The manifest's `command_sequence` string describes the CLI/API steps to reproduce the scenario (e.g., `prepare_watchdog; drop_watchdog_feed; check_status`), and `expected_columns` lists the STATUS/READ fields the scenario expects. Names such as `env_temp`, `bean_temp`, and `fault_condition` are aliases for `et`, `bt`, and `fault_flag`. The current runner persists 19 columns; `TempScale` and `FaultFlag` must be checked on live STATUS. (The Rust fault-injection harness is separate: `cargo test --target x86_64-unknown-linux-gnu --features test,regression --test fault_injection_scenarios`.)
- **Reference artifacts:** The manifest's `golden_output` path points to the approved CSV at `tests/hardware/goldens/{scenario_id}.csv`. This directory is a promotion target that is created when a validated run is copied there — the capture runner writes only under `tests/hardware/runs/`. The report links the promotion path; comparing a new run with the golden artifact is a manual review step, not an automated gate.

## Artifact Tracking & Retention
- **Telemetry deposits:** Validation runner deposits raw telemetry rows at `tests/hardware/runs/{scenario_id}/{timestamp}/telemetry.csv` (plus `read_telemetry.csv` for READ rows in `--hardware-mode`) and a companion `telemetry.csv.json` metadata file that repeats the run id, the scenario id, the manifest entry, the max command latency, and the run directory. Auditors use these directories to verify each golden run, independently of the golden CSV.
- **Naming guidance:** Use `{scenario_id}` exactly as listed in `SCENARIO_MATRIX.md` and `scenario_manifest.json`; each capture run gets a compact UTC timestamp directory. For example, `tests/hardware/runs/WD-01/20260320T120000Z/telemetry.csv` and the promoted `tests/hardware/goldens/WD-01.csv`.
- **Retention policy:** This evidence retention policy keeps every promoted golden CSV/metadata pair for **60 days** as documented in the manifest's `retention_days` field, and ensures `tests/hardware/runs/{scenario_id}` retains audit artifacts for at least the same window. (Archive rotation is a manual process step; no tooling currently moves files.)
- **Golden run checklist:** Before promoting a run to golden status, verify:
  - Stable telemetry: `et`, `bt`, `heater`, and `fan` stay within 2°C/2% of their pre-fault baselines.
  - Guard/watchdog counts: `guard_timeouts < 3` and `failure_count`/`watchdog_flag` match the scenario's `metadata` expectations.
  - Columns present: the persisted STATUS columns are present in `telemetry.csv`. `TempScale` and
    `FaultFlag` are not persisted; record their live-STATUS values in the report.
  - Retention readiness: The `evidence_owner` (`libreroaster/audits`) has been notified, the run artifacts live under `tests/hardware/runs/`, and the approved copy is at the manifest's `golden_output` path for 60 days.
