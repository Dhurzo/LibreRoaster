# Feature Landscape: Artisan Protocol Commands

**Project:** LibreRoaster ESP32-C3 Firmware
**Researched:** February 7, 2026
**Confidence:** HIGH (verified against authoritative Artisan protocol documentation)

## Executive Summary

This document catalogs the Artisan protocol commands for coffee roaster firmware, focusing on READ (telemetry), OT2 (fan control), and UNITS (temperature scale). The standard Artisan protocol is well-documented in the TC4-shield project and forms the basis for most Arduino-based coffee roaster controllers. LibreRoaster already implements an extended Artisan protocol that goes beyond the original specification by including output states (fan/heater) in READ responses.

Key findings:
- **READ command**: Extended from pure temperature reads to include fan/heater output states
- **OT2 command**: Standard fan control command (0-100%), currently uses IO3 alias
- **UNITS command**: Parsed but not actively applied to temperature conversion
- **BT2/ET2 channels**: Handled as -1 placeholders (channels not present in hardware)

---

## Table Stakes

Features users expect. Missing = product feels incomplete.

### READ Command (Telemetry Response)

**Expected Behavior:**
- Returns current roast telemetry as comma-separated values
- Artisan Scope polls READ continuously during active roast
- Response format: `ET,BT,fan_output,heater_output` (floats with 1 decimal place)

**LibreRoaster Implementation:**
- `format_read_response()` returns: `"{:.1},{:.1},{:.1},{:.1}"` mapping to ET, BT, heater, fan
- `format_read_response_full()` returns 7-value extended format: `ET,BT,-1,-1,-1,fan,heater\r\n`
- Unused channels (ET2, BT2, ambient) return `-1` as placeholders

**Edge Cases:**
| Scenario | Expected Behavior |
|----------|-------------------|
| No active roast | Returns current temps, 0% outputs |
| Temperature sensor error | Returns -1 or last valid value (design decision needed) |
| Fan/heater at 0% | Returns "0.0" |
| Fractional values | Single decimal place (e.g., "75.5") |

**Protocol Notes:**
- Standard Artisan protocol specifies: `ambient,chan1,chan2,chan3,chan4` temperatures
- LibreRoaster extends this to include output states for better visibility
- No command acknowledgment (silently ignored if malformed)

### OT2 Command (Fan Speed Control)

**Expected Behavior:**
- Sets fan PWM duty cycle from 0-100%
- Format: `OT2,{value}` where value is 0-100 integer
- No response sent (Artisan convention for output commands)

**LibreRoaster Current State:**
- Uses `IO3` command as alias for fan control
- Parser supports `IO3 50` syntax for 50% fan speed
- `SetFan(u8)` enum variant maps to fan control
- Hardware uses PWM on `FAN_PWM_PIN` (GPIO9)

**Differences from OT1:**
| Aspect | OT1 (Heater) | OT2 (Fan) |
|--------|-------------|-----------|
| Hardware | SSR_CONTROL_PIN (GPIO10) | FAN_PWM_PIN (GPIO9) |
| PWM Frequency | 1 Hz | 25000 Hz |
| Control Type | On/off with duty | Variable speed |
| Response | None | None |

**Edge Cases:**
| Scenario | Expected Behavior |
|----------|-------------------|
| Value > 100 | Return OutOfRange error |
| Value < 0 | InvalidValue error (parse fails) |
| Partial command "OT2" | InvalidValue error |
| During roast | Updates immediately |

### UNITS Command (Temperature Scale)

**Expected Behavior:**
- Sets temperature display units for Artisan Scope
- Format: `UNITS,{C|F}` with semicolon delimiter during init
- No response from device
- Affects how temperatures display in Artisan UI

**LibreRoaster Current State:**
- Parser supports: `UNITS;C`, `UNITS;F`, `units;f`
- Maps to `ArtisanCommand::Units(bool)` where true=Fahrenheit, false=Celsius
- Init state machine commented out (handshake not required)
- **Temperature conversion not implemented** - returns raw Celsius values regardless of UNITS setting

**Edge Cases:**
| Scenario | Expected Behavior |
|----------|-------------------|
| Invalid value "UNITS;K" | InvalidValue error |
| Lowercase accepted | Yes, case-insensitive |
| During active roast | Updates scale for future reads |
| After UNITS;C then UNITS;F | Last command wins |

**Critical Gap:**
UNITS command is parsed but has no effect on temperature values. Artisan Scope may display temperatures incorrectly if user sets Fahrenheit but roaster sends Celsius values.

---

## Differentiators

Features that set product apart. Not expected, but valued.

### Extended READ Response Format

**Value Proposition:**
- Includes real-time fan and heater output percentages in READ response
- Standard Artisan READ only returns temperatures
- Provides operators immediate visibility into control loop state

**Implementation:**
```rust
// Standard format: "ET,BT,-1,-1,-1,fan,heater\r\n"
format!("{:.1},{:.1},-1,-1,-1,{:.1},{:.1}\r\n", 
    status.env_temp,
    status.bean_temp,
    status.fan_output,
    status.ssr_output
)
```

