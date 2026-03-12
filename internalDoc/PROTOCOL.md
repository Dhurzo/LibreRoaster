# Artisan Protocol Specification

**Last Updated:** 2026-03-11 (v5.1 documentation update)  
**Version:** v5.1  
**Purpose:** Complete Artisan protocol specification for LibreRoaster ESP32-C3 firmware.

---

## Overview

LibreRoaster implements Artisan+ protocol compatibility for coffee roasting control. This protocol enables temperature telemetry and heater/fan control via serial communication channels.

**Communication Channels:**
- USB CDC (Virtual COM port)
- UART0 serial interface

**Protocol Model:** Command/response pattern where Artisan sends commands and roaster responds with telemetry or acknowledgment.

---

## Commands

Commands are organized by workflow: **Setup** (configuration), **Control** ( roast operation), and **Monitoring** (telemetry).

### Setup Commands

#### UNITS - Temperature Scale Preference

**Purpose:** Set temperature display preference (C or F)

**Syntax:** `UNITS<C|F>`

**Parameters:**

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| C | Char | - | Celsius preference |
| F | Char | - | Fahrenheit preference |

**Example:** `UNITSF` (set to Fahrenheit)

**Response:** `OK` or `ERR`

**Important Implementation Note:** UNITS is parse-only. The preference is stored but NO temperature conversion is applied. All internal temperatures remain in Celsius regardless of UNITS setting. The UNITS command uses the `ManualCommandPolicy` trait pattern via `forward_artisan_manual_command()` for centralized manual command handling. Refer to ARCHITECTURE.md for implementation details.

---

### Control Commands

#### OT1 - Heater Control

**Purpose:** Set heater output percentage (integer 0-100)

**Syntax:** `OT1<value>`

**Parameters:**

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| value | Integer | 0-100 | Heater PWM percentage |

**Example:** `OT175` (sets heater to 75%)

**Response:** `OK` or `ERR`

---

#### IO3 - Fan Control

**Purpose:** Set fan output percentage (integer 0-100)

**Syntax:** `IO3<value>`

**Parameters:**

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| value | Integer | 0-100 | Fan PWM percentage |

**Example:** `IO350` (sets fan to 50%)

**Response:** `OK` or `ERR`

---

#### OT2 - Fan Control (Decimal)

**Purpose:** Set fan output percentage (decimal, 0-100) with rounding and clamping

**Syntax:** `OT2<value>`

**Parameters:**

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| value | Decimal | 0-100 | Fan PWM percentage (supports decimals) |

**Example:** `OT275.5` (rounds to 76%, clamped to 0-100 range)

**Behavior:**
- Decimal values are rounded to nearest integer
- Values <0 clamped to 0
- Values >100 clamped to 100

**Safety Note:** When fan value is clamped, heater is automatically stopped to prevent overheating. Refer to ARCHITECTURE.md for safety implementation.

**Response:** `OK` or `ERR`

**OT2 Flow Diagram:**

```
Command Received (OT275.5)
         │
         ▼
    ┌─────────┐
    │ Parser  │ ← Rounding: 75.5 → 76
    └────┬────┘
         │
         ▼
    ┌─────────┐
    │ Handler │ ← Clamping: check 0-100 range
    └────┬────┘
         │
         ▼
  ┌──────────────┐
  │ Fan Control  │ ← Update fan PWM
  └──────┬───────┘
         │
         ▼
    Response: OK
```

---

#### UP - Heater Increment

**Purpose:** Increment heater output by 5%, clamped to 0-100

**Syntax:** `UP`

**Example:** `UP`

**Response:** `OK` or `ERR`

---

#### DOWN - Heater Decrement

**Purpose:** Decrement heater output by 5%, clamped to 0-100

**Syntax:** `DOWN`

**Example:** `DOWN`

**Response:** `OK` or `ERR`

---

#### START - Start Roast

**Purpose:** Begin roast session, initialize control loops

**Syntax:** `START`

**Example:** `START`

**Response:** `OK` or `ERR`

---

#### STOP - Stop Roast

**Purpose:** Stop roast session, disable outputs

**Syntax:** `STOP`

**Example:** `STOP`

**Response:** `OK` or `ERR`

---

### Monitoring Commands

#### READ - Telemetry Response

**Purpose:** Request current temperature and status telemetry

**Syntax:** `READ`

**Response Format:** 4-value CSV: `ET,BT,HEATER,FAN`

**Field Details:**

| Field | Type | Unit | Description |
|-------|------|------|-------------|
| ET | Decimal | °C | Exhaust Temperature |
| BT | Decimal | °C | Bean Temperature |
| HEATER | Decimal | % | Heater PWM percentage |
| FAN | Decimal | % | Fan PWM percentage |

**Example Command:** `READ`

**Example Response:** `185.3,201.4,45,80`

**Note:** This is the 4-value CSV format (ET,BT,HEATER,FAN), corrected from the legacy 7-value specification.

**Precision:** All values have one decimal place.

**Unused Channels:** ET2 and BT2 (second thermocouple channels) are not supported and return -1 as placeholder values.

#### STATUS/STAT - Automation Telemetry Snapshot

**Purpose:** Request comprehensive telemetry including safety metrics and watchdog status

