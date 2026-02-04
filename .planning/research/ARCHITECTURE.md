# Architecture Research: Artisan Serial Protocol Integration

**Project:** LibreRoaster ESP32-C3 Firmware  
**Research Date:** February 4, 2025  
**Focus:** Full Artisan serial protocol integration with existing UART architecture  
**Confidence:** HIGH - Protocol documented, codebase analyzed

---

## Executive Summary

LibreRoaster currently implements a **subset** of the Artisan TC4 serial protocol, supporting basic commands (READ, OT1, IO3, START, STOP) with a working UART infrastructure. The existing architecture provides solid foundations:

- Embassy async task framework for UART I/O
- Command multiplexer for dual-channel (UART + USB CDC) support
- ArtisanFormatter for READ response generation
- RoasterControl with ArtisanCommandHandler for command processing

To achieve **full Artisan protocol compatibility**, the implementation needs:

1. **Protocol initialization sequence** (CHAN → UNITS → FILT acknowledgment)
2. **Extended command set** (PID, DCFAN, extended OT2 support)
3. **Enhanced delimiter parsing** (semicolon, comma, space, equals)
4. **Bidirectional streaming** during active roasting
5. **Command acknowledgment system** with `#` prefix responses

The existing architecture integrates well with these requirements. Minimal structural changes are needed—primarily parser extensions and response handling enhancements.

---

## Current Architecture Analysis

### Existing Component Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         LibreRoaster Architecture                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────┐     ┌──────────────────────────────────────────┐       │
│  │  Artisan        │     │          UART Hardware Layer             │       │
│  │  Software      │◄────┤  ┌──────────────────────────────────┐    │       │
│  │  (Host)        │     │  │ UART0 on GPIO20/21 (ESP32-C3)    │    │       │
│  └────────┬────────┘     │  │ 115200 baud, 8N1                  │    │       │
│           │              │  │ get_uart_driver() singleton        │    │       │
│           ▼              │  └──────────────┬─────────────────────┘    │       │
│  ┌─────────────────┐     │               │                          │       │
│  │ uart_reader_task │     │               ▼                          │       │
│  │ (Embassy task)  │────►│  ┌──────────────────────────────────┐    │       │
│  │ - 64-byte reads │     │  │ CircularBuffer + COMMAND_PIPE     │    │       │
│  │ - Line assembly │     │  │ (embassy_sync::Pipe)              │    │       │
│  └────────┬────────┘     │  └──────────────┬─────────────────────┘    │       │
│           │              │                 │                          │       │
│           ▼              │                 ▼                          │       │
│  ┌─────────────────┐     │  ┌──────────────────────────────────┐    │       │
│  │ process_command │     │  │ uart_writer_task                 │    │       │
│  │ (in tasks.rs)  │     │  │ - Writes responses to UART       │    │       │
│  │ - Parser::     │     │  │ - Handles stream output          │    │       │
│  │   parse_       │     │  └──────────────────────────────────┘    │       │
│  │   artisan_cmd  │     │                                         │       │
│  └────────┬────────┘     └──────────────────────────────────────────┘       │
│           │                                                                │
│           ▼                                                                │
│  ┌───────────────────────────────────────────────────────────────────┐       │
│  │                    Command Multiplexer                             │       │
│  │  ┌─────────────────────────────────────────────────────────────┐  │       │
│  │  │ CommChannel::Uart / CommChannel::Usb                        │  │       │
│  │  │ 60-second idle timeout for channel switching                 │  │       │
│  │  │ on_command_received() + should_write_to()                  │  │       │
│  │  └─────────────────────────────────────────────────────────────┘  │       │
│  └───────────────────────────────────────────────────────────────────┘       │
│           │                                                                │
│           ▼                                                                │
│  ┌───────────────────────────────────────────────────────────────────┐       │
│  │              ServiceContainer (Dependency Injection)               │       │
│  │  ┌────────────────┐  ┌────────────────┐  ┌────────────────────┐  │       │
│  │  │ ARTISAN_CMD_   │  │ ARTISAN_      │  │ MULTIPLEXER       │  │       │
│  │  │ CHANNEL (8)    │  │ OUTPUT_CH     │  │ CommandMultiplexer │  │       │
│  │  │ ArtisanCommand │  │ (16) String   │  │                    │  │       │
│  │  └────────────────┘  └────────────────┘  └────────────────────┘  │       │
│  └───────────────────────────────────────────────────────────────────┘       │
│           │                                                                │
│           ▼                                                                │
│  ┌───────────────────────────────────────────────────────────────────┐       │
│  │              RoasterControl + Handlers                             │       │
│  │  ┌─────────────────────────────────────────────────────────────┐  │       │
│  │  │ ArtisanCommandHandler                                        │  │       │
│  │  │ - SetHeaterManual(u8) → status.ssr_output                  │  │       │
│  │  │ - SetFanManual(u8) → status.fan_output                     │  │       │
│  │  │ - clear_manual()                                           │  │       │
│  │  └─────────────────────────────────────────────────────────────┘  │       │
│  └───────────────────────────────────────────────────────────────────┘       │
│           │                                                                │
│           ▼                                                                │
│  ┌───────────────────────────────────────────────────────────────────┐       │
│  │              ArtisanFormatter                                       │       │
│  │  ┌─────────────────────────────────────────────────────────────┐  │       │
│  │  │ format_read_response() → "ET,BT,Power,Fan" (CSV)          │  │       │
│  │  │ format() → "time,ET,BT,ROR,Gas" (streaming CSV)         │  │       │
│  │  └─────────────────────────────────────────────────────────────┘  │       │
│  └───────────────────────────────────────────────────────────────────┘       │
│           │                                                                │
│           ▼                                                                │
│  ┌───────────────────────────────────────────────────────────────────┐       │
│  │              dual_output_task                                      │       │
│  │  - Routes responses to active CommChannel (Uart or Usb)           │       │
│  │  - Appends "\r\n" to responses                                  │       │
│  └───────────────────────────────────────────────────────────────────┘       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Existing ArtisanCommand Coverage