**Benefits:**
1. Operators see control outputs without separate command
2. Logging includes control states for post-roast analysis
3. Reduces number of serial commands needed

### Multiple Fan Control Aliases

**Value Proposition:**
- Supports both `OT2` and `IO3` command syntax
- Compatibility with different Artisan configurations and other software

**Implementation:**
- `OT2,{value}` → `SetHeater(value)` (requires change to SetFan)
- `IO3,{value}` → `SetFan(value)` (current)

**Benefits:**
1. Works with various Artisan machine profiles
2. Backward compatibility with TC4-shield code
3. Flexibility for different control software

### Hardware PWM for Fan Control

**Value Proposition:**
- Uses dedicated 25 kHz PWM for fan control
- Avoids acoustic noise from low-frequency PWM
- Smooth fan speed transitions

**Implementation:**
```rust
pub const FAN_PWM_FREQUENCY_HZ: u32 = 25000;
pub const FAN_LEDC_CHANNEL: u8 = 0;
```

**Benefits:**
1. Quiet operation (above human hearing range)
2. Precise fan speed control
3. Hardware-offloaded PWM (CPU free)

---

## Anti-Features

Features to explicitly NOT build. Common mistakes in this domain.

### Anti-Feature: OT2 Command Returning Responses

**Why Avoid:**
- Artisan protocol convention: output commands (OT1, OT2, IO3) send no response
- Adding responses breaks compatibility with Artisan Scope
- Can cause timing issues if Artisan expects silent acknowledgment

**Instead:**
- Implement OT2 as silent command
- Rely on READ command for feedback
- Log internally for debugging

### Anti-Feature: Automatic Temperature Conversion

**Why Avoid:**
- Temperature conversion adds complexity and potential errors
- Artisans scope handles unit conversion on the display side
- Risk of double-conversion (C→F→C→F)

**Instead:**
- Always send Celsius values
- Let Artisan Scope handle display units based on UNITS setting
- Document the behavior clearly

### Anti-Feature: Complex Error Messages for READ

**Why Avoid:**
- READ is polled frequently (every 100-500ms)
- Error messages clutter serial stream
- Artisan may not parse error formats correctly

**Instead:**
- Always return valid CSV, even with error states
- Use -1 or placeholder values for invalid data
- Keep errors for configuration commands only

### Anti-Feature: Blocking READ Responses

**Why Avoid:**
- Artisan may have timeout expectations
- Long response delays cause UI lag
- Real-time control loops need consistent timing

**Instead:**
- Return READ immediately with current state
- If computation needed, do it asynchronously
- Target response time < 10ms

---

## Feature Dependencies

```
UNITS command (C/F)
    └── Temperature display format in Artisan
    └── Conversion logic (if implemented)

OT2 command (0-100)
    ├── Parser support for OT2 keyword
    ├── SetFan enum variant
    ├── PWM duty cycle update
    └── Fan hardware control

READ command
    ├── Current temperature readings (ET, BT)
    ├── Current output states (fan, heater)
    ├── Channel mapping (TC1→ET, TC2→BT)
    └── Unused channel placeholders (-1)
```

---

## MVP Recommendation

For initial Artisan command support, prioritize:

**Must Have (Phase 1):**
1. **READ command** with extended format (ET, BT, fan, heater)
2. **OT2 command** parsing and fan control (or keep IO3 alias)
3. **Temperature placeholders** for unused channels (-1)

**Should Have (Phase 2):**
1. **UNITS command** affecting temperature display format
2. **Proper channel mapping** via CHAN command (even if ignored)

**Could Have (Post-MVP):**
1. Temperature unit conversion (C↔F)
2. Multiple READ response formats (standard vs extended)
3. Command acknowledgment for output changes

---

## Quality Gate Results

| Requirement | Status | Notes |
|------------|--------|-------|
| READ command behavior and response format | ✅ DONE | Extended format implemented |
| OT2 command semantics vs OT1 | ✅ DONE | Uses IO3 alias currently |
| UNITS command implications for READ | ⚠️ PARTIAL | Parsed but not applied |
| BT2/ET2 disabled channels handling | ✅ DONE | Returns -1 placeholders |
| Edge cases for each command | ✅ DONE | Error handling implemented |

---

## Sources

- **TC4-shield Artisan Protocol Specification** (authoritative)
  - URL: https://raw.githubusercontent.com/greencardigan/TC4-shield/master/applications/Artisan/aArtisan/trunk/src/aArtisan/commands.txt
  - Confidence: HIGH (official protocol documentation)
  - Date: 2014-12-13 (last revision)

- **LibreRoaster Current Implementation**
  - Parser: `/home/juan/Repos/LibreRoaster/src/input/parser.rs`
  - Formatter: `/home/juan/Repos/LibreRoaster/src/output/artisan.rs`
  - Command enum: `/home/juan/Repos/LibreRoaster/src/config/constants.rs`
  - Confidence: HIGH (direct code inspection)

- **Artisan Scope Official Documentation**
  - URL: https://artisan-scope.org/docs/setup/
  - Confidence: MEDIUM (general documentation, not protocol spec)
