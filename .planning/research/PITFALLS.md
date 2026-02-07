# Domain Pitfalls: Artisan Command Parsing for ESP32-C3

**Project:** LibreRoaster - ESP32-C3 Coffee Roaster Firmware
**Research Date:** February 7, 2026
**Domain:** Artisan Protocol Implementation (OT2, READ, UNITS commands)
**Confidence Level:** MEDIUM - Based on community forum discussions, GitHub issues, and firmware documentation analysis

---

## Executive Summary

This document catalogs common pitfalls encountered when adding OT2 (fan control), READ (telemetry), and UNITS (temperature scale) commands to an existing ESP32-C3 Artisan protocol implementation. The research draws from community experiences with TC4-shield implementations, Artisan software discussions, and ESP32-C3 serial communication documentation. These pitfalls span command parsing, serial timing, temperature conversion accuracy, fan control smoothness, and state management.

---

## Critical Pitfalls

### Pitfall 1: OT2 Fan Command Parsing Edge Cases

**What Goes Wrong:**
Fan speed commands fail to parse correctly when receiving unexpected input formats, causing the fan to remain at previous speed or default to 0%.

**Why It Happens:**
- Artisan sends OT2 commands with various delimiter formats (comma, space, semicolon, equals sign) that must all be handled
- The first five characters of the command name are significant per Artisan spec, but implementations often assume exact matching
- Fan speed values may arrive as integers (0-100) or floating point percentages requiring normalization

**Consequences:**
- Fan unresponsive to Artisan control commands
- Unexpected fan behavior during roast profiles
- Potential safety hazard if fan fails during high-temperature phases

**Prevention:**
```rust
// Parse OT2 command with multiple delimiter support
fn parse_ot2_command(input: &str) -> Result<u8, ParseError> {
    // Normalize command by removing whitespace and handling delimiters
    let normalized = input.trim();
    
    // Split on any supported delimiter
    let parts: Vec<&str> = normalized
        .split(|c| c == ',' || c == ' ' || c == ';' || c == '=')
        .collect();
    
    // Extract and validate fan speed value
    if parts.len() < 2 {
        return Err(ParseError::InvalidFormat);
    }
    
    let speed_str = parts[1].trim();
    let speed: u8 = speed_str
        .parse()
        .map_err(|_| ParseError::InvalidValue)?;
    
    // Clamp to valid range
    Ok(speed.clamp(0, 100))
}
```

**Detection:**
- Monitor parsed command success/failure rates during Artisan connection
- Log malformed commands for debugging
- Test with Artisan's command logger to verify format expectations

**Phase to Address:** **Command Implementation Phase** - Add comprehensive parsing with error recovery

---

### Pitfall 2: READ Command Response Format Mismatch

**What Goes Wrong:**
Artisan fails to recognize temperature data due to incorrect response formatting, showing "uu" or stale readings.

**Why It Happens:**
- Response format must exactly match: `READ,ET,BT,FAN,HEATER` with values separated by commas
- Missing or extra delimiters cause parsing failures
- Line ending expectations vary between Artisan versions (\r\n vs \n)
- Response rate must align with Artisan's sampling interval

**Consequences:**
- Artisan shows "uu" (unavailable) for temperature channels
- Temperature graphs fail to update
- Roast profile data collection stops

**Prevention:**
```rust
// Verify READ response format matches Artisan expectations
fn format_read_response(et: f32, bt: f32, fan: u8, heater: u8) -> String {
    format!("READ,{},{},{},{}\r\n", et, bt, fan, heater)
}

// Ensure consistent decimal precision (typically 1 decimal place)
fn format_temperature(temp: f32) -> String {
    format!("{:.1}", temp)
}
```

**Critical Note:** The TC4-shield reference implementations show Artisan expects exactly this format. Deviation causes connection failures.

**Detection:**
- Capture serial traffic during Artisan connection to verify response format
- Compare against known-working implementations
- Monitor Artisan's connection status and channel indicators

**Phase to Address:** **Telemetry Implementation Phase** - Validate response format with Artisan during integration testing

---

### Pitfall 3: Temperature Unit Conversion Accuracy

**What Goes Wrong:**
UNITS command changes temperature scale but internal calculations or historical data don't convert correctly, causing profile inconsistencies.

**Why It Happens:**
- Converting between Celsius and Fahrenheit requires precise formula: F = (C × 9/5) + 32
- Rounding errors compound over multiple conversions
- Historical roast data may not convert properly when switching units mid-session
- Rate of rise (ROR) calculations fail when units change