**Syntax:** `STATUS` or `STAT` (alias)

**Response Format:** 18-value CSV: `ET,BT,Heater,Fan,WatchdogOK,WatchdogFailures,LastWatchdogReason,LEDCGuardTimeouts,RegressionActive,PV,MV,IntegratorValue,DerivativeValue,SaturationFlag,IntegratorClampFlag,DerivativeAvailableFlag,CommandLatency,MaxCommandLatency`

**Key Fields:**
- **ET, BT, Heater, Fan:** Same as READ response for backward compatibility
- **WatchdogOK (0/1):** Watchdog feed success status
- **WatchdogFailures:** Consecutive watchdog failure count
- **LastWatchdogReason:** Failure reason token (or "none")
- **LEDCGuardTimeouts:** LEDC guard timeout counter
- **RegressionActive (0/1):** Over‑temperature regression test active flag
- **PV, MV, IntegratorValue, DerivativeValue:** PID controller internal state
- **SaturationFlag, IntegratorClampFlag, DerivativeAvailableFlag:** PID saturation flags
- **CommandLatency, MaxCommandLatency:** Command processing latency in microseconds

**Example Command:** `STATUS`

**Example Response:** `120.3,150.5,75.0,50.0,1,0,none,0,0,150.5,88.5,37.1,-0.42,1,1,1,1250,5000`

**Note:** For complete field definitions and instrumentation usage, see [INSTRUMENTATION_README.MD](INSTRUMENTATION_README.MD).

#### REG - Over‑Temperature Regression Trigger

**Purpose:** Trigger over‑temperature regression testing sequence (safety validation)

**Syntax:** `REG`

**Behavior:**
- Ramps heater and fan to 100% for regression testing
- Keeps watchdog fed during the sequence
- Emits `SAFETY OT-REGRESSION` log records for automation monitoring
- Returns `OK` when regression sequence starts

**Safety Notes:**
- Regression sequences should only be triggered in controlled test environments
- System monitors temperature and will emergency‑stop if limits exceeded
- Use `STATUS` command to monitor `RegressionActive` flag

**Example Command:** `REG`

**Response:** `OK` (when regression sequence starts)

---

## Response Formats

### READ Response Format

4-value CSV: `ET,BT,HEATER,FAN`

- **ET:** Exhaust Temperature (°C, one decimal)
- **BT:** Bean Temperature (°C, one decimal)
- **HEATER:** Heater PWM percentage (0-100)
- **FAN:** Fan PWM percentage (0-100)

**Example:** `185.3,201.4,45,80`

### Error Response Format

All errors returned in ERR format:

**Syntax:** `ERR<error_code>`

**Error Codes:**

| Code | Description |
|------|-------------|
| ERR1 | Invalid command |
| ERR2 | Invalid parameter value |
| ERR3 | Command not allowed in current state |
| ERR4 | Communication timeout |

**Example:** `ERR2` (invalid parameter value)

---

## Behavior Specifications

### OT2 Rounding and Clamping

**Rounding:**
- Values ≥0.5 round up (75.5 → 76)
- Values <0.5 round down (75.4 → 75)

**Clamping:**
- Values <0 → 0 (heater stopped as safety measure)
- Values >100 → 100

### UNITS Parse-Only Behavior

The UNITS command stores the temperature scale preference but does NOT apply any temperature conversion:

- Preference is stored for Artisan display purposes
- All internal temperatures remain in Celsius
- READ responses always return Celsius values
- Conversion must be performed by Artisan if needed

**Implementation Reference:** See `constants.rs:119-142`

### Placeholder Values for Unused Channels

ET2 and BT2 (second thermocouple channels) are not supported:

- Returns `-1` for ET2 position
- Returns `-1` for BT2 position
- These are placeholder values, not actual measurements

---

## Quick Reference

| Command | Syntax | Parameters | Description |
|---------|--------|------------|-------------|
| READ | READ | None | Get telemetry (ET,BT,HEATER,FAN) |
| STATUS | STATUS or STAT | None | Automation telemetry (18 fields, includes safety metrics) |
| REG | REG | None | Over‑temperature regression trigger (safety testing) |
| START | START | None | Begin roast session |
| STOP | STOP | None | Stop roast session |
| OT1 | OT1<value> | 0-100 | Set heater percentage |
| IO3 | IO3<value> | 0-100 | Set fan percentage |
| OT2 | OT2<value> | 0-100 (decimal) | Set fan percentage (decimal) |
| UP | UP | None | Increment heater +5% |
| DOWN | DOWN | None | Decrement heater -5% |
| UNITS | UNITS<C\|F> | C or F | Set temperature scale |

**Important Notes:**
- READ returns 4-value CSV (ET,BT,HEATER,FAN), NOT 7-value legacy format
- UNITS is parse-only, no temperature conversion applied
- OT2 includes decimal rounding and clamping (safety: heater stops if clamped)

---

## References

- **Code References:**
  - READ format: `artisan.rs:109-121`
  - OT2 parsing: `parser.rs:116-132`
  - UNITS implementation: `constants.rs:119-142`
  - OT2 safety: `roaster_refactored.rs:521-528`

- **Related Documentation:**
  - ARCHITECTURE.md (internal implementation details)
  - HARDWARE.md (hardware specifications)
