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

---

## Hardware Component Validation

This section covers hardware component tests (TC-, SSR-, FAN-, GPIO-) which differ from the fault injection scenarios (WD-, GD-, CM-) documented above.

### How Hardware Scenarios Differ

| Attribute | Hardware Scenarios (TC-, SSR-, FAN-, GPIO-) | Fault Injection (WD-, GD-, CM-) |
|-----------|-----------------------------------------------|------------------------------|
| **Purpose** | Validate physical hardware components | Test fault handling and recovery |
| **Safety** | All safe (no heater activation) | May trigger fault conditions |
| **Firmware** | Main firmware (Tier 1) or test firmware (Tier 2) | Main firmware only |
| **Read/Write** | Mix of read-only (TC) and safe writes (SSR/FAN/GPIO) | Primarily fault injection commands |
| **Golden Output** | `tests/hardware/goldens/<ID>.csv` | `tests/hardware/goldens/<ID>.csv` |
| **Runner** | `validation_runner.py --hardware-mode` or `hardware_test_runner.py` | `validation_runner.py` |

Hardware scenarios exercise real peripherals: thermocouples (SPI), SSR (LEDC PWM), fan (LEDC PWM), and GPIO (heat detection pin). All SSR tests use 0% duty cycle only — the heater is never activated.

### Running Hardware Scenarios via validation_runner

Use the `--hardware-mode` flag to tell the runner to execute hardware scenarios instead of fault injection scenarios:

```bash
python tests/hardware/validation_runner.py \
  --port /dev/ttyACM0 \
  --scenario TC-01 \
  --hardware-mode
```

Run all hardware scenarios:

```bash
python tests/hardware/validation_runner.py \
  --port /dev/ttyACM0 \
  --hardware-mode \
  --run-all
```

The runner executes the `command_sequence` from `scenario_manifest.json`, captures `STATUS` telemetry, and writes results to `tests/hardware/runs/<SCENARIO>/<TIMESTAMP>/telemetry.csv`.

### Running Tier 2 Test Firmware via hardware_test_runner

Tier 2 tests use dedicated test firmware from `examples/`. These run at the driver level and validate peripherals directly:

```bash
python tests/hardware/hardware_test_runner.py \
  --port /dev/ttyACM0 \
  --example hil_tc
```

Available test firmware:

| Example | Tests | Safety |
|---------|-------|--------|
| `hil_tc` | 6 tests (SPI bus, raw voltage, registers, conversion) | Read-only |
| `hil_ssr` | 4 tests (LEDC init, zero-duty readback) | SAFE ONLY (no heater) |
| `hil_c1` | 5 tests (DUTY_R latency, config-DUTY sync, ramp verify) | **NON-ZERO duty on GPIO10 — disconnect SSR power** |
| `hil_fan` | 7 tests (LEDC init, duty sweep 0-100%, frequency) | Safe |
| `hil_gpio` | 3 tests (pull-up, readback, consistency) | Safe |

The runner handles building and flashing the test firmware automatically. To manually build and flash:

```bash
# Build test firmware
cargo build --release --target riscv32imc-unknown-none-elf \
  --features embedded --example hil_tc

# Flash test firmware
cargo espflash flash --release --example hil_tc
```

### Safety Checklist for Hardware Tests

Before running any hardware test, verify:

- [ ] ESP32-C3 is connected via USB and visible (`cargo espflash list`)
- [ ] Firmware is flashed (main for Tier 1, test firmware for Tier 2)
- [ ] Heater power is disconnected (recommended for SSR tests)
- [ ] Thermocouples are connected (for TC tests)
- [ ] No metal objects near GPIO pins (prevent shorts)
- [ ] You have proper electrical knowledge if modifying hardware

**Key safety facts:**
- All SSR tests use 0% duty cycle — heater NEVER activates
- Fan tests are safe (no heating element involved)
- Thermocouple tests are read-only
- GPIO tests only read the heat detection pin state

### Interpreting Hardware Test Results

#### Tier 1 (validation_runner output)

After running a scenario, check the output:

```
Scenario: TC-01
Result: PASS
Telemetry: tests/hardware/runs/TC-01/20260501T120000Z/telemetry.csv
Metadata: tests/hardware/runs/TC-01/20260501T120000Z/telemetry.csv.json
```

Open the telemetry CSV to verify:
- `env_temp` and `bean_temp` are within expected ranges
- `fault_condition=0` (no faults)
- `watchdog_flag=1` (watchdog healthy)
- `heater` and `fan` values match commanded values (for SSR/FAN tests)

#### Tier 2 (hardware_test_runner output)

The test runner outputs per-test results:

```
[PASS] hil_tc - Test 1: SPI bus enumeration
[PASS] hil_tc - Test 2: Raw thermocouple voltage (ET)
[PASS] hil_tc - Test 3: Raw thermocouple voltage (BT)
...
Result: 6/6 tests passed
```

Any `[FAIL]` indicates a hardware or firmware issue that needs investigation.

### Promoting Hardware Test Runs to Golden Artifacts

Once a hardware scenario passes and you want to promote it as the golden reference:

1. **Identify the validated run** (e.g., `tests/hardware/runs/TC-01/20260501T120000Z/telemetry.csv`)

2. **Verify the results** meet golden criteria:
   - Stable telemetry (temps within 2°C, outputs within 2%)
   - `watchdog_flag=1`, `failure_count=0`
   - `guard_timeouts < 3`
   - `fault_condition=0` (unless scenario expects otherwise)

3. **Copy to golden path** (from `scenario_manifest.json` `golden_output` field):
   ```bash
   cp tests/hardware/runs/TC-01/20260501T120000Z/telemetry.csv \
      tests/hardware/goldens/TC-01.csv
   ```

4. **Bundle for auditors** (same as fault injection scenarios):
   ```bash
   tar -czf tests/hardware/goldens/WG-HW03-TC-01-20260501T120000Z.tar.gz \
     tests/hardware/runs/TC-01/20260501T120000Z/telemetry.csv \
     tests/hardware/runs/TC-01/20260501T120000Z/telemetry.csv.json \
     tests/hardware/reports/TC-01/20260501T120000Z.md \
     tests/hardware/scenario_manifest.json \
     tests/hardware/goldens/TC-01.csv
   ```

5. **Document retention** — golden artifacts must remain available for 60 days (per manifest `metadata.retention_days`).

### Hardware vs Fault Injection: Quick Reference

| Step | Hardware Scenarios | Fault Injection Scenarios |
|------|---------------------|--------------------------|
| **Manifest** | `scenario_manifest.json` (TC-, SSR-, FAN-, GPIO-) | `scenario_manifest.json` (WD-, GD-, CM-) |
| **Runner** | `validation_runner.py --hardware-mode` | `validation_runner.py` |
| **Tier 2 Runner** | `hardware_test_runner.py --example <name>` | N/A |
| **Golden Path** | `tests/hardware/goldens/<ID>.csv` | `tests/hardware/goldens/<ID>.csv` |
| **Reports** | `tests/hardware/reports/<ID>/` | `tests/hardware/reports/<ID>/` |
| **Analysis** | `analysis.py --scenario TC-01` | `analysis.py --scenario WD-01` |

For the full hardware test plan, see `HARDWARE-TEST-PLAN.md`. For manual PWM verification with an oscilloscope, see `MANUAL-PWM-VERIFICATION.md`. For a quick scenario reference, see `SCENARIO_MATRIX_HW.md`.
