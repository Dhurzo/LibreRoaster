# Architecture Patterns: OT2, READ, and UNITS Integration

**Domain:** Artisan protocol command parsing for ESP32-C3 coffee roaster firmware
**Researched:** February 2026
**Confidence:** HIGH - Based on codebase analysis and official Artisan protocol specification

## Executive Summary

This document outlines the architecture for integrating OT2 (fan control), READ (telemetry response), and UNITS (temperature scale) commands into the existing LibreRoaster ESP32-C3 firmware. The existing architecture has a well-designed command handler chain pattern that OT2, READ, and UNITS can integrate into with minimal modifications.

**Key findings:**
- OT2 maps directly to existing `SetFan` command via `ArtisanCommandHandler`
- READ requires a new response formatting path that already partially exists in `ArtisanFormatter`
- UNITS needs a temperature unit state that's consulted during READ response formatting
- Existing architecture correctly returns `-1` for BT2/ET2 channels (single thermocouple design)

## Recommended Architecture

### Command Flow Overview

```
USB/UART Input
     ↓
Parser (parse_artisan_command)
     ↓
CommandMultiplexer (channel routing, UNITS logging)
     ↓
ArtisanCommand Channel → ArtisanInput Task
     ↓
RoasterControl::process_artisan_command()
     ↓
├─ ArtisanCommandHandler (OT1→SetHeater, OT2/IO3→SetFan, READ→status, UNITS→log)
└─ TemperatureCommandHandler, SafetyCommandHandler, SystemCommandHandler
     ↓
Hardware (SSR, FanController, Max31856 sensors)
     ↓
Response Formatting (ArtisanFormatter)
     ↓
USB/UART Output
```

### Component Boundaries

| Component | Responsibility | Communicates With |
|-----------|---------------|------------------|
| `input/parser.rs` | Parse ASCII commands to `ArtisanCommand` enum | `input/multiplexer.rs` |
| `input/multiplexer.rs` | Channel routing, UNITS/CHAN/FILT handling (logging only) | `parser.rs`, `application/service_container.rs` |
| `control/handlers.rs` | Execute Artisan commands (heater, fan, telemetry) | `control/roaster_refactored.rs` |
| `control/roaster_refactored.rs` | Command dispatch, hardware abstraction | `handlers.rs`, `hardware/fan.rs`, `hardware/ssr.rs` |
| `output/artisan.rs` | Format telemetry responses | `application/service_container.rs` |
| `application/service_container.rs` | Global access to roaster control | All above components |

### Data Flow for OT2 Command

```
1. User sends: "OT2 75\n"
2. Parser → ArtisanCommand::SetFan(75)
3. Multiplexer → routes to ArtisanInput
4. ArtisanInput → sends ArtisanCommand through channel
5. RoasterControl::process_artisan_command(SetFan(75))
   ├─ Calls: RoasterCommand::SetFanManual(75)
   ├─ Dispatch: ArtisanCommandHandler::handle_command()
   └─ Update: status.fan_output = 75.0
6. RoasterControl::update_control() called each cycle
   ├─ Calls: fan.set_speed(75.0) → hardware/fan.rs
   └─ Calls: status.fan_output = artisan_handler.get_manual_fan()
7. No response sent (Artisan protocol: OT2 has no response)
```

### Data Flow for READ Command

```
1. User sends: "READ\n"
2. Parser → ArtisanCommand::ReadStatus
3. Multiplexer → routes to ArtisanInput
4. ArtisanInput → sends ArtisanCommand through channel
5. RoasterControl::process_artisan_command(ReadStatus)
   ├─ Updates: status.ssr_hardware_status = heater.get_status()
   └─ Logs: "READ command - SSR status: Available"
6. Reader task queries ServiceContainer for status
7. ArtisanFormatter::format_read_response(status, fan_speed)
   ├─ Applies UNITS conversion (C → F if needed)
   ├─ Formats: "ET,BT,FAN,HEATER" CSV line
   └─ Returns: "120.3,150.5,25.0,75.0\n"
8. Response sent to USB/UART
```

