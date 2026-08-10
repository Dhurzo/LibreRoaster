# Hardware Component Test Plan

Master test plan for validating LibreRoaster hardware components on ESP32-C3.

## Overview

LibreRoaster uses a three-tier testing approach to validate hardware components safely and thoroughly:

| Tier | Approach | Firmware | Safety |
|------|----------|---------|--------|
| **Tier 1** | Serial integration tests via main firmware | Main (default) | All safe (no heater activation) |
| **Tier 2** | Dedicated test firmware via `examples/` | Test-specific | SAFE ONLY (no heater) |
| **Tier 3** | Manual procedures with measurement equipment | Main | Depends on test |

Tier 1 exercises hardware through the production firmware using Artisan protocol commands. Tier 2 flashes purpose-built test firmware that directly validates peripherals at the driver level. Tier 3 requires oscilloscopes or logic analyzers for signal quality verification.

## Prerequisites

Before running any hardware test, ensure you have:

1. **ESP32-C3 board** connected via USB
2. **Firmware built** (main for Tier 1/3, test firmware for Tier 2)
3. **Python 3.8+** installed on your host machine
4. **pyserial** installed (`pip install pyserial`)
5. **espflash** installed (`cargo install espflash`)

Verify the device is visible:

```bash
cargo espflash list
```

You should see a device like `/dev/ttyACM0` (Linux), `/dev/cu.usbmodem-*` (macOS), or `COM3` (Windows).

## Tier 1 Tests (Serial Integration)

These tests run against the main firmware and exercise hardware through Artisan protocol commands. All SSR tests are safe (heater never activated).

### Running Tier 1 Tests

```bash
python tests/hardware/validation_runner.py \
  --port /dev/ttyACM0 \
  --scenario TC-01 \
  --hardware-mode
```

The `--hardware-mode` flag tells the runner to execute hardware scenarios (TC-, SSR-, FAN-, GPIO-) instead of fault-injection scenarios (WD-, GD-, CM-).

### Thermocouple Tests (TC-01 through TC-04)

| ID | Description | Safety | Equipment |
|----|-------------|--------|-----------|
| **TC-01** | READ returns plausible ambient temperature from both ET and BT channels | Read-only | ESP32-C3, thermocouples connected |
| **TC-02** | Both thermocouple channels report valid, non-identical temperatures | Read-only | ESP32-C3, two thermocouples |
| **TC-03** | Firmware detects and reports thermocouple fault when sensor disconnected | Read-only | ESP32-C3, ability to disconnect TC |
| **TC-04** | Temperature readings stable over 10-second window (no random jumps) | Read-only | ESP32-C3, stable temperature environment |

Expected behavior:
- TC-01: `READ` returns `AMB,ET,BT,0.0,0.0` with temperatures between 0-50°C for ambient
- TC-02: ET and BT differ by less than 15°C (they measure different locations)
- TC-03: `STATUS` shows `fault_condition=1` when TC disconnected, clears when reconnected
- TC-04: 10 consecutive reads show temperature delta less than 5°C

### SSR Tests (SSR-01 through SSR-03)

All SSR tests are **SAFE** — the heater is never activated.

| ID | Description | Safety | Equipment |
|----|-------------|--------|-----------|
| **SSR-01** | OT1 0 sets heater to 0%, STATUS confirms heater=0 and LEDC initialized | SAFE | ESP32-C3 |
| **SSR-02** | SSR initialization verification - heater remains 0 after boot | SAFE | ESP32-C3 |
| **SSR-03** | SSR duty cycle set-clear cycle remains at 0 (safe initialization test) | SAFE | ESP32-C3 |

Expected behavior:
- SSR-01: `OT1 0` then `STATUS` shows `heater=0.0`
- SSR-02: After boot, `STATUS` shows `heater=0.0` without sending commands
- SSR-03: Multiple `OT1 0` commands, `STATUS` always shows `heater=0.0`

### Fan Tests (FAN-01 through FAN-03)

Fan tests are inherently safe — no heating element is involved.

| ID | Description | Safety | Equipment |
|----|-------------|--------|-----------|
| **FAN-01** | IO3 0 sets fan to 0%, STATUS confirms fan=0 | Safe | ESP32-C3 |
| **FAN-02** | IO3 50 sets fan to ~50%, STATUS confirms | Safe | ESP32-C3 |
| **FAN-03** | Fan responds correctly across full 0-100% range in 25% steps | Safe | ESP32-C3 |

Expected behavior:
- FAN-01: `IO3 0` then `STATUS` shows `fan=0.0`
- FAN-02: `IO3 50` then `STATUS` shows `fan=50.0` (±5% tolerance)
- FAN-03: Sweep `IO3 0`, `25`, `50`, `75`, `100`, `0` — each `STATUS` matches commanded value within tolerance

