# LibreRoaster HIL Validation Playbook

## Purpose

This HIL validation playbook documents how contributors turn **manifest-driven HIL runs** into auditor-ready artifacts that satisfy the HW-03 golden-output requirements. Follow it step-by-step so every scenario run captures the same metadata, analysis, and packaging expectations auditors rely on.

## Manifest-first workflow

1. **Read the manifest** at `tests/hardware/scenario_manifest.json`. Each entry contains:
   - `id`, `category`, `description`, and `command_sequence` so you know which scenario is being exercised.
   - A `golden_output` path under `tests/hardware/goldens/` and associated `metadata.retention_days` / `evidence_owner` values.
   - Threshold expectations inside the manifest metadata for auditors (watchdog passes, guard timeouts, fault conditions).
2. **Pick a scenario** by its `id`. The scenario manifest drives the `validation_runner` filters, the `analysis.py` report, and the `tests/hardware/goldens/*.csv` reference.
3. **Ensure the manifest stays unchanged** while you run the scenario; copy it into your artifact bundle if you document a new golden output.

## Running `validation_runner`

Use the runner to stream `STATUS` telemetry from the ESP32-C3 and persist both CSV data and metadata JSON. Example command:

```bash
python tests/hardware/validation_runner.py \
  --port /dev/ttyACM0 \
  --manifest tests/hardware/scenario_manifest.json \
  --scenario WD-01 \
  --runs-dir tests/hardware/runs \
  --metadata-suffix .json
```

- `--port` and `--baud` match your Artisan connection (defaults 115200).
- `--interval` controls how often `STATUS` is polled (default 1s).
- The runner creates `tests/hardware/runs/<SCENARIO>/<TIMESTAMP>/telemetry.csv` plus `<telemetry.csv>.json` metadata.
- Metadata includes run ID, max latency, manifest pointers, and the scenario entry used for the run.

Stop the runner with `Ctrl+C`. When it exits gracefully, it writes metadata beside the CSV so you know which manifest entry produced each stream.

## Running `analysis.py`

Turn CSV + metadata into markdown reports using the thresholds in `tests/hardware/thresholds.json`:

```bash
python tests/hardware/analysis.py \
  --thresholds tests/hardware/thresholds.json \
  --template tests/hardware/report_template.md \
  --manifest tests/hardware/scenario_manifest.json \
  --runs-dir tests/hardware/runs \
  --reports-dir tests/hardware/reports \
  --scenario WD-01
```

- The script discovers every `telemetry.csv` under `tests/hardware/runs/` that matches the scenario filter.
- Each run generates `tests/hardware/reports/<SCENARIO>/<TIMESTAMP>.md` (report template + scenario metadata, verdicts, and run metadata).
- Threshold verdicts in the final report use PASS/FAIL badges so auditors can read threshold alignment at a glance.
- Reports cite the manifest description, command sequence, golden artifact link, and retention guidance so the evidence trail is obvious.

## Packaging golden artifacts for audits

1. **Identify the golden CSV** produced by the validated run (e.g., `tests/hardware/runs/WD-01/20260320T120000Z/telemetry.csv`).
2. **Mark it as golden** by copying it to the manifest-provided golden path (e.g., `tests/hardware/goldens/WD-01.csv`).
3. **Bundle everything auditors expect** into a tarball named `tests/hardware/goldens/WG-HW03-WD-01-<TIMESTAMP>.tar.gz` containing:
   - `telemetry.csv` and its metadata JSON
   - The markdown report (`tests/hardware/reports/.../.md`)
   - The manifest (`tests/hardware/scenario_manifest.json`) or the trimmed entry for traceability
   - The golden CSV (`tests/hardware/goldens/<SCENARIO>.csv`)
4. **Document retention** inside the bundle by noting the manifest `metadata.retention_days` value; auditors expect this artifact to live at least that long.

## Safety and quality notes

- **Verify firmware has already earned gold status** before you start capturing data. Running against unapproved firmware produces invalid artifacts.
- **Flush logs before copying files**: ensure the runner has closed the CSV file (`Ctrl+C`) before tarballing to avoid partial writes.
- **Avoid manipulating the golden CSV in-place**—always copy the CSV produced by the run to `tests/hardware/goldens/` so reviewers can find the approved file.
- **Retain run metadata for 60+ days** (or the manifest-specified retention) so auditors can replay the evidence if needed.
- **Keep the playbook path in README** (see README section below) so contributors know where to find the latest process.

## Reporting

When you ship a run, link the tarball and the report in your work item, note the `telemetry.csv` run ID, and point to this playbook. Auditors rely on the manifest + report + golden artifact to sign HW-03 off, so keep this workflow atomic and repeatable.