| Command | Parser Support | Handler Support | Response |
|---------|---------------|-----------------|----------|
| READ | ✅ `parse_artisan_command("READ")` | ✅ `process_artisan_command(ReadStatus)` | ✅ `format_read_response()` |
| START | ✅ `parse_artisan_command("START")` | ✅ `process_artisan_command(StartRoast)` | ❌ No response |
| OT1 | ✅ `parse_artisan_command("OT1 x")` | ✅ `ArtisanCommandHandler` | ❌ No acknowledgment |
| IO3 | ✅ `parse_artisan_command("IO3 x")` | ✅ `ArtisanCommandHandler` | ❌ No acknowledgment |
| STOP | ✅ `parse_artisan_command("STOP")` | ✅ `SafetyCommandHandler` | ❌ No acknowledgment |

---

## Artisan Serial Protocol Specification

### Protocol Fundamentals (from TC4 specification)

The Artisan TC4 protocol operates at **115200 baud** with the following characteristics:

1. **Line-oriented**: Commands terminated by `\n` (LF)
2. **Multi-delimiter**: Accepts comma `,`, space ` `, semicolon `;`, equals `=` as parameter separators
3. **Acknowledgment system**: Responses prefixed with `#` for successful commands
4. **Initialization sequence**: CHAN → UNITS → FILT must complete before READ loop begins
5. **Streaming mode**: Continuous temperature output during active roasting

### Full Command Set

#### Initialization Commands

| Command | Format | Purpose | Expected Response |
|---------|--------|---------|------------------|
| CHAN | `CHAN;ijkl` or `CHAN ijkl` | Map physical ports to logical channels | `# Active channels set to ijkl` |
| UNITS | `UNITS;C` or `UNITS F` | Set temperature scale (C/F) | None |
| FILT | `FILT;l1,l2,l3,l4` | Set digital filtering (0-100%) | None |

#### Data Commands

| Command | Format | Purpose | Response |
|---------|--------|---------|----------|
| READ | `READ` | Request current temperatures | `ambient,chan1,chan2,chan3,chan4` |
| OT1 | `OT1;duty` or `OT1 duty` | Heater PWM (0-100%) | None |
| OT2 | `OT2;duty` or `OT2 duty` | Secondary heater/output | None |
| IO3 | `IO3;duty` or `IO3 duty` | Fan PWM (0-100%) | None |
| DCFAN | `DCFAN;duty` or `DCFAN duty` | Fan with rate limiting | None |

#### PID Commands

| Command | Format | Purpose |
|---------|--------|---------|
| PID;ON | `PID;ON` or `PID ON` | Enable PID control |
| PID;OFF | `PID;OFF` or `PID OFF` | Disable PID control |
| PID;SV;vvv | `PID;SV;vvv` or `PID SV vvv` | Set PID setpoint |
| PID;T;ppp;iii;ddd | `PID;T;ppp;iii;ddd` | Set PID tuning parameters |
| PID;CHAN;i | `PID;CHAN;i` | Set PID input channel |
| PID;CT;mmmm | `PID;CT;mmmm` | Set PID cycle time (ms) |