### GPIO Tests (GPIO-01 through GPIO-02)

| ID | Description | Safety | Equipment |
|----|-------------|--------|-----------|
| **GPIO-01** | Heat detection pin (GPIO1) reads HIGH when no load (internal pull-up active) | Safe | ESP32-C3 |
| **GPIO-02** | GPIO state consistency - multiple STATUS queries return consistent data | Safe | ESP32-C3 |

Expected behavior:
- GPIO-01: `OT1 0` then `STATUS` — heat detection reflects SSR off state
- GPIO-02: Three consecutive `STATUS` commands return consistent `heater`, `fan`, `guard_timeouts` values

### Complete Scenario Table

| ID | Category | Description | Safety Level | Equipment |
|----|-----------|-------------|-------------|-----------|
| TC-01 | Thermocouple | READ returns plausible ambient temperature | Read-only | ESP32-C3, thermocouples |
| TC-02 | Thermocouple | Dual-channel valid, non-identical temps | Read-only | ESP32-C3, two thermocouples |
| TC-03 | Thermocouple | Fault detection on sensor disconnect | Read-only | ESP32-C3, disconnectable TC |
| TC-04 | Thermocouple | Temperature stability over 10s window | Read-only | ESP32-C3, stable environment |
| SSR-01 | SSR | OT1 0 sets heater to 0%, LEDC init | SAFE | ESP32-C3 |
| SSR-02 | SSR | SSR initialization, heater 0 after boot | SAFE | ESP32-C3 |
| SSR-03 | SSR | Duty cycle set-clear remains at 0 | SAFE | ESP32-C3 |
| FAN-01 | Fan | IO3 0 sets fan to 0% | Safe | ESP32-C3 |
| FAN-02 | Fan | IO3 50 sets fan to ~50% | Safe | ESP32-C3 |
| FAN-03 | Fan | Full 0-100% range in 25% steps | Safe | ESP32-C3 |
| GPIO-01 | GPIO | Heat detection pin reads HIGH, pull-up active | Safe | ESP32-C3 |
| GPIO-02 | GPIO | GPIO state consistency across queries | Safe | ESP32-C3 |

## Tier 2 Tests (Test Firmware)

Tier 2 uses dedicated test firmware from the `examples/` directory. These tests run directly on the hardware with firmware built specifically for testing.

### Running Tier 2 Tests

```bash
python tests/hardware/hardware_test_runner.py \
  --port /dev/ttyACM0 \
  --example hil_tc
```

The `hardware_test_runner.py` flashes the specified example firmware, then runs the test sequence and reports results.

### Building and Flashing Test Firmware

Before running Tier 2 tests, build the test firmware:

```bash
# Build test firmware (example: hil_tc)
cargo build --release --target riscv32imc-unknown-none-elf \
  --features embedded --example hil_tc

# Flash test firmware
cargo espflash flash --release --example hil_tc
```

Or let the runner handle it:

```bash
python tests/hardware/hardware_test_runner.py \
  --port /dev/ttyACM0 \
  --example hil_tc \
  --auto-flash
```

### Test Firmware Suites

#### `hil_tc` — Thermocouple Raw SPI Validation (6 tests)

Validates the MAX31856 SPI communication and temperature conversion at the driver level.

| Test | Description | Expected Result |
|------|-------------|-----------------|
| 1 | SPI bus enumeration (both CS pins respond) | Both GPIO3 and GPIO4 respond |
| 2 | Raw thermocouple voltage read (ET channel) | Plausible voltage value |
| 3 | Raw thermocouple voltage read (BT channel) | Plausible voltage value |
| 4 | MAX31856 register readback (config register) | Matches expected config |
| 5 | Temperature conversion (ET channel) | Celsius value, reasonable range |
| 6 | Temperature conversion (BT channel) | Celsius value, reasonable range |

#### `hil_ssr` — SSR LEDC Initialization and Readback (4 tests) — SAFE ONLY

Validates LEDC PWM peripheral configuration for heater control. **Heater is NEVER activated.**

| Test | Description | Expected Result |
|------|-------------|-----------------|
| 1 | LEDC channel init (GPIO10, 5 Hz zero-cross) | Channel configured |
| 2 | Duty cycle set to 0%, readback | Duty = 0 |
| 3 | LEDC timer config verification | 5 Hz frequency confirmed |
| 4 | Multiple init calls (idempotency) | No error, consistent state |

#### `hil_c1` — DUTY_R Latency / Write-Verification (5 tests) — **DRIVES GPIO10 NON-ZERO**

Bench measurement closing audit finding C1 (2026-08-10): verifies the register
semantics the fix relies on, at production SSR configuration (Timer0, 14-bit, 5 Hz,
Channel1, GPIO10). **⚠️ Disconnect SSR/load power before running** — the channel
carries non-zero duty; it is left at 0 % on completion.

