# Hardware Scenario Matrix

Quick-reference table of all hardware component test scenarios. These scenarios validate physical hardware behavior through the ESP32-C3 firmware.

## Hardware Test Scenarios

| ID | Category | Tier | Description | Safety | Equipment | Automated? |
|----|-----------|------|-------------|--------|-----------|-------------|
| **TC-01** | Thermocouple | 1 | READ returns plausible ambient temperature from both ET and BT channels | Read-only | ESP32-C3, thermocouples connected | Yes |
| **TC-02** | Thermocouple | 1 | Both thermocouple channels report valid, non-identical temperatures | Read-only | ESP32-C3, two thermocouples | Yes |
| **TC-03** | Thermocouple | 1 | Firmware detects and reports thermocouple fault when sensor disconnected | Read-only | ESP32-C3, ability to disconnect TC | Partial (manual disconnect) |
| **TC-04** | Thermocouple | 1 | Temperature readings stable over 10-second window (no random jumps) | Read-only | ESP32-C3, stable temperature environment | Yes |
| **SSR-01** | SSR | 1 | OT1 0 sets heater to 0%, STATUS confirms heater=0 and LEDC initialized | SAFE | ESP32-C3 | Yes |
| **SSR-02** | SSR | 1 | SSR initialization verification - heater remains 0 after boot | SAFE | ESP32-C3 | Yes |
| **SSR-03** | SSR | 1 | SSR duty cycle set-clear cycle remains at 0 (safe initialization test) | SAFE | ESP32-C3 | Yes |
| **FAN-01** | Fan | 1 | IO3 0 sets fan to 0%, STATUS confirms fan=0 | Safe | ESP32-C3 | Yes |
| **FAN-02** | Fan | 1 | IO3 50 sets fan to ~50%, STATUS confirms | Safe | ESP32-C3 | Yes |
| **FAN-03** | Fan | 1 | Fan responds correctly across full 0-100% range in 25% steps | Safe | ESP32-C3 | Yes |
| **GPIO-01** | GPIO | 1 | Heat detection pin (GPIO1) reads HIGH when no load (internal pull-up active) | Safe | ESP32-C3 | Yes |
| **GPIO-02** | GPIO | 1 | GPIO state consistency - multiple STATUS queries return consistent data | Safe | ESP32-C3 | Yes |

## Tier 2 Test Firmware Scenarios

These scenarios use dedicated test firmware from `examples/` and run via `hardware_test_runner.py`:

| Example Firmware | Category | Tests | Safety | Equipment | Automated? |
|------------------|-----------|-------|--------|-----------|-------------|
| `hil_tc` | Thermocouple | 6 tests (SPI bus, raw voltage, registers, conversion) | Read-only | ESP32-C3, thermocouples | Yes |
| `hil_ssr` | SSR | 4 tests (LEDC init, zero-duty readback) | SAFE ONLY | ESP32-C3 | Yes |
| `hil_fan` | Fan | 7 tests (LEDC init, duty sweep 0-100%, frequency) | Safe | ESP32-C3 | Yes |
| `hil_gpio` | GPIO | 3 tests (pull-up, readback, consistency) | Safe | ESP32-C3 | Yes |

## Safety Summary

| Category | Safety Level | Heater Activated? | Notes |
|-----------|---------------|-------------------|-------|
| **Thermocouple** | Read-only | No | Only temperature readings requested |
| **SSR** | SAFE | Never | All tests use 0% duty cycle only |
| **Fan** | Safe | No | No heating element involved |
| **GPIO** | Safe | No | Input pin testing only |

## Running Hardware Scenarios

### Tier 1 (Serial Integration)

```bash
# Run a specific scenario
python tests/hardware/validation_runner.py \
  --port /dev/ttyACM0 \
  --scenario TC-01 \
  --hardware-mode

# Run all hardware scenarios
python tests/hardware/validation_runner.py \
  --port /dev/ttyACM0 \
  --hardware-mode \
  --run-all
```

### Tier 2 (Test Firmware)

```bash
# Run thermocouple test firmware
python tests/hardware/hardware_test_runner.py \
  --port /dev/ttyACM0 \
  --example hil_tc

# Run SSR test firmware (SAFE - no heater activation)
python tests/hardware/hardware_test_runner.py \
  --port /dev/ttyACM0 \
  --example hil_ssr

# Run fan test firmware
python tests/hardware/hardware_test_runner.py \
  --port /dev/ttyACM0 \
  --example hil_fan

# Run GPIO test firmware
python tests/hardware/hardware_test_runner.py \
  --port /dev/ttyACM0 \
  --example hil_gpio
```

## Scenario Details from Manifest

These scenarios are defined in `tests/hardware/scenario_manifest.json` with the following key attributes:

- `id`: Scenario identifier (TC-01, SSR-01, etc.)
- `category`: Hardware component being tested
- `tier`: Test tier (1 = serial integration, 2 = test firmware)
- `description`: Plain-text description of the test
- `command_sequence`: Semicolon-separated commands the automation executes
- `requires_unsafe`: Always `false` for hardware scenarios (all are safe)
- `expected_columns`: STATUS/READ columns validated during the test
- `golden_output`: Path to approved golden CSV for the scenario
- `metadata`: Retention, owner, and scenario-specific expectations

## Comparison with Fault Injection Scenarios

| Attribute | Hardware Scenarios (TC-, SSR-, FAN-, GPIO-) | Fault Injection Scenarios (WD-, GD-, CM-) |
|-----------|-----------------------------------------------|------------------------------------------|
| **Purpose** | Validate hardware components | Test fault handling and recovery |
| **Tier** | Tier 1 (serial) + Tier 2 (firmware) | Tier 1 (serial) |
| **Safety** | All safe (no heater activation) | May trigger fault conditions |
| **Firmware** | Main firmware (Tier 1) or test firmware (Tier 2) | Main firmware only |
| **Golden Output** | `tests/hardware/goldens/<ID>.csv` | `tests/hardware/goldens/<ID>.csv` |
| **Runner** | `validation_runner.py --hardware-mode` or `hardware_test_runner.py` | `validation_runner.py` |