### Response Formats

#### READ Response (Active Channels)
```
# Expected format from Artisan TC4:
ambient,chan1,chan2,chan3,chan4

# Example:
23.5,120.3,150.5,2.1,75.0

# Where:
# - ambient: cold junction/reference temperature
# - chan1: ET (Environment/Bean Temperature #1)
# - chan2: BT (Bean Temperature #2)
# - chan3-4: Additional channels if active
```

#### Acknowledgment Responses
```
# Successful command:
# <acknowledgment message>

# Example:
# Active channels set to 1200
```

#### Error Responses (Artisan convention)
```
# If implemented:
ERR <error_code> <message>

# Example:
ERR invalid_value "OT1 value must be 0-100"
```

### Standard Artisan CSV Streaming Format

During active roasting, Artisan expects streaming CSV format:

```
time,ET,BT,ROR,Gas
```

| Field | Description | Example |
|-------|-------------|---------|
| time | Seconds.milliseconds since start | `123.45` |
| ET | Environment temperature (°C) | `120.3` |
| BT | Bean temperature (°C) | `150.5` |
| ROR | Rate of rise (°C/second) | `2.15` |
| Gas | Heater output (%) | `75.0` |

---

## Integration Points Analysis

### Integration Point 1: Parser Enhancement

**Location:** `src/input/parser.rs`

**Current State:**
```rust
pub fn parse_artisan_command(command: &str) -> Result<ArtisanCommand, ParseError> {
    let parts: heapless::Vec<&str, 4> = trimmed.split_whitespace().collect();
    match parts.as_slice() {
        ["READ"] => Ok(ArtisanCommand::ReadStatus),
        ["START"] => Ok(ArtisanCommand::StartRoast),
        ["OT1", value_str] => Ok(ArtisanCommand::SetHeater(parse_percentage(value_str)?)),
        ["IO3", value_str] => Ok(ArtisanCommand::SetFan(parse_percentage(value_str)?)),
        ["STOP"] => Ok(ArtisanCommand::EmergencyStop),
        _ => Err(ParseError::UnknownCommand),
    }
}
```

**Required Changes:**
1. Support semicolon delimiters: `split(|c| c == ' ' || c == ';' || c == ',' || c == '=')`
2. Add PID command variants
3. Add OT2 command support
4. Add initialization command parsing (CHAN, UNITS, FILT)
5. Add numeric parameter parsing for PID tuning

**New ArtisanCommand Variants Needed:**
```rust
pub enum ArtisanCommand {
    ReadStatus,
    StartRoast,
    SetHeater(u8),      // OT1
    SetFan(u8),         // IO3
    SetHeater2(u8),     // OT2 (new)
    SetDcfan(u8),       // DCFAN (new)
    PidOn,              // PID;ON
    PidOff,             // PID;OFF
    PidSv(f32),         // PID;SV;value
    PidTuning { kp: f32, ki: f32, kd: f32 }, // PID;T;...
    PidChan(u8),        // PID;CHAN;channel
    PidCycleTime(u16),  // PID;CT;ms
    ChanMapping(u8, u8, u8, u8), // CHAN;ijkl
    Units(char),        // UNITS;C or UNITS;F
    Filter(u8, u8, u8, u8), // FILT;l1,l2,l3,l4
    EmergencyStop,
}
```

### Integration Point 2: Response Handling

**Location:** `src/hardware/uart/tasks.rs` and `src/application/tasks.rs`

**Current State:**
- `send_response()` writes directly to UART
- `send_stream()` writes to COMMAND_PIPE for streaming
- No acknowledgment system implemented

**Required Changes:**
1. Implement acknowledgment responses (`#...` format)
2. Add command-specific response handlers
3. Create response builder for structured responses
4. Integrate with dual_output_task for channel-aware responses

**New Response Handler Interface:**
```rust
pub async fn send_ack(response: &str) -> Result<(), InputError> {
    let mut response = String::from("# ");
    response.push_str(response);
    send_response(&response).await
}

pub async fn send_error(code: &str, message: &str) -> Result<(), InputError> {
    let response = format!("ERR {} {}", code, message);
    send_response(&response).await
}
```

### Integration Point 3: Initialization Sequence

**Location:** `src/input/multiplexer.rs` (new handler)

