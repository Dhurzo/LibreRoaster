# Stack Research: Artisan Command Parsing Additions

**Domain:** Artisan Protocol Command Parsing - ESP32-C3 Embedded Rust
**Researched:** 2026-02-07
**Milestone:** Adding OT2 (fan), READ (telemetry), and UNITS (temperature scale) commands
**Confidence:** HIGH

## Executive Summary

**No new stack additions required.** The existing LibreRoaster architecture using `embassy-rs`, `esp-hal`, and `heapless` is fully capable of implementing the required Artisan commands (OT2, READ, UNITS). The existing parser, ArtisanCommand enum, and ArtisanFormatter provide all necessary infrastructure. The required work is purely implementation (adding OT2 command pattern), not stack expansion.

## Recommended Stack Changes

### Zero Stack Additions

All capabilities for OT2, READ, and UNITS commands exist in the current stack:

| Category | Current Stack | Status |
|----------|---------------|--------|
| Async Runtime | `embassy-executor` 0.9.1 | ✅ Sufficient |
| Hardware Access | `esp-hal` ~1.0 | ✅ Sufficient |
| Command Parsing | `heapless` 0.8.0 | ✅ Sufficient |
| Serial Communication | USB CDC + UART0 | ✅ Working |
| Logging | `log` 0.4.27 | ✅ Sufficient |

### No Cargo.toml Changes Required

```toml
# Existing dependencies already cover all needs
[dependencies]
heapless = "0.8.0"           # Command parsing with heapless::Vec
embassy-executor = "0.9.1"   # Async task scheduling
esp-hal = "~1.0"            # Peripheral access
log = "0.4.27"              # Logging
```

## Artisan Protocol Requirements

### Command Reference

Based on TC4-shield Artisan protocol specification:

| Command | Syntax | Purpose | Existing |
|---------|--------|---------|----------|
| **OT2** | `OT2,duty` (0-100) | Fan speed with rate limiting | ❌ **NEW** |
| **IO3** | `IO3,duty` (0-100) | Fan speed immediate | ✅ Existing |
| **READ** | `READ\n` | Telemetry request | ✅ Existing |
| **UNITS** | `UNITS;C` or `UNITS;F` | Temperature scale | ✅ Existing |

### OT2 vs IO3 Semantics

The Artisan protocol distinguishes fan commands:

**OT2 Command:**
- Format: `OT2,duty` where duty = 0 to 100
- Behavior: Rate-limited increase (25 points/sec max)
- Use case: Prevents fan inrush current issues
- No response sent to host

**IO3 Command:**
- Format: `IO3,duty` where duty = 0 to 100
- Behavior: Instantaneous change
- Use case: Legacy compatibility, immediate response needed
- No response sent to host

**LibreRoaster Implementation Decision:** OT2 should be the primary command with rate limiting. IO3 should work as an alias for backward compatibility.

## Implementation Changes Required

### 1. Parser Extension (`src/input/parser.rs`)

Add OT2 command pattern to existing match arms:

```rust
// Add to the whitespace-delimited command matching:
["OT2", value_str] => {
    let value = parse_percentage(value_str)?;
    Ok(ArtisanCommand::SetFanRateLimited(value))
}
```

**Status:** Minimal change - follows existing OT1/IO3 pattern exactly.

### 2. ArtisanCommand Enum (`src/config/constants.rs`)

Current enum structure supports this use case:

```rust
pub enum ArtisanCommand {
    ReadStatus,
    StartRoast,
    SetHeater(u8),    // OT1
    SetFan(u8),        // IO3
    EmergencyStop,
    IncreaseHeater,
    DecreaseHeater,
    Chan(u16),
    Units(bool),
    Filt(u8),
}
```

**Decision:** Add new variant for rate-limited fan:

```rust
pub enum ArtisanCommand {
    // ... existing ...
    SetFanRateLimited(u8),  // NEW: OT2 with rate limiting
}
```

**Rationale:** Keeps command dispatch explicit. Handler can decide behavior based on variant.

### 3. READ Response Verification (`src/output/artisan.rs`)

Current ArtisanFormatter already implements READ response:

```rust
pub fn format_read_response(status: &SystemStatus, fan_speed: f32) -> String {
    format!(
        "{:.1},{:.1},{:.1},{:.1}",
        status.env_temp,   // ET
        status.bean_temp,  // BT
        status.ssr_output, // Heater
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
```

**Status:** ✅ Complete. Matches Artisan protocol requirement: `ambient,chan1,chan2,chan3,chan4`

