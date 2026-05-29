# Hardware Integration Tests

This document catalogs the integration tests that validate **real hardware** on the ESP32-C3 — thermocouples, SSR, fan PWM, and GPIO pins. All tests operate in **safe mode**: heater is never activated.

For the full playbook and manual procedures, see:
- [HIL Playbook](hardware/HIL-PLAYBOOK.md)
- [Hardware Test Plan](hardware/HARDWARE-TEST-PLAN.md)
- [PWM Manual Verification](hardware/MANUAL-PWM-VERIFICATION.md)
- [Scenario Matrix](hardware/SCENARIO_MATRIX_HW.md)

---

## Architecture

Three-tier testing approach:

| Tier | Method | Validates | Equipment |
|------|--------|-----------|-----------|
| **1** | Serial commands to main firmware via USB CDC | Full stack: command → control → hardware → telemetry | ESP32-C3 + USB cable |
| **2** | Dedicated test firmware binaries flashed via `espflash` | Raw peripheral: SPI registers, LEDC duty readback, GPIO state | ESP32-C3 + USB cable |
| **3** | Manual procedures with oscilloscope/logic analyzer | PWM frequency (1 Hz SSR (zero-cross), 25 kHz Fan), signal integrity | Oscilloscope or logic analyzer |

---

## Test Scenarios

### Thermocouple (TC-01..04)

Verifies MAX31856 SPI communication, temperature conversion, and fault detection.

| ID | Description | Tier | Command |
|----|-------------|------|---------|
| TC-01 | READ returns plausible ambient (0-50°C) from ET and BT | 1 | `READ` x3 |
| TC-02 | Both channels report valid, non-identical temperatures | 1 | `STATUS; READ; STATUS` |
| TC-03 | Firmware detects thermocouple fault when disconnected | 1 | `STATUS` → disconnect → `STATUS; READ; STATUS` |
| TC-04 | Readings stable over 10s window (variance < 5°C) | 1 | `READ` x10 at 1s |

Tier 2 firmware: `examples/hil_tc.rs` — validates SPI init, raw conversion, ambient range.

```bash
# Tier 1
python tests/hardware/validation_runner.py --port /dev/ttyACM0 --scenario TC-01 --hardware-mode

# Tier 2
python tests/hardware/hardware_test_runner.py --port /dev/ttyACM0 --example hil_tc
```

### SSR (SSR-01..03) — SAFE MODE ONLY

Verifies LEDC PWM output on GPIO10 and heat detection input on GPIO1. **Heater NEVER activated** — all tests at 0% duty.

| ID | Description | Tier | Command |
|----|-------------|------|---------|
| SSR-01 | OT1 0 sets heater to 0%, STATUS confirms | 1 | `OT1 0; STATUS; STATUS` |
| SSR-02 | SSR initialization — heater stays 0 after boot | 1 | `STATUS; OT1 0; STATUS; STATUS` |
| SSR-03 | Duty set-clear cycle at 0% | 1 | `OT1 0; STATUS; OT1 0; STATUS` |

Tier 2 firmware: `examples/hil_ssr.rs` — validates LEDC init, duty=0 readback, GPIO1 pull-up.

```bash
# Tier 1
python tests/hardware/validation_runner.py --port /dev/ttyACM0 --scenario SSR-01 --hardware-mode

# Tier 2
python tests/hardware/hardware_test_runner.py --port /dev/ttyACM0 --example hil_ssr
```

### Fan (FAN-01..03)

Verifies LEDC PWM output on GPIO9 (25kHz) across full speed range.

| ID | Description | Tier | Command |
|----|-------------|------|---------|
| FAN-01 | IO3 0 sets fan to 0%, STATUS confirms | 1 | `IO3 0; STATUS; STATUS` |
| FAN-02 | IO3 50 sets fan to ~50%, STATUS confirms | 1 | `IO3 0; IO3 50; STATUS; STATUS` |
| FAN-03 | Full 0-100% sweep in 25% steps | 1 | `IO3 0→25→50→75→100→0` with STATUS after each |

Tier 2 firmware: `examples/hil_fan.rs` — validates LEDC init, duty sweep 0→25→50→75→100→0 with register readback verification at each step.

```bash
# Tier 1
python tests/hardware/validation_runner.py --port /dev/ttyACM0 --scenario FAN-01 --hardware-mode

# Tier 2
python tests/hardware/hardware_test_runner.py --port /dev/ttyACM0 --example hil_fan
```

### GPIO (GPIO-01..02)

Verifies GPIO1 input with internal pull-up.

| ID | Description | Tier | Command |
|----|-------------|------|---------|
| GPIO-01 | Heat detection pin reads HIGH when no load (pull-up) | 1 | `OT1 0; STATUS` |
| GPIO-02 | Multiple STATUS queries return consistent data | 1 | `STATUS` x3 |

Tier 2 firmware: `examples/hil_gpio.rs` — validates pull-up state, 10× read consistency, error-free operation.

```bash
# Tier 1
python tests/hardware/validation_runner.py --port /dev/ttyACM0 --scenario GPIO-01 --hardware-mode

# Tier 2
python tests/hardware/hardware_test_runner.py --port /dev/ttyACM0 --example hil_gpio
```

---

## Infrastructure

| Component | Path | Purpose |
|-----------|------|---------|
| Scenario manifest | `tests/hardware/scenario_manifest.json` | All 24 scenarios (12 hardware + 12 existing) |
| Thresholds | `tests/hardware/thresholds.json` | System + hardware pass/fail thresholds |
| Hardware thresholds | `tests/hardware/hardware_thresholds.json` | Hardware-specific threshold values |
| HIL runner | `tests/hardware/validation_runner.py` | Tier 1: serial command execution + CSV capture |
| Analysis | `tests/hardware/analysis.py` | CSV analysis + report generation |
| Hardware runner | `tests/hardware/hardware_test_runner.py` | Tier 2: build → flash → capture → validate |
| Helpers | `tests/hardware/hardware_test_helpers.py` | Serial output parsers, result reporting |
| Test firmware | `examples/hil_tc.rs` | Thermocouple raw hardware validation |
| Test firmware | `examples/hil_ssr.rs` | SSR safe-mode hardware validation |
| Test firmware | `examples/hil_fan.rs` | Fan PWM hardware validation |
| Test firmware | `examples/hil_gpio.rs` | GPIO input hardware validation |

---

## Safety

- **SSR tests NEVER activate the heater.** All duty cycles are at 0%.
- Fan tests are inherently safe (no heat involved).
- Thermocouple tests are read-only.
- Always disconnect heater mains power before running any test if unsure.
- The `--unsafe-allow-heater` flag exists in the infrastructure for future use but should be treated with extreme caution.

---

## Quick Start

```bash
# Prerequisites
pip install pyserial
cargo install espflash

# List available test firmware
python tests/hardware/hardware_test_runner.py --list

# Run Tier 1 (connected hardware required)
python tests/hardware/validation_runner.py --port /dev/ttyACM0 --scenario FAN-01 --hardware-mode

# Run Tier 2 (build + flash + validate)
python tests/hardware/hardware_test_runner.py --port /dev/ttyACM0 --example hil_fan

# Analyze captured runs
python tests/hardware/analysis.py --scenario FAN-01
```