**Required Behavior:**
```rust
impl ArtisanProtocolHandler {
    async fn handle_init_sequence(&mut self, command: &str) -> bool {
        match self.init_state {
            InitState::ExpectingChan => {
                if let Ok(mapping) = parse_chan_command(command) {
                    self.init_state = InitState::ExpectingUnits;
                    send_ack("Active channels set to 1200").await;
                    return true;
                }
            }
            InitState::ExpectingUnits => {
                if let Ok(units) = parse_units_command(command) {
                    self.init_state = InitState::ExpectingFilt;
                    // No response for UNITS per spec
                    return true;
                }
            }
            InitState::ExpectingFilt => {
                if let Ok(filters) = parse_filt_command(command) {
                    self.init_state = InitState::Ready;
                    // No response for FILT per spec
                    return true;
                }
            }
            InitState::Ready => {
                // Normal command processing
                return false;
            }
        }
        false
    }
}
```

### Integration Point 4: ArtisanFormatter Streaming

**Location:** `src/output/artisan.rs`

**Current State:**
- `format_read_response()` → `"ET,BT,Power,Fan"` (4 values)
- `format()` → `"time,ET,BT,ROR,Gas"` (5 values, streaming)

**Required Changes:**
1. Support full 5-value READ response when all channels active
2. Add ambient temperature to responses
3. Support Fahrenheit mode conversion
4. Ensure consistent CSV formatting with Artisan expectations

---

## New Components Required

### 1. Artisan Protocol State Machine

**File:** `src/input/artisan_protocol.rs` (new)

```rust
/// Man Artisan protocol initialization and state
pub struct ArtisanProtocolState {
    init_state: InitState,
    channel_mapping: (u8, u8, u8, u8),  // Physical → Logical mapping
    units: TemperatureUnits,
    filters: [u8; 4],
    pid_state: PidState,
}

enum InitState {
    ExpectingChan,     // Must receive CHAN first
    ExpectingUnits,   // Then UNITS
    ExpectingFilt,    // Then FILT
    Ready,            // Normal operation
}

enum PidState {
    Disabled,
    Enabled { sv: f32, channel: u8 },
}
```

### 2. Enhanced Response Builder

**File:** `src/output/artisan_response.rs` (new)

```rust
/// Builds Artisan protocol responses
pub struct ArtisanResponseBuilder {
    buffer: String<128>,
}

impl ArtisanResponseBuilder {
    pub fn new() -> Self { ... }
    
    pub fn ack(&mut self, message: &str) -> &mut Self { ... }
    
    pub fn temperatures(&mut self, ambient: f32, ch1: f32, ch2: f32, ch3: f32, ch4: f32) -> &mut Self { ... }
    
    pub fn build(self) -> String<128> { ... }
}
```

### 3. Extended Parser Module

**File:** `src/input/artisan_parser.rs` (new, replaces/extends parser.rs)

```rust
/// Parse Artisan commands with full protocol support
pub struct ArtisanParser;

impl ArtisanParser {
    pub fn parse(command: &str) -> Result<ParsedCommand, ParseError> { ... }
    
    fn split_params(input: &str) -> Vec<&str> { ... }
    
    fn parse_chan(params: &[&str]) -> Result<ChanMapping, ParseError> { ... }
    
    fn parse_pid(command: &str, params: &[&str]) -> Result<ArtisanCommand, ParseError> { ... }
}
```

### 4. PID Command Handler Extension

**Location:** Extend `src/control/handlers.rs`

```rust
impl ArtisanCommandHandler {
    // Existing methods...
    
    pub fn set_pid_sv(&mut self, sv: f32) {
        self.pid_sv = Some(sv);
    }
    
    pub fn get_pid_sv(&self) -> Option<f32> {
        self.pid_sv
    }
    
    pub fn set_pid_tuning(&mut self, kp: f32, ki: f32, kd: f32) {
        self.pid_tuning = Some((kp, ki, kd));
    }
}
```

---

## Data Flow Analysis

### Current Data Flow (READ Command)

