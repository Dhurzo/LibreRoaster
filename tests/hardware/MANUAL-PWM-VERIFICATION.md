# Manual PWM Verification Procedures

Procedures for verifying PWM frequency and duty cycle using measurement equipment (oscilloscope or logic analyzer).

## Equipment Requirements

| Equipment | Purpose | Notes |
|-----------|---------|-------|
| **Oscilloscope** or **Logic Analyzer** | Capture PWM waveforms | Minimum 100kHz bandwidth for fan PWM |
| **Probe clips** | Connect to ESP32-C3 GPIO pins | 10x probe preferred for accuracy |
| **ESP32-C3** | Device under test | Running main firmware |
| **Host machine** | Send serial commands | `cargo espflash monitor` or serial terminal |

## SSR PWM Verification (GPIO10)

The SSR PWM output operates at **1 Hz** (zero-cross compatible) with variable duty cycle (0-100%).

### Setup

1. Connect probe to **GPIO10** (SSR PWM output on ESP32-C3)
2. Set probe to 10x attenuation if available
3. Set oscilloscope to **200 ms/div** timebase (to capture several 1 Hz cycles)
4. Set trigger to rising edge, 1.65V threshold (3.3V logic midpoint)
5. Connect serial terminal to ESP32-C3: `cargo espflash monitor --speed 115200`

### Test Procedure

#### Step 1: Send Command

In the serial terminal, send:
```
OT1 50
```

This sets the heater power to 50%. Note: For safety, this test only verifies the PWM signal shape. The heater power should be disconnected during this test.

#### Step 2: Measure Frequency

Capture the waveform and measure the period (T):

| Measurement | Expected Value | Acceptable Range |
|--------------|----------------|------------------|
| **Frequency** | 1 Hz | 0.95 - 1.05 Hz (±5%) |
| **Period (T)** | 1000 ms | 950 - 1050 ms |

#### Step 3: Measure Duty Cycle

Measure the high time (T_high) vs period:

| Measurement | Expected Value | Acceptable Range |
|--------------|----------------|------------------|
| **Duty Cycle** | 50% | 48% - 52% (±2%) |
| **T_high** | 500 ms | 475 - 525 ms |
| **T_low** | 500 ms | 475 - 525 ms |

#### Step 4: Repeat for Other Duty Cycles

| Command | Expected Duty | Acceptable Range | Pass? |
|---------|----------------|------------------|------|
| `OT1 0` | 0% | 0% (always low) | [ ] |
| `OT1 25` | 25% | 23% - 27% | [ ] |
| `OT1 50` | 50% | 48% - 52% | [ ] |
| `OT1 75` | 75% | 73% - 77% | [ ] |
| `OT1 100` | 100% | 100% (always high) | [ ] |

#### Step 5: Document Results

For each measurement, capture a screenshot or note the values:

```
SSR PWM Verification Results (GPIO10)
Date: _______________
Operator: _______________
Oscilloscope: _______________

| Duty Command | Frequency (Hz) | Duty Cycle (%) | Screenshot File |
|--------------|------------------|-----------------| -----------------|
| OT1 0 | | 0% | |
| OT1 25 | | | |
| OT1 50 | | | |
| OT1 75 | | | |
| OT1 100 | | 100% | |
```

## Fan PWM Verification (GPIO9)

The fan PWM output operates at **25kHz** with variable duty cycle (0-100%). This higher frequency requires an oscilloscope with adequate bandwidth.

### Setup

1. Connect probe to **GPIO9** (Fan PWM output on ESP32-C3)
2. Set probe to 10x attenuation
3. Set oscilloscope to **20µs/div** timebase (to capture ~2-3 cycles at 25kHz)
4. Set trigger to rising edge, 1.65V threshold
5. Connect serial terminal: `cargo espflash monitor --speed 115200`

### Test Procedure

#### Step 1: Send Command

In the serial terminal, send:
```
IO3 50
```

This sets the fan speed to 50%.

#### Step 2: Measure Frequency

| Measurement | Expected Value | Acceptable Range |
|--------------|----------------|------------------|
| **Frequency** | 25 kHz | 23.75 - 26.25 kHz (±5%) |
| **Period (T)** | 40 µs | 38 - 42 µs |

#### Step 3: Measure Duty Cycle