**Consequences:**
- Inconsistent temperature readings between sessions
- Incorrect first crack or development time references
- Profile comparisons become meaningless across unit changes

**Prevention:**
```rust
// Use precise conversion with controlled rounding
fn celsius_to_fahrenheit(c: f32) -> f32 {
    (c * 9.0 / 5.0) + 32.0
}

fn fahrenheit_to_celsius(f: f32) -> f32 {
    (f - 32.0) * 5.0 / 9.0
}

// Store all temperatures internally in a canonical unit (Celsius)
// Convert only when responding to READ commands based on current UNITS state
struct TemperatureState {
    canonical_temp: f32,  // Always stored in Celsius
    current_units: Units,  // Celsius or Fahrenheit
}

impl TemperatureState {
    fn get_reading(&self) -> f32 {
        match self.current_units {
            Units::Celsius => self.canonical_temp,
            Units::Fahrenheit => celsius_to_fahrenheit(self.canonical_temp),
        }
    }
}
```

**Critical Note:** Community discussions reveal Fahrenheit conversion bugs are among the most reported Artisan integration issues, particularly with Skywalker roasters.

**Phase to Address:** **UNITS Command Phase** - Implement canonical storage and consistent conversion

---

### Pitfall 4: ESP32-C3 UART Serial Timing Issues

**What Goes Wrong:**
Serial communication becomes unreliable at higher baud rates or under load, causing dropped commands and corrupted responses.

**Why It Happens:**
- ESP32-C3 has documented UART frame error issues at higher baud rates
- USB CDC serial on ESP32-C3 can stop responding under high traffic
- Artisan polls at specific intervals that may conflict with firmware processing timing
- Asynchronous embassy-rs framework may produce responses faster than Artisan expects

**Consequences:**
- Intermittent Artisan connection failures
- Data corruption in temperature readings
- Fan/heater commands execute incorrectly or not at all

**Prevention:**
```rust
// Use conservative baud rate (115200 is standard for Artisan)
let uart_config = Config::new()
    .baud_rate(Parity::NONE, BaudRate::Baud115200)
    .stop_bits(StopBits::STOP1)
    .data_bits(DataBits::Data8);

// Implement response timing to match Artisan expectations
// Artisan typically expects responses within 50-100ms of command receipt

// Add small delay if needed to prevent overwhelming Artisan
// WARNING: Excessive delays cause timeout failures
fn response_delay() {
    embassy_time::Timer::after_millis(10).await;
}
```

**Critical Note:** ESP32-C3 GitHub issues document USB CDC serial freezing under sustained high-traffic scenarios common during active roasting.

**Phase to Address:** **Serial Communication Phase** - Test under realistic roast load conditions

---

### Pitfall 5: Fan Speed Control Smoothness

**What Goes Wrong:**
Fan speed changes abruptly, causing airflow fluctuations that affect roast development and temperature stability.

**Why It Happens:**
- Direct PWM changes without ramp-up/down cause mechanical stress
- Fan doesn't respond linearly to PWM values
- Large step changes in fan speed affect bean agitation unexpectedly
- Artisan may send rapid successive fan commands during profile execution

**Consequences:**
- Inconsistent roast profiles
- Mechanical wear on fan assembly
- Temperature instability during critical roast phases

**Prevention:**
```rust
// Implement fan speed ramping for smooth transitions
const FAN_RAMP_STEP: u8 = 5;  // Maximum step per update
const FAN_RAMP_INTERVAL_MS: u64 = 100;  // Update interval

async fn set_fan_speed_ramped(target: u8, current: &mut u8) {
    while *current != target {
        let step = if target > *current {
            (target - *current).min(FAN_RAMP_STEP)
        } else {
            (*current - target).min(FAN_RAMP_STEP)
        };
        *current += step;
        set_fan_pwm(*current);
        Timer::after_millis(FAN_RAMP_INTERVAL_MS).await;
    }
}

// Alternative: Use Artisan's built-in fan ramp if available
// Configure fan slider to use gradual transitions in Artisan settings
```

**Phase to Address:** **Fan Control Phase** - Implement smooth transitions before integration testing

---

### Pitfall 6: OT1/OT2 Channel Multiplexing Conflicts

**What Goes Wrong:**
OT1 (heater) and OT2 (fan) commands interfere with each other, or Artisan commands target wrong output channel.