```
Artisan Host
     │
     │ Serial: "READ\n"
     ▼
┌─────────────────────────────────────────────────────────────────────┐
│ uart_reader_task ( Embassy async task )                              │
│ - Reads 64-byte chunks from UART0                                    │
│ - Accumulates in CircularBuffer                                      │
│ - Splits on '\n' delimiter                                          │
│ - Calls process_command_data()                                       │
└─────────────────────────────────────────────────────────────────────┘
     │
     │ Complete line: "READ"
     ▼
┌─────────────────────────────────────────────────────────────────────┐
│ process_command_data()                                               │
│ - Parses "READ" → ArtisanCommand::ReadStatus                        │
│ - Checks multiplexer: should_process_command(CommChannel::Uart)   │
│ - Sends to ARTISAN_CMD_CHANNEL                                       │
└─────────────────────────────────────────────────────────────────────┘
     │
     │ Channel send: ReadStatus
     ▼
┌─────────────────────────────────────────────────────────────────────┐
│ control_loop_task                                                    │
│ - Receives from ARTISAN_CMD_CHANNEL                                  │
│ - Calls roaster.process_artisan_command(ReadStatus)                  │
│ - Gets status: bean_temp, env_temp, ssr_output, fan_output          │
│ - Calls ArtisanFormatter::format_read_response()                      │
│ - Sends formatted response to ARTISAN_OUTPUT_CHANNEL                  │
└─────────────────────────────────────────────────────────────────────┘
     │
     │ Channel send: "120.3,150.5,75.0,25.0"
     ▼
┌─────────────────────────────────────────────────────────────────────┐
│ dual_output_task                                                     │
│ - Receives from ARTISAN_OUTPUT_CHANNEL                              │
│ - Gets active channel from multiplexer                               │
│ - Appends "\r\n"                                                    │
│ - Routes to UART0 or USB CDC based on active channel                │
└─────────────────────────────────────────────────────────────────────┘
     │
     │ Serial: "120.3,150.5,75.0,25.0\r\n"
     ▼
Artisan Host (displays temperatures)
```

### Enhanced Data Flow (Full Protocol)

```
┌─────────────────────────────────────────────────────────────────────┐
│ Initialization Sequence (first connection)                           │
│                                                                      │
│ Artisan: "CHAN;1200\n"                                             │
│ → ArtisanProtocolState::handle_init()                                │
│ → Responds: "# Active channels set to 1200"                          │
│                                                                      │
│ Artisan: "UNITS;C\n"                                                │
│ → Sets units to Celsius                                              │
│ → No response (per spec)                                             │
│                                                                      │
│ Artisan: "FILT;10,10,10,10\n"                                      │
│ → Sets filters                                                       │
│ → No response (per spec)                                             │
│                                                                      │
│ Artisan: "READ\n"                                                    │
│ → Normal command processing begins                                    │
└─────────────────────────────────────────────────────────────────────┘
     │
     ▼
┌─────────────────────────────────────────────────────────────────────┐
│ PID Control Flow                                                     │
│                                                                      │
│ Artisan: "PID;ON\n"                                                 │
│ → ArtisanCommand::PidOn                                             │
│ → RoasterControl::enable_pid()                                       │
│ → Responds: "# PID enabled"                                          │
│                                                                      │
│ Artisan: "PID;SV;185.5\n"                                           │
│ → ArtisanCommand::PidSv(185.5)                                      │
│ → TemperatureCommandHandler::set_pid_target(185.5)                  │
│ → Responds: "# PID setpoint: 185.5"                                 │
│                                                                      │
│ Artisan: "OT1;75\n"                                                 │
│ → ArtisanCommand::SetHeater(75)                                      │
│ → Immediate heater control (bypasses PID)                            │
│ → Responds: "# OT1: 75%"                                            │
└─────────────────────────────────────────────────────────────────────┘
     │
     ▼
┌─────────────────────────────────────────────────────────────────────┐
│ Streaming Mode (during roast)                                        │
│                                                                      │
│ Artisan: "START\n"                                                  │
│ → Enables continuous output                                          │
│ → dual_output_task begins sending CSV lines:                         │
│    "time,ET,BT,ROR,Gas\r\n" at 10Hz                                 │
│                                                                      │
│ Artisan: Can interject commands during streaming                      │
│ - OT1/IO3: Immediate control updates                                │
│ - PID;SV: Dynamic setpoint changes                                   │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Suggested Build Order

### Phase 1: Protocol Foundation (Week 1)

**Goal:** Support initialization sequence and acknowledgment responses

**Tasks:**
1. Create `src/input/artisan_protocol.rs` with state machine
2. Implement CHAN → UNITS → FILT initialization sequence
3. Add acknowledgment response system (`#` prefix)
4. Modify `process_command_data()` to route initialization commands
5. Add error response format (`ERR code message`)

**Dependencies:** None (new module)  
**Testing:** Manual testing with serial terminal

### Phase 2: Extended Command Parsing (Week 2)

