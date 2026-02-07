# Project Research Summary

**Project:** LibreRoaster ESP32-C3 Firmware
**Domain:** Artisan Protocol Command Parsing — Embedded IoT
**Researched:** February 7, 2026
**Confidence:** HIGH

## Executive Summary

LibreRoaster's Artisan protocol implementation for ESP32-C3 firmware requires **no stack additions** — the existing architecture using `embassy-rs`, `esp-hal`, and `heapless` fully supports the required OT2 (fan control), READ (telemetry), and UNITS (temperature scale) commands. The implementation work is purely extension of existing code patterns, not infrastructure expansion. The parser, ArtisanCommand enum, and ArtisanFormatter provide all necessary foundations; the gaps are in OT2 parser support, READ command wiring, and temperature unit state management.

The recommended approach follows a three-phase roadmap: Phase 1 implements OT2 command parsing and fan rate-limiting; Phase 2 wires READ telemetry responses through existing formatters; Phase 3 adds temperature unit conversion state. Critical risks include ESP32-C3 UART serial timing issues under load, temperature conversion accuracy bugs (the most reported Artisan integration issues in community forums), and fan speed control smoothness to avoid airflow fluctuations during roasting. All pitfalls are preventable with existing stack capabilities — no new dependencies required.

## Key Findings

### Recommended Stack

No changes to Cargo.toml or dependencies required. The existing LibreRoaster stack already provides:

| Category | Current Stack | Status |
|----------|---------------|--------|
| Async Runtime | `embassy-executor` 0.9.1 | ✅ Sufficient |
| Hardware Access | `esp-hal` ~1.0 | ✅ Sufficient |
| Command Parsing | `heapless` 0.8.0 | ✅ Sufficient |
| Serial Communication | USB CDC + UART0 | ✅ Working |
| Logging | `log` 0.4.27 | ✅ Sufficient |

**Required implementation additions:**
- OT2 parser pattern (~2 lines in `parser.rs`)
- `SetFanRateLimited` enum variant (1 line in `constants.rs`)
- Rate-limited fan handler (~15 lines in `handlers.rs`)
- Temperature state field (`use_fahrenheit: bool` in `ArtisanFormatter`)

### Expected Features

**Must have (table stakes):**
- READ command — Returns `ET,BT,FAN,HEATER` CSV telemetry format (existing formatter, needs wiring)
- OT2 command — Fan speed 0-100% with rate limiting (25 points/sec max per Artisan spec)
- UNITS command — Temperature scale C/F switching (parsed but not applied — **critical gap**)
- BT2/ET2 placeholders — Returns `-1` for disabled channels (correctly implemented)

**Should have (competitive):**
- Extended READ response format — Includes fan/heater output states (differentiator over standard Artisan)
- Multiple fan control aliases — Support both OT2 and IO3 syntax (compatibility)
- Hardware PWM at 25kHz — Quiet fan operation (already implemented)

**Defer (v2+):**
- Temperature unit conversion at sensor read (only convert at output formatting)
- Command acknowledgment for output changes
- Historical telemetry storage

### Architecture Approach

LibreRoaster uses a well-designed command handler chain pattern that OT2, READ, and UNITS integrate into with minimal modifications. The flow is: Serial Input → Parser → Multiplexer → ArtisanInput Task → RoasterControl::process_artisan_command() → ArtisanCommandHandler → Hardware → ArtisanFormatter → Serial Output. The ArtisanFormatter already has READ response methods; the gap is wiring the ReadStatus command to trigger response formatting. Temperature unit state should be stored in ArtisanFormatter to avoid contaminating internal calculations with display-only conversions.

**Major components:**
1. **Parser** (`src/input/parser.rs`) — Parses ASCII commands to ArtisanCommand enum, needs OT2 pattern added
2. **ArtisanCommandHandler** (`src/control/handlers.rs`) — Executes commands, routes SetFan to fan hardware
3. **ArtisanFormatter** (`src/output/artisan.rs`) — Formats READ responses, needs use_fahrenheit state added
4. **RoasterControl** — Central dispatch, maintains SystemStatus for telemetry

### Critical Pitfalls

1. **READ Command Response Format Mismatch** — Artisan fails to recognize data with incorrect CSV formatting or line endings. Prevention: Use exact format `ET,BT,FAN,HEATER\r\n` with single decimal place.

2. **Temperature Unit Conversion Accuracy** — Most reported Artisan integration bugs. Prevention: Store canonical temperatures in Celsius, convert only at READ output using formula F = (C × 9/5) + 32.

3. **ESP32-C3 UART Serial Timing Issues** — USB CDC serial can freeze under high-traffic roasting scenarios. Prevention: Use 115200 baud, target <100ms response times, test under realistic load.

4. **Fan Speed Control Smoothness** — Abrupt PWM changes cause airflow fluctuations affecting roast. Prevention: Implement ramping (5% step per 100ms) for fan transitions.