**Why It Happens:**
- Both use similar command structures but different channels
- Artisan configuration may map channels incorrectly
- Internal state machine conflicts when both outputs active simultaneously
- PWM channels may share hardware resources on ESP32-C3

**Consequences:**
- Heater responds to fan commands or vice versa
- Both outputs activate simultaneously causing thermal runaway
- Artisan shows incorrect feedback for channel states

**Prevention:**
```rust
// Explicit channel separation in command handler
enum OutputChannel {
    Heater,  // OT1
    Fan,     // OT2
}

struct OutputControl {
    heater_pwm: u8,
    fan_pwm: u8,
}

impl OutputControl {
    fn handle_ot_command(channel: OutputChannel, value: u8) {
        match channel {
            OutputChannel::Heater => {
                // Validate heater limits
                let safe_value = value.clamp(0, 100);
                self.heater_pwm = safe_value;
                self.update_heater_pwm();
            }
            OutputChannel::Fan => {
                let safe_value = value.clamp(0, 100);
                self.fan_pwm = safe_value;
                self.update_fan_pwm();
            }
        }
    }
}
```

**Phase to Address:** **Channel Integration Phase** - Verify channel isolation before testing with Artisan

---

### Pitfall 7: Command State Machine Conflicts

**What Goes Wrong:**
Processing one Artisan command while another is in progress causes state corruption or missed commands.

**Why It Happens:**
- Artisan sends commands in rapid succession during profile execution
- Asynchronous embassy-rs framework may interleave command processing
- Long-running operations (like fan ramping) block command responses
- Buffer overflow from accumulated commands during high-activity periods

**Consequences:**
- Commands execute out of order
- Some commands silently fail
- Firmware becomes unresponsive

**Prevention:**
```rust
// Use command queue with atomic processing
use embassy_sync::pipe::Pipe;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

static COMMAND_PIPE: Pipe<CriticalSectionRawMutex, 32, 256> = Pipe::new();

async fn process_commands() {
    loop {
        if let Some(cmd) = COMMAND_PIPE.read().await {
            match cmd {
                Command::OT1(value) => handle_ot1(value).await,
                Command::OT2(value) => handle_ot2(value).await,
                Command::READ => handle_read().await,
                Command::UNITS(unit) => handle_units(unit).await,
            }
            // Signal Artisan we're ready (if using sync protocol)
        }
    }
}

// Ensure each handler completes quickly (< 50ms preferred)
async fn handle_ot2(value: u8) {
    // Don't block here - queue fan ramp as separate task
    spawn_fan_ramp(value).await;
}
```

**Phase to Address:** **Command Processing Phase** - Implement proper concurrency model

---

## Moderate Pitfalls

### Pitfall 8: Temperature Sensor Reading Consistency

**What Goes Wrong:**
BT (Bean Temperature) and ET (Environmental Temperature) readings fluctuate wildly or show impossible values.

**Why It Happens:**
- Thermocouple cold-junction compensation errors
- ADC noise on ESP32-C3 analog inputs
- Thermocouple probe degradation over time
- Electrical noise from heater PWM affecting sensor readings

**Prevention:**
```rust
// Implement rolling average filter
const FILTER_WINDOW: usize = 8;

struct TemperatureFilter {
    readings: [f32; FILTER_WINDOW],
    index: usize,
}

impl TemperatureFilter {
    fn add_reading(&mut self, raw: f32) -> f32 {
        self.readings[self.index] = raw;
        self.index = (self.index + 1) % FILTER_WINDOW;
        
        // Calculate average
        self.readings.iter().sum::<f32>() / FILTER_WINDOW as f32
    }
}

// Add sensor validation
fn validate_temperature_reading(temp: f32) -> Option<f32> {
    match temp {
        t if t < -50.0 => None,  // Impossible cold
        t if t > 500.0 => None,  // Impossible hot for coffee roasting
        t if t.is_nan() => None,
        t if t.is_infinite() => None,
        _ => Some(temp),
    }
}
```

**Phase to Address:** **Sensor Integration Phase** - Before telemetry implementation

---

### Pitfall 9: UNITS State Persistence

**What Goes Wrong:**
Temperature units reset to default after power cycle or Artisan reconnection, causing confusion.

**Why It Happens:**
- UNITS setting stored only in RAM, not persistent storage
- Artisan may reset unit state on reconnection
- Multiple Artisan instances with different unit preferences

