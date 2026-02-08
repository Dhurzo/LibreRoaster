# Artisan Protocol Specification

**Last Updated:** 2026-02-07 (v2.2)  
**Version:** v2.2  
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

**Important Implementation Note:** UNITS is parse-only. The preference is stored but NO temperature conversion is applied. All internal temperatures remain in Celsius regardless of UNITS setting. Refer to ARCHITECTURE.md for implementation details.

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

**Purpose:** Increment heater output by 1%, clamped to 0-100

**Syntax:** `UP`

**Example:** `UP`

**Response:** `OK` or `ERR`

---

#### DOWN - Heater Decrement

**Purpose:** Decrement heater output by 1%, clamped to 0-100

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

**Implementation Reference:** See `roaster_refactored.rs:426-434`

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
| START | START | None | Begin roast session |
| STOP | STOP | None | Stop roast session |
| OT1 | OT1<value> | 0-100 | Set heater percentage |
| IO3 | IO3<value> | 0-100 | Set fan percentage |
| OT2 | OT2<value> | 0-100 (decimal) | Set fan percentage (decimal) |
| UP | UP | None | Increment heater +1% |
| DOWN | DOWN | None | Decrement heater -1% |
| UNITS | UNITS<C\|F> | C or F | Set temperature scale |

**Important Notes:**
- READ returns 4-value CSV (ET,BT,HEATER,FAN), NOT 7-value legacy format
- UNITS is parse-only, no temperature conversion applied
- OT2 includes decimal rounding and clamping (safety: heater stops if clamped)

---

## References

- **Code References:**
  - READ format: `artisan.rs:111-119`
  - OT2 parsing: `parser.rs:115-131`
  - UNITS implementation: `roaster_refactored.rs:426-434`
  - OT2 safety: `roaster_refactored.rs:374-385`

- **Related Documentation:**
  - ARCHITECTURE.md (internal implementation details)
  - hardware.md (hardware specifications)