**Goal:** Full Artisan command support

**Tasks:**
1. Create `src/input/artisan_parser.rs` with enhanced delimiter handling
2. Add PID command parsing (PID;ON, PID;SV;xxx, PID;T;...)
3. Add OT2 command support
4. Add DCFAN command support
5. Extend `ArtisanCommand` enum with new variants
6. Add temperature units (C/F) handling in ArtisanFormatter

**Dependencies:** Phase 1 complete  
**Testing:** Unit tests for all command variants

### Phase 3: Response Enhancement (Week 3)

**Goal:** Complete response handling

**Tasks:**
1. Create `src/output/artisan_response.rs` response builder
2. Implement ambient temperature in READ responses
3. Add PID status responses
4. Enhance dual_output_task for acknowledgment routing
5. Add response queuing for slow operations

**Dependencies:** Phase 2 complete  
**Testing:** Integration tests with Artisan software

### Phase 4: PID Integration (Week 4)

**Goal:** Full PID control via Artisan

**Tasks:**
1. Extend `ArtisanCommandHandler` with PID state
2. Implement `PidOn`/`PidOff` handlers
3. Implement `PidSv` setpoint updates
4. Add PID tuning parameter support
5. Integrate with existing PID controller in TemperatureCommandHandler
6. Add PID status reporting in READ responses

**Dependencies:** Phase 3 complete  
**Testing:** Closed-loop PID control testing

### Phase 5: Streaming Optimization (Week 5)

**Goal:** Robust streaming performance

**Tasks:**
1. Optimize CSV formatting for 10Hz streaming
2. Add streaming buffer management
3. Implement backpressure handling
4. Add streaming start/stop acknowledgment
5. Test with Artisan's "Control" mode active

**Dependencies:** Phase 4 complete  
**Testing:** Long-duration streaming tests

---

## Architecture Recommendations

### 1. Keep Command Multiplexer Architecture

The existing `CommandMultiplexer` with 60-second timeout is appropriate for handling both UART and USB CDC channels. The architecture correctly routes responses to the active channel.

**Recommendation:** Extend, don't replace. Add `init_state` tracking to multiplexer or create separate `ArtisanProtocolState` that works alongside it.

### 2. Separate Parser from Protocol State

Current architecture separates:
- **Parser** (`parser.rs`): Syntax → ArtisanCommand
- **Handler** (`handlers.rs`): ArtisanCommand → State change

**Recommendation:** Maintain this separation. New protocol initialization commands should follow the same pattern—parse to a command, then route to appropriate handler.

### 3. Response Building Strategy

Current: Direct string building in multiple locations  
**Recommendation:** Centralize response building in `ArtisanResponseBuilder` to ensure consistent formatting and channel awareness.

### 4. Error Handling Strategy

Current: Basic ParseError variants  
**Recommendation:** Add Artisan-specific error codes for responses:
- `ERR unknown_command "CMD"`
- `ERR invalid_value "Value out of range"`
- `ERR not_initialized "Send CHAN command first"`

### 5. Backward Compatibility

Current implementation already supports basic Artisan usage (READ, OT1, IO3). New features should not break this.

**Recommendation:** Feature flags for extended commands, or graceful unknown command handling that doesn't crash.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Protocol Specification | HIGH | TC4 protocol well-documented, verified against Artisan source |
| Existing Architecture | HIGH | Codebase fully analyzed, patterns understood |
| Integration Points | HIGH | Clear boundaries identified |
| Build Order | MEDIUM | Sequential dependencies logical, may need adjustment based on testing |
| PID Integration | MEDIUM | Requires existing PID implementation details |

---

## Gaps to Address in Later Phases

1. **Fahrenheit Support**: Temperature conversion logic needs verification
2. **Multi-channel Support**: Currently only ET/BT, TC3/TC4 unused
3. **Filter Settings**: Digital filtering implementation unclear
4. **PID Tuning UI**: Artisan's PID dialog requires extended response set
5. **Performance Testing**: Real-world streaming performance at 10Hz

---

## Sources

- **TC4 Protocol Specification**: [greencardigan/TC4-shield GitHub](https://github.com/greencardigan/TC4-shield)
- **Artisan Documentation**: [artisan-scope.org/devices/arduino](https://artisan-scope.org/devices/arduino/)
- **Artisan Discussions**: [GitHub artisan-roaster-scope/artisan](https://github.com/artisan-roaster-scope/artisan/discussions/866)
- **Existing Implementation**: LibreRoaster codebase analysis