### 4. UNITS Temperature Conversion (MISSING)

Current parser accepts UNITS commands but no temperature conversion exists:

```rust
// Parser already handles this:
"UNITS" => match args.trim() {
    "C" | "c" => Ok(ArtisanCommand::Units(false)),
    "F" | "f" => Ok(ArtisanCommand::Units(true)),
    _ => Err(ParseError::InvalidValue),
},
```

**Missing:** Temperature state management and conversion.

**Required Addition:**

```rust
// Add to SystemStatus:
pub struct SystemStatus {
    // ... existing fields ...
    pub use_fahrenheit: bool,  // Track temperature scale
}

// Add conversion helper:
impl SystemStatus {
    pub fn formatted_temp(&self, celsius: f32) -> String {
        let temp = if self.use_fahrenheit {
            (celsius * 9.0 / 5.0) + 32.0
        } else {
            celsius
        };
        format!("{:.1}", temp)
    }
}
```

## Architecture Integration

### Command Flow

```
Serial Input → Multiplexer → Parser → ArtisanCommand → Handler → Hardware
                                                              ↓
                                                        Fan/Heater PWM
```

### Fan Control Abstraction

Existing `fan_host.rs` or `hardware/fan.rs` should be extended:

```rust
// Rate-limited fan control for OT2
pub async fn set_fan_rate_limited(target: u8) {
    let current = get_fan_speed();
    let step = 25;  // Max increase per second (Artisan standard)
    
    if target > current {
        for speed in (current + step..=target).step_by(step as usize) {
            set_fan_speed(speed);
            embassy_time::Timer::after_secs(1).await;
        }
        // Ensure exact target
        set_fan_speed(target);
    } else {
        set_fan_speed(target);  // Immediate decrease is safe
    }
}

// Immediate fan control for IO3
pub fn set_fan_immediate(target: u8) {
    set_fan_speed(target);
}
```

## What NOT to Add

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| New parsing crate | `heapless::Vec` pattern already works | Extend existing parser |
| Temperature conversion crate | C↔F is simple math | Manual conversion |
| Separate OT2 handler | Existing handlers.rs handles commands | Extend handlers.rs |
| Custom serial layer | USB CDC + UART0 working | Reuse multiplexor |
| Additional async runtime | embassy-executor sufficient | Add tasks, not runtimes |

## Feature Dependencies

```
OT2 Command
├── Parser: Add OT2 pattern (1 function, ~5 lines)
├── Command Enum: Add SetFanRateLimited variant (1 line)
├── Handler: Add rate-limited fan control (~15 lines)
└── Hardware: May need PWM rate limiting (varies by fan)

READ Command
├── Parser: Already implemented (0 changes)
├── Formatter: Already implemented (0 changes)
└── Channel Config: Uses existing SystemStatus (0 changes)

UNITS Command  
├── Parser: Already implemented (0 changes)
├── Command Enum: Already has Units(bool) (0 changes)
└── Temperature State: ADD use_fahrenheit field (~3 lines)
    └── Conversion Helper: ADD formatted_temp method (~8 lines)
```

## MVP Scope

For initial milestone completion, implement only:

1. **OT2 parser pattern** (2 lines added to parser.rs)
2. **SetFanRateLimited variant** (1 line added to enum)
3. **Rate-limited fan handler** (async function, ~15 lines)
4. **Temperature state field** (1 field added to SystemStatus)

Defer:
- IO3 alias support (backward compatibility only)
- Temperature conversion display formatting
- Rate limiting configuration (hardcode 25 points/sec)

## Sources

- **TC4-shield Artisan Protocol**: https://github.com/greencardigan/TC4-shield/blob/master/applications/Artisan/aArtisan/trunk/src/aArtisan/commands.txt
  - Official command specification (OT2, IO3, READ, UNITS formats)
  - Rate limiting requirements (25 points/sec for DCFAN/OT2)
  - Channel configuration via CHAN command

- **Existing LibreRoaster Implementation**:
  - `src/input/parser.rs` (v1.0+) - Command parsing patterns
  - `src/output/artisan.rs` (v1.0) - ArtisanFormatter READ response
  - `src/config/constants.rs` - ArtisanCommand enum definition

- **Artisan Official Documentation**: https://artisan-scope.org/docs/setup/
  - Machine configuration guidelines
  - Supported devices and protocols

---

*Stack research for: Artisan command parsing additions (OT2, READ, UNITS)*
*Researched: 2026-02-07*
