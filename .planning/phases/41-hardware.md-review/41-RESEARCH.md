# RESEARCH: Phase 41 - hardware.md Review

**Phase:** 41 - hardware.md Review
**Research Date:** 2026-02-08
**Mode:** Documentation verification

---

## Summary

Phase 41 requires verifying hardware.md accuracy against v2.2 implementation. Key findings:

1. **Pin Assignments:** ✓ Consistent between hardware.md and source code (no v2.2 changes)
2. **OT2 Fan Control:** ⚠️ Hardware implications documented but READ format is wrong (7-value vs 4-value)
3. **Thermocouple Configuration:** ✓ Correct (BT/ET only, no ET2/BT2)

**Critical Issue:** hardware.md documents 7-value READ format but v2.2 uses 4-value (ET,BT,HEATER,FAN)

---

## Detailed Findings

### HW-01: Verify Pin Assignments

**Status:** ✓ ACCURATE

**Comparison Table:**

| Pin | hardware.md | constants.rs | Status |
|-----|-------------|--------------|--------|
| GPIO3 (ET CS) | ✓ | THERMOCOUPLE_ET_CS_PIN: 3 | ✓ Match |
| GPIO4 (BT CS) | ✓ | THERMOCOUPLE_BT_CS_PIN: 4 | ✓ Match |
| GPIO5 (MOSI) | ✓ | SPI_MOSI_PIN: 5 | ✓ Match |
| GPIO6 (MISO) | ✓ | SPI_MISO_PIN: 6 | ✓ Match |
| GPIO7 (SCLK) | ✓ | SPI_SCLK_PIN: 7 | ✓ Match |
| GPIO9 (Fan PWM) | ✓ | FAN_PWM_PIN: 9 | ✓ Match |
| GPIO10 (SSR) | ✓ | SSR_CONTROL_PIN: 10 | ✓ Match |
| GPIO1 (Heat Detect) | ✓ | HEAT_DETECTION_PIN: 1 | ✓ Match |
| GPIO20 (UART TX) | ✓ | UART_TX_PIN: 20 | ✓ Match |
| GPIO21 (UART RX) | ✓ | UART_RX_PIN: 21 | ✓ Match |

**PWM Frequencies:**
- Fan: 25kHz (correct in both)
- SSR: 1Hz (correct in both)

**No v2.2 pin assignment changes detected.** Pin configuration is identical to what was documented.

---

### HW-02: OT2 Fan Control Hardware Implications

**Status:** ⚠️ PARTIALLY ACCURATE

**What's Correct:**
- Fan PWM pin (GPIO9) ✓
- PWM frequency (25kHz) ✓
- MOSFET driver circuit ✓
- SSR pin (GPIO10) ✓

**What's Incorrect:**
- READ format section documents 7-value response but v2.2 uses 4-value
- Line 365: "Retorna: `ET,BT,Power,Fan`" but line 358 shows 7-value format
- Line 358: "`time,ET,BT,ROR,Gas`" - this format is from pre-v2.2

**OT2 vs IO3:**
- hardware.md correctly documents IO3 fan control (line 368)
- OT2 command is NOT documented in hardware.md
- v2.2 added OT2 fan speed command but this doesn't change hardware (same GPIO9, same PWM)

**Hardware Implications of OT2:**
- No new hardware required
- Same GPIO9 PWM output used
- Same MOSFET driver circuit
- OT2 is a software protocol change for fan speed control with decimals

---

### HW-03: Thermocouple Configuration

**Status:** ✓ ACCURATE

**Evidence:**
- MAX31856 x2 for BT and ET (lines 16-17)
- Type-K thermocouples (line 18)
- GPIO4 for BT CS (line 47), GPIO3 for ET CS (line 48)
- No references to ET2/BT2 anywhere in source code
- src/config/constants.rs only defines BT and ET

**v2.2 Thermocouple Behavior:**
- Only BT and ET thermocouples used
- No second set (ET2/BT2) implemented
- READ response uses only BT and ET values

---

## Documentation Updates Needed

### Critical Updates Required:

1. **READ Response Format (Line ~358-365):**
   - Current: "ET,BT,Power,Fan" caption but 7-value format
   - Should be: 4-value format: `ET,BT,HEATER,FAN`
   - Section "Comandos ARTISAN+ Soportados" needs update

2. **OT2 Command Documentation:**
   - Add OT2 command to supported commands table
   - Document that OT2 controls fan speed (not heater)
   - Clarify OT2 vs IO3: OT2 has decimals, IO3 does not

3. **Response Format Clarification:**
   - Line 365: "Retorna: `ET,BT,Power,Fan`" 
   - This is actually correct (4 values)
   - But diagram shows 7-value - update diagram to match

### Minor Updates:

1. **Line 358 format string:**
   - Current: `time,ET,BT,ROR,Gas`
   - v2.2 uses: `ET,BT,HEATER,FAN` (4 values, no ROR, no Gas)
   - Note: ROR calculated by Artisan, Gas is heater output

2. **Last Updated timestamp:**
   - Current: No timestamp visible
   - Should add: "Last Updated: 2026-02-08 (v2.3)"

---

## Verification Approach

### 1. Pin Assignment Verification

```bash
# Verify pins match between hardware.md and constants.rs
grep -E "GPIO[0-9]+" internalDoc/hardware.md | grep -v "Strapping\|NO USADOS"
grep -E "pub const.*_PIN: u8" src/config/constants.rs
```

**Expected:** All pins documented in hardware.md appear in constants.rs

### 2. READ Format Verification

```bash
# Check format_read_response implementation
grep -A5 "format_read_response" src/output/artisan.rs
```

**Expected:** 4 comma-separated values (ET, BT, HEATER, FAN)

### 3. OT2 Documentation Check

```bash
# Verify OT2 is documented
grep -i "OT2" internalDoc/hardware.md
```

**Expected:** OT2 should appear in supported commands table

### 4. Thermocouple Count Verification

```bash
# Verify no ET2/BT2 references
grep -i "ET2\|BT2" src/
```

**Expected:** No results (only BT and ET exist)

---

## Recommendations

### Update Priority:

**High (Required for v2.3):**
1. Fix READ format from 7-value to 4-value
2. Add OT2 command documentation
3. Update communication diagram to show 4-value format

**Medium (Recommended):**
4. Add "Last Updated" timestamp
5. Clarify OT2 vs IO3 distinction in documentation

**Low (Nice to have):**
6. Add note about OT2 decimals vs IO3 integer-only

### Testing After Update:

1. Read updated hardware.md
2. Verify pin table matches constants.rs
3. Verify READ format matches PROTOCOL.md
4. Verify OT2 is in commands table
5. Verify no ET2/BT2 mentions remain

---

## Research Notes

**Files Reviewed:**
- src/config/constants.rs
- internalDoc/hardware.md
- internalDoc/ARCHITECTURE.md
- src/output/artisan.rs (format_read_response)
- src/input/parser.rs (OT2 parsing)

**Key Source Code Findings:**
- SystemStatus.fan_output: f32 (line 141 in constants.rs)
- format_read_response returns 4 values: ET, BT, heater, fan_speed
- OT2 command: SetFanSpeed(u8, bool) with decimal rounding

**v2.2 Hardware Stability:**
- Pin assignments unchanged from v2.0/v2.1
- Only software protocol changes (READ format, OT2 command)
- No PCB changes required for v2.2

---

*Research complete. Ready for planning Phase 41.*