### Data Flow for UNITS Command

```
1. User sends: "UNITS;F\n"
2. Parser → ArtisanCommand::Units(true) // true = Fahrenheit
3. Multiplexer → logs "Units command received"
4. No state change (handshake disabled)
5. Future: Store in RoasterControl or ArtisanFormatter
6. READ responses convert C → F when Units = Fahrenheit
```

## OT2 Integration with Control Module

### Current State

The `ArtisanCommand` enum already has the necessary variant:

```rust
// src/config/constants.rs
pub enum ArtisanCommand {
    SetHeater(u8),   // OT1 maps here
    SetFan(u8),      // OT2 and IO3 map here
    ReadStatus,      // READ maps here
    Units(bool),       // UNITS maps here
    // ...
}
```

The `ArtisanCommandHandler` already implements fan control:

```rust
// src/control/handlers.rs (lines 267-279)
impl RoasterCommandHandler for ArtisanCommandHandler {
    fn handle_command(&mut self, command: ...) -> Result<(), RoasterError> {
        match command {
            RoasterCommand::SetFanManual(value) => {
                status.artisan_control = true;
                status.pid_enabled = false;
                self.manual_fan = value as f32;
                status.fan_output = value as f32;
                // ...
                Ok(())
            }
            // ...
        }
    }
}
```

### Required Parser Updates

The parser already handles OT1 and IO3. Need to add OT2 support:

```rust
// src/input/parser.rs - Current (lines 68-76)
["OT1", value_str] => {
    let value = parse_percentage(value_str)?;
    Ok(ArtisanCommand::SetHeater(value))
}

["IO3", value_str] => {
    let value = parse_percentage(value_str)?;
    Ok(ArtisanCommand::SetFan(value))
}

// Add OT2 support (lines after IO3):
["OT2", value_str] => {
    let value = parse_percentage(value_str)?;
    Ok(ArtisanCommand::SetFan(value))
}
```

**Rationale:** OT2 and IO3 are functionally equivalent in the Artisan protocol - both control fan output. Using the existing `SetFan` variant avoids duplication.

### Integration Points

| From | To | Action |
|------|-----|--------|
| Parser `parse_artisan_command()` | Parser variant matching | Add `["OT2", value_str]` case |
| Parser `ArtisanCommand::SetFan` | RoasterControl | Routes to `apply_manual_fan()` |
| RoasterControl | `artisan_handler.set_manual_fan()` | Updates state |
| RoasterControl | `fan.set_speed()` hardware call | Physical fan actuation |
| Status | `status.fan_output` | Persists for telemetry |

## READ Telemetry Aggregation Flow

### Current Implementation

The `ArtisanFormatter` already has READ response formatters:

```rust
// src/output/artisan.rs (lines 101-119)
impl ArtisanFormatter {
    pub fn format_read_response(status: &SystemStatus, fan_speed: f32) -> String {
        format!(
            "{:.1},{:.1},{:.1},{:.1}",
            status.env_temp,   // ET
            status.bean_temp,  // BT
            status.ssr_output, // Power (heater)
            fan_speed          // Fan
        )
    }

    pub fn format_read_response_full(status: &SystemStatus) -> String {
        format!(
            "{:.1},{:.1},-1,-1,-1,{:.1},{:.1}\r\n",
            status.env_temp,   // ET
            status.bean_temp,  // BT
            status.fan_output, // Fan
            status.ssr_output  // Heater
        )
    }
}
```

### Required Integration

The response formatting exists but needs to be wired into the command path:

1. **Command Handler Update:** `process_artisan_command(ReadStatus)` should trigger response
2. **Reader Task Integration:** USB/UART reader tasks should call formatter after READ
3. **Temperature Unit Conversion:** Apply `C → F` conversion when `Units = Fahrenheit`

**READ Response Format:**