| Test | Description | Expected Result |
|------|-------------|-----------------|
| 1 | Init LEDC exactly like production (14-bit, 5 Hz) | Channel configured |
| 2 | Config DUTY readback at +1 ms after a 50 % write | Matches commanded ticks (synchronous) |
| 3 | DUTY_R at +1 ms / +250 ms after a 30 % write | Converges at +250 ms; +1 ms may lag (reported) |
| 4 | Ramp 1 % → 60 % verified via config-DUTY readback | Both steps OK (pre-fix path failed > 0.8 %) |
| 5 | Safe shutdown: 0 % write, config + DUTY_R converged | Channel left at 0 % |

#### `hil_fan` — Fan LEDC Duty Sweep and Readback (7 tests)

Validates LEDC PWM peripheral for fan control across the full range.

| Test | Description | Expected Result |
|------|-------------|-----------------|
| 1 | LEDC channel init (GPIO9, 25kHz) | Channel configured |
| 2 | Duty cycle 0% set and readback | Duty = 0 |
| 3 | Duty cycle 25% set and readback | Duty ≈ 25% |
| 4 | Duty cycle 50% set and readback | Duty ≈ 50% |
| 5 | Duty cycle 75% set and readback | Duty ≈ 75% |
| 6 | Duty cycle 100% set and readback | Duty = 100% |
| 7 | Frequency verification (25kHz ±5%) | 23.75-26.25 kHz |

#### `hil_gpio` — GPIO1 Pull-up and Consistency (3 tests)

Validates the heat detection input pin configuration.

| Test | Description | Expected Result |
|------|-------------|-----------------|
| 1 | GPIO1 pull-up enabled verification | Pull-up active |
| 2 | GPIO1 reads HIGH with no load | Logic high |
| 3 | Multiple reads return consistent state | Stable reading |

## Safety Warnings

### SSR Safe Mode

All hardware tests are designed to be safe:
- **Heater is NEVER activated** in any test scenario
- SSR tests only verify LEDC initialization and zero-duty state
- The `OT1` command in Tier 1 tests is always called with `0` (0% duty)

### Fan Tests

Fan tests are inherently safe:
- No heating element is involved
- Fan PWM operates at 25kHz with variable duty
- No thermal risk during fan testing

### Thermocouple Tests

TC tests are read-only:
- No actuators are commanded
- Only temperature readings are requested via `READ` or `STATUS`
- Safe to run with or without thermocouples connected (fault detection is tested)

### General Precaution

Always disconnect heater power before running tests if you are unsure about the firmware state. While all tests are designed to be safe, disconnecting the heater power eliminates any risk of unintended activation.

## Troubleshooting

### No Serial Output

**Symptom:** `validation_runner.py` or `hardware_test_runner.py` hangs with no output.

**Fixes:**
1. Verify the port is correct: `cargo espflash list`
2. Check the ESP32-C3 is booted: `cargo espflash monitor --speed 115200`
3. Ensure firmware is flashed: `cargo espflash flash --release`

### Wrong Baud Rate

**Symptom:** Garbled serial output or `pyserial` timeout errors.

**Fix:** LibreRoaster uses 115200 baud. Ensure your tool specifies this:
```bash
python tests/hardware/validation_runner.py --port /dev/ttyACM0 --baud 115200 --scenario TC-01 --hardware-mode
```

### Firmware Not Flashed

**Symptom:** Tests fail immediately or device doesn't respond to commands.

**Fix:** Flash the firmware before testing:
```bash
cargo espflash flash --release
cargo espflash monitor  # Verify boot messages
```

### Permission Denied on Serial Port (Linux)

**Symptom:** `PermissionError` when accessing `/dev/ttyACM0`.

**Fix:**
```bash
sudo usermod -a -G dialout $USER
# Log out and back in, or:
sudo chmod 666 /dev/ttyACM0  # Temporary fix
```

### Thermocouple Read Errors

**Symptom:** `STATUS` shows fault or implausible temperatures.

**Fixes:**
1. Check thermocouple wiring (Type-K, connected to MAX31856)
2. Verify MAX31856 CS pins: GPIO3 (ET), GPIO4 (BT)
3. Ensure SPI connections are solid (MOSI/MISO/SCLK shared bus)

## Next Steps

After running hardware tests:
1. Review the output for PASS/FAIL status
2. For Tier 1: Check `tests/hardware/runs/<SCENARIO>/` for telemetry CSVs
3. For Tier 2: Check the test runner output for per-test results
4. If all tests pass, the hardware is validated and ready for roast sessions
5. If tests fail, consult the troubleshooting section or check `docs/HARDWARE.md`