**Prevention:**
```rust
// Store in NVS for persistence across power cycles
fn save_units_setting(units: Units) {
    // embassy_nvs::Nvs::set(...)
}

fn load_units_setting() -> Units {
    // embassy_nvs::Nvs::get(...)
        // Default to Celsius if not set
}
```

**Phase to Address:** **Configuration Phase** - Implement persistent settings

---

### Pitfall 10: Command Response Timing Expectations

**What Goes Wrong:**
Artisan times out waiting for command responses, showing communication errors.

**Why It Happens:**
- Artisan has strict timeout expectations (typically 2-5 seconds for most commands)
- Slow sensor readings delay READ command responses
- Fan ramp functions block command handler
- Debug logging slows response times

**Prevention:**
```rust
// Keep command handlers fast
// Target < 100ms response time for all commands

async fn handle_read() {
    // Quick sensor reads only
    // Defer processing to separate task if needed
}

// Monitor response times in production
fn check_response_time() {
    let elapsed = start.elapsed();
    if elapsed > Duration::from_millis(2000) {
        // Log warning - approaching Artisan timeout
    }
}
```

**Phase to Address:** **Performance Phase** - Optimize before integration testing

---

## Minor Pitfalls

### Pitfall 11: Delimiter Handling Inconsistencies

**What Goes Wrong:**
Commands work with some delimiter formats but fail with others, causing intermittent issues.

**Prevention:**
- Test all delimiter variations during development
- Document supported formats for debugging

### Pitfall 12: Error Response Format

**What Goes Wrong:**
Returning errors in wrong format causes Artisan to hang or crash.

**Prevention:**
- Return "ERROR" or "BAD" followed by explanation for invalid commands
- Always end error responses with newline

### Pitfall 13: Float Formatting Variations

**What Goes Wrong:**
Temperature values formatted differently between implementations cause parsing issues.

**Prevention:**
- Use consistent decimal places (1 decimal is standard)
- Avoid scientific notation
- Handle negative temperatures correctly

---

## Phase-Specific Warnings

| Phase | Likely Pitfall | Mitigation |
|-------|---------------|------------|
| **OT2 Command Implementation** | PWM hardware conflict with OT1 | Verify GPIO pin assignments before implementation |
| **READ Command Implementation** | Response format mismatch | Capture traffic from working Artisan setup for comparison |
| **UNITS Command Implementation** | Conversion accuracy | Use canonical storage, test round-trip conversions |
| **Serial Communication** | UART timing issues | Test at realistic baud rates with Artisan load |
| **Integration Testing** | State machine conflicts | Use command queue, test rapid command sequences |

---

## Testing Recommendations

Before completing each phase, test these scenarios:

### OT2 Testing:
- [ ] Commands with all delimiter formats
- [ ] Out-of-range values (negative, >100)
- [ ] Rapid successive commands
- [ ] Fan ramp behavior during transitions

### READ Testing:
- [ ] Response format validation
- [ ] Temperature sensor failure scenarios
- [ ] Response timing under load
- [ ] Artisan reconnection handling

### UNITS Testing:
- [ ] Round-trip conversion accuracy
- [ ] Mid-roast unit changes
- [ ] Historical data conversion
- [ ] Persistence across power cycles

---

## Sources

**Confidence Assessment:**

| Source Type | Confidence Level | Notes |
|-------------|------------------|-------|
| TC4-shield GitHub Repository | HIGH | Reference implementation for Artisan protocol |
| Artisan Scope Documentation | HIGH | Official software documentation |
| ESP32-C3 GitHub Issues | MEDIUM | UART timing documented but may be chip-specific |
| Homeroasters Community Forum | MEDIUM | User experiences with various implementations |
| Skywalker Roaster Firmware | MEDIUM | Similar hardware, common pitfalls documented |

**Key References:**
- TC4-shield commands.txt specification: https://github.com/greencardigan/TC4-shield
- Artisan official documentation: https://artisan-scope.org/docs/
- ESP32-C3 UART troubleshooting: https://docs.espressif.com/projects/esp-idf/en/latest/esp32c3/

---

## Research Gaps

The following areas could not be fully verified and may require phase-specific research:

1. **embassy-rs specific UART behaviors** - Limited documentation on interaction with Artisan protocol
2. **Exact Artisan timeout values** - Varies by version, need to verify with target version
3. **OT2 PWM frequency requirements** - May vary by fan type, hardware-specific
4. **USB CDC vs UART selection impact** - Both modes documented with different tradeoffs

**Recommendation:** Validate these gaps during the actual implementation phase with live testing against the target Artisan version.