| Field | Source | Notes |
|-------|--------|-------|
| ET | `status.env_temp` | Environment temperature |
| BT | `status.bean_temp` | Bean temperature |
| FAN | `status.fan_output` | Fan speed percentage |
| HEATER | `status.ssr_output` | Heater power percentage |

**Example Response:** `120.3,150.5,25.0,75.0\n`

### Temperature Unit Handling Strategy

#### Option A: Store in ArtisanFormatter (Recommended)

```rust
pub struct ArtisanFormatter {
    start_time: Instant,
    last_bt: f32,
    bt_history: Vec<f32>,
    use_fahrenheit: bool, // NEW: temperature unit state
}
```

**Pros:** Formatter already handles output, clean separation
**Cons:** UNITS command currently doesn't change state

#### Option B: Store in RoasterControl

```rust
pub struct RoasterControl {
    // ... existing fields ...
    temperature_scale: TemperatureScale, // NEW
}

enum TemperatureScale {
    Celsius,
    Fahrenheit,
}
```

**Pros:** Centralizes state, visible to all components
**Cons:** Requires changes to multiple modules

#### Recommendation: Option A

Add `use_fahrenheit: bool` to `ArtisanFormatter` with default `false` (Celsius).

**Conversion formula:**
```rust
fn celsius_to_fahrenheit(c: f32) -> f32 {
    c * 9.0 / 5.0 + 32.0
}
```

Apply conversion in `format_read_response()` when `use_fahrenheit = true`.

## BT2/ET2 Disabled Comment Placement

The existing code correctly returns `-1` for unused channels:

```rust
// src/output/artisan.rs (lines 113-118)
pub fn format_read_response_full(status: &SystemStatus) -> String {
    format!(
        "{:.1},{:.1},-1,-1,-1,{:.1},{:.1}\r\n",
        status.env_temp,   // ET (chan1)
        status.bean_temp,  // BT (chan2)
        -1,               // ET2 (disabled)
        -1,               // BT2 (disabled)
        -1,               // ambient (disabled)
        status.fan_output,
        status.ssr_output
    )
}
```

**Comments already in place:** The comment at `src/input/multiplexer.rs` line 26-28:

```rust
// NOTE: Handshake (CHAN → UNITS → FILT) is DISABLED for Artisan Scope compatibility
// Artisan Scope does not perform handshake - it simply sends and receives data
// Placeholder types kept for potential future re-enabling
```

**BT2/ET2 comments:** Should add in `format_read_response_full()`:

```rust
pub fn format_read_response_full(status: &SystemStatus) -> String {
    // BT2 and ET2 thermocouples are not installed (single-sensor design)
    // Artisan READ response format: ET,BT,ET2,BT2,ambient,FAN,HEATER
    format!(
        "{:.1},{:.1},-1,-1,-1,{:.1},{:.1}\r\n",
        status.env_temp,   // ET
        status.bean_temp,  // BT
        -1,               // ET2 (not installed)
        -1,               // BT2 (not installed)
        -1,               // ambient (not installed)
        status.fan_output,
        status.ssr_output
    )
}
```

## Phase-Specific Architecture

### Phase 1: OT2 Command Integration

**Scope:** Add OT2 parser support, test hardware integration

**Dependencies:** None (uses existing SetFan infrastructure)

**Changes:**
1. Add `["OT2", value_str]` case in `parser.rs`
2. Add integration test for OT2 → fan actuation
3. Verify fan responds to OT2 commands

**Success criteria:**
- `OT2 50` sets fan to 50%
- No response sent (matches Artisan protocol)
- Fan speed reflects in READ response

### Phase 2: READ Telemetry Response

**Scope:** Wire READ command to response formatter

**Dependencies:** Phase 1 complete

**Changes:**
1. Update `ArtisanCommandHandler::handle_command(ReadStatus)` to return data
2. Wire reader tasks to call `ArtisanFormatter::format_read_response()`
3. Add UNITS conversion state to `ArtisanFormatter`