| Measurement | Expected Value | Acceptable Range |
|--------------|----------------|------------------|
| **Duty Cycle** | 50% | 48% - 52% (±2%) |
| **T_high** | 20 µs | 19.2 - 20.8 µs |
| **T_low** | 20 µs | 19.2 - 20.8 µs |

#### Step 4: Repeat for Other Duty Cycles

| Command | Expected Duty | Acceptable Range | Pass? |
|---------|----------------|------------------|------|
| `IO3 0` | 0% | 0% (always low) | [ ] |
| `IO3 25` | 25% | 23% - 27% | [ ] |
| `IO3 50` | 50% | 48% - 52% | [ ] |
| `IO3 75` | 75% | 73% - 77% | [ ] |
| `IO3 100` | 100% | 100% (always high) | [ ] |

#### Step 5: Document Results

```
Fan PWM Verification Results (GPIO9)
Date: _______________
Operator: _______________
Oscilloscope: _______________

| Duty Command | Frequency (kHz) | Duty Cycle (%) | Screenshot File |
|--------------|-------------------|-----------------|------------------|
| IO3 0 | | 0% | |
| IO3 25 | | | |
| IO3 50 | | | |
| IO3 75 | | | |
| IO3 100 | | 100% | |
```

## Signal Quality Checks

Beyond frequency and duty cycle, verify the signal quality:

### Rise and Fall Times

| Parameter | Expected | Acceptable |
|-----------|----------|------------|
| **Rise Time** (10%-90%) | < 50 ns | < 100 ns |
| **Fall Time** (90%-10%) | < 50 ns | < 100 ns |

Fast edges indicate healthy GPIO drive strength.

### Voltage Levels

| Parameter | Expected | Acceptable |
|-----------|----------|------------|
| **High Level** | 3.3V | 2.8V - 3.3V |
| **Low Level** | 0V | 0V - 0.4V |
| **Overshoot** | < 5% | < 10% |

### Noise Levels

| Parameter | Expected | Acceptable |
|-----------|----------|------------|
| **Ripple (when low)** | < 50mV | < 200mV |
| **Ringing (after edges)** | < 5% of Vcc | < 10% of Vcc |

## Results Documentation Template

Use this template to record your measurements:

```markdown
# PWM Verification Report

**Date:** YYYY-MM-DD
**Operator:** Your Name
**Firmware Version:** (from `STATUS` command)
**ESP32-C3 Board:** [e.g., ESP32-C3-DevKitM-1]

## Equipment Used
- Oscilloscope: [model]
- Probes: [model, 10x]
- Serial Terminal: `cargo espflash monitor`

## SSR PWM (GPIO10)
- **Expected Frequency:** 1 Hz (zero-cross)
- **Expected Voltage:** 3.3V logic

| Command | Frequency (Hz) | Duty (%) | Rise (ns) | Fall (ns) | Vhigh (V) | Vlow (V) | Pass? |
|---------|------------------|-----------|------------|------------|------------|-----------|-------|
| OT1 0 | | 0% | | | | | |
| OT1 25 | | | | | | | |
| OT1 50 | | | | | | | |
| OT1 75 | | | | | | | |
| OT1 100 | | 100% | | | | | |

## Fan PWM (GPIO9)
- **Expected Frequency:** 25 kHz
- **Expected Voltage:** 3.3V logic

| Command | Frequency (kHz) | Duty (%) | Rise (ns) | Fall (ns) | Vhigh (V) | Vlow (V) | Pass? |
|---------|-------------------|-----------|------------|------------|------------|-----------|-------|
| IO3 0 | | 0% | | | | | |
| IO3 25 | | | | | | | |
| IO3 50 | | | | | | | |
| IO3 75 | | | | | | | |
| IO3 100 | | 100% | | | | | |

## Screenshots
- [ ] SSR 50% duty cycle captured
- [ ] Fan 50% duty cycle captured
- [ ] Rising edge detail (both signals)

## Notes
[Any observations, anomalies, or issues encountered]

## Verdict
- [ ] SSR PWM operates within specifications
- [ ] Fan PWM operates within specifications
- [ ] Signal quality is acceptable
- [ ] All measurements documented

**Overall Pass/Fail:** [PASS / FAIL]
```

## Safety Notes

- Disconnect heater power before probing SSR PWM (GPIO10)
- Fan tests are safe (no high voltage or heat involved)
- Avoid shorting GPIO pins with probe tips
- Double-check probe connections before powering the ESP32-C3