5. **OT2 Command Parsing Edge Cases** — Various delimiter formats (comma, space, semicolon, equals) must all be handled. Prevention: Normalize input before parsing, test all delimiter variations.

## Implications for Roadmap

Based on research, the suggested three-phase structure respects dependencies and avoids known pitfalls.

### Phase 1: OT2 Command Implementation
**Rationale:** OT2 is the foundation for fan control and has no dependencies. It follows the existing OT1/IO3 pattern exactly, making it the lowest-risk starting point.

**Delivers:**
- OT2 parser pattern in `parser.rs`
- `SetFanRateLimited` enum variant
- Rate-limited fan handler (25 points/sec per Artisan spec)
- Integration test verifying fan actuation

**Addresses:**
- Pitfall 1: OT2 parsing edge cases
- Pitfall 5: Fan speed control smoothness
- Feature: OT2 command fan control

**Research Flags:** LOW — Standard parser extension pattern, well-documented in Artisan spec. Skip phase research.

### Phase 2: READ Telemetry Wiring
**Rationale:** READ depends on OT2 completion because fan state must be available in SystemStatus. The ArtisanFormatter already exists; this phase wires the command path.

**Delivers:**
- ReadStatus command triggers ArtisanFormatter response
- BT2/ET2 comments clarifying disabled channels
- Full 7-value response format: `ET,BT,-1,-1,-1,FAN,HEATER\r\n`

**Addresses:**
- Pitfall 2: READ response format mismatch
- Feature: Extended READ response with output states

**Research Flags:** MEDIUM — Response timing needs validation against actual Artisan version. Test format with live Artisan during implementation.

### Phase 3: UNITS Temperature Conversion
**Rationale:** UNITS is the final piece; it depends on READ being functional to verify temperature display. Temperature conversion is the highest-risk area (most community bug reports).

**Delivers:**
- `use_fahrenheit: bool` state in ArtisanFormatter
- UNITS command updates formatter state
- Temperature conversion in format_read_response()
- Verification: C and F modes work correctly

**Addresses:**
- Pitfall 3: Temperature conversion accuracy
- Pitfall 9: UNITS state persistence (consider NVS for power-cycle safety)

**Research Flags:** HIGH — Conversion accuracy critical, needs validation during implementation. Test round-trip conversions.

### Phase Ordering Rationale

- **OT2 → READ → UNITS** order respects data dependencies (fan state needed for READ, READ needed to verify UNITS conversion)
- **Three phases** matches the natural component boundaries (parser, formatter, state)
- **Phases 1-2 are low risk** (extend existing patterns), Phase 3 requires careful testing (conversion bugs are common)
- Prevents **Pitfall 7** (command state machine conflicts) by sequencing integrations

### Research Flags Summary

| Phase | Research Level | Notes |
|-------|----------------|-------|
| Phase 1: OT2 | SKIP | Well-documented Artisan spec, existing parser pattern |
| Phase 2: READ | LIGHT | Verify response timing, test format with live Artisan |
| Phase 3: UNITS | DEEP | Conversion accuracy critical, community bug history |

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Zero additions required, existing stack verified |
| Features | HIGH | READ formatter exists, OT2/UNITS patterns clear |
| Architecture | HIGH | Command flow well-designed, integration points identified |
| Pitfalls | MEDIUM | Based on community experiences, some ESP32-C3 UART gaps |

**Overall confidence:** HIGH

### Gaps to Address

- **Exact Artisan timeout values** — Varies by version. Validate during Phase 2 integration testing.
- **USB CDC vs UART selection impact** — Both modes documented, tradeoffs may affect serial reliability. Test under realistic roast load.
- **embassy-rs UART behaviors** — Limited documentation on interaction with Artisan protocol timing. Monitor response times during implementation.

## Sources

### Primary (HIGH confidence)
- **TC4-shield Artisan Protocol Specification** — https://github.com/greencardigan/TC4-shield/blob/master/applications/Artisan/aArtisan/trunk/src/aArtisan/commands.txt — Official command syntax, OT2 rate limiting (25 points/sec), READ response format
- **LibreRoaster Current Implementation** — Direct code inspection of parser.rs, artisan.rs, constants.rs — Verified existing patterns and gaps

### Secondary (MEDIUM confidence)
- **Artisan Scope Documentation** — https://artisan-scope.org/docs/setup/ — General configuration guidelines, version-specific behaviors need validation
- **ESP32-C3 GitHub Issues** — UART timing documented but chip-specific, USB CDC freezing under load reported
- **Homeroasters Community Forum** — User experiences with Skywalker roasters, common Fahrenheit conversion bugs documented

### Tertiary (LOW confidence)
- **Skywalker Roaster Firmware** — Similar hardware, pitfall patterns applicable but exact implementations differ

---

*Research completed: 2026-02-07*
*Ready for roadmap: yes*