**Success criteria:**
- `READ` returns `ET,BT,FAN,HEATER\n` CSV format
- Response terminates with `\r\n`
- Values reflect current system state

### Phase 3: UNITS Temperature Scale

**Scope:** Store and apply temperature unit conversions

**Dependencies:** Phase 2 complete

**Changes:**
1. Add `use_fahrenheit: bool` to `ArtisanFormatter`
2. Parse UNITS command into state
3. Apply conversion in `format_read_response()`

**Success criteria:**
- `UNITS;C` returns Celsius values
- `UNITS;F` returns Fahrenheit values
- Conversion formula correct: `C × 9/5 + 32`

## Build Order (Dependency Respecting)

```
Phase 1: OT2 Command
├─ Task 1.1: Add OT2 parser case
├─ Task 1.2: Test OT2 → SetFan routing
├─ Task 1.3: Verify fan actuation
└─ Task 1.4: Integration test

Phase 2: READ Telemetry
├─ Task 2.1: Add READ response path
├─ Task 2.2: Wire formatter to command
├─ Task 2.3: Add BT2/ET2 comments
└─ Task 2.4: Integration test

Phase 3: UNITS Scale
├─ Task 3.1: Add Fahrenheit state
├─ Task 3.2: Parse UNITS → state
├─ Task 3.3: Apply conversion
└─ Task 3.4: Integration test
```

## Anti-Patterns to Avoid

### Anti-Pattern 1: Duplicate Fan Control Paths

**What:** Implementing OT2 and IO3 as separate handlers

**Why bad:** Duplicates code, inconsistent behavior between commands

**Instead:** Route both OT2 and IO3 to same `SetFan` command

### Anti-Pattern 2: Blocking READ Responses

**What:** Waiting in `process_artisan_command()` for READ response

**Why bad:** Blocks command processing, violates async architecture

**Instead:** Update status in handler, let reader task format response

### Anti-Pattern 3: Temperature Conversion at Sensor Read

**Why bad:** Would affect PID, logging, and all other temperature uses

**Instead:** Convert only at output formatting (READ, Artisan CSV lines)

### Anti-Pattern 4: Ignoring Artisan Protocol Spec

**What:** Sending responses for OT1/OT2 (protocol says no response)

**Why bad:** Confuses Artisan software, breaks protocol compliance

**Instead:** Only send response for READ command

## Scalability Considerations

| Concern | At Current Scope | At 10K users | At 100K users |
|---------|-----------------|--------------|---------------|
| Command parsing | heapless::Vec, O(n) | Same | Same |
| Telemetry formatting | String<128>, single READ | Same | Consider streaming |
| Temperature conversion | Inline in formatter | Same | Same |
| State storage | bool flag | Same | Same |

The current architecture scales well for single-device Artisan control. No changes needed for increased scale.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| OT2 integration | HIGH | Parser → SetFan → handler path exists |
| READ response | HIGH | Formatter exists, needs wiring |
| UNITS handling | MEDIUM | Conversion strategy defined, state placement needs validation |
| Protocol compliance | HIGH | Based on official Artisan commands.txt spec |
| Phase ordering | HIGH | Dependency-respecting order clear |

## Gaps to Address in Later Phases

- **Multi-roaster support:** Current singleton `ServiceContainer` limits multiple roasters
- **Historical telemetry:** READ returns current state only, no historical data
- **WebSocket streaming:** Artisan can stream, current implementation is request-response
- **Extended channels:** ET2, BT2, ambient placeholders for future thermocouples

## Sources

- [TC4-shield Artisan Protocol Commands.txt](https://raw.githubusercontent.com/greencardigan/TC4-shield/master/applications/Artisan/aArtisan/trunk/src/aArtisan/commands.txt) - Official Artisan serial command specification
- [TC4+ Arduino Shield Manual](https://artisanaltechnologies.co.uk/assets/TC4plus_Manual.pdf) - Hardware documentation
- LibreRoaster codebase analysis - Current implementation reviewed
