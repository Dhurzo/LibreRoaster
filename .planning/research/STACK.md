# Technology Stack: Artisan Serial Protocol Implementation

**Project:** LibreRoaster v1.5
**Researched:** 2026-02-04
**Focus:** Full Artisan serial protocol for ESP32-C3 firmware

---

## Recommended Stack Additions

### Parser Library: `nom` 8.x

| Property | Value |
|----------|-------|
| Version | 8.0.0 |
| Purpose | Robust parser combinator for serial command parsing |
| Why | Industry-standard Rust parsing library, excellent no_std support, zero-copy parsing |

**Rationale:**
The Artisan protocol uses semicolon-delimited commands with structured responses. A parser combinator approach provides:

1. **Compositional parsing** - Build complex command parsers from simple primitives (tag, separated_list, etc.)
2. **Error reporting** - Precise error locations when parsing fails (critical for debugging serial issues)
3. **Zero-copy parsing** - nom can parse without allocating, important for embedded memory constraints
4. **Battle-tested** - 10k+ stars, used in production embedded systems

**NOT using simple string split:**
While the current implementation uses `split_whitespace()`, the full Artisan protocol requires:
- Semicolon delimiter support (`CHAN;1200`)
- Response acknowledgment patterns (`#`)
- Numeric parsing with error handling
- Structured command responses

**Alternative considered: `heapless::Vec` with manual parsing**
Manual parsing would work but lacks:
- Error location reporting
- Composable parser primitives
- Community verification

```toml
# Cargo.toml addition
[dependencies]
nom = { version = "8.0", default-features = false, features = ["alloc"] }
```

**Feature note:** Use `default-features = false` to disable std dependencies. The `alloc` feature enables heap allocation which we need for command parsing buffers.

---

## No Stack Changes Required

### Existing Stack Components (Already Present)

| Component | Status | Notes |
|-----------|--------|-------|
| `esp-hal` | ✅ Compatible | UART peripheral access via esp-hal |
| `embassy-executor` | ✅ Compatible | Async task scheduling for serial handling |
| `embassy-time` | ✅ Compatible | Timeout handling for 60s command window |
| `heapless` | ✅ Compatible | `heapless::Vec` for fixed-capacity buffers |
| `embedded-hal` | ✅ Compatible | Peripheral trait abstractions |

**Rationale:**
The existing Embassy + esp-hal stack already provides:
- UART TX/RX via `embassy-esp32c3` UART driver
- Async read/write operations
- Timeout infrastructure via `embassy_time::Timer`
- Buffer management via `heapless::Vec`

No additional UART drivers, async runtimes, or peripheral abstractions are needed.

---

## Protocol Implementation Strategy

### 1. Command Parser: `nom`-based Artisan Protocol

**Parser structure:**

```rust
// Core command parsers
named!(parse_read_command, ParseResult<ArtisanCommand>);
named!(parse_ot1_command, ParseResult<ArtisanCommand>);
named!(parse_io3_command, ParseResult<ArtisanCommand>);

// Initialization sequence (required for Artisan to enable control features)
named!(parse_chan_command, ParseResult<ConfigCommand>);
named!(parse_units_command, ParseResult<ConfigCommand>);
named!(parse_filt_command, ParseResult<ConfigCommand>);

// Response formatter (existing ArtisanFormatter)
pub fn format_ack() -> &'static str { "#OK" }
```

**Protocol requirements from research:**

| Command | Direction | Format | Response |
|---------|-----------|--------|----------|
| `CHAN;ABCD` | Artisan→Roaster | Set channel config | `#<any_response>` |
| `UNITS;C` | Artisan→Roaster | Set temperature units | `#<any_response>` |
| `FILT;N` | Artisan→Roaster | Set filter value | `#<any_response>` |
| `READ` | Artisan→Roaster | Request telemetry | `ET,BT,Power,Fan` |
| `OT1;N` | Artisan→Roaster | Set heater % | `#<any_response>` |
| `IO3;N` | Artisan→Roaster | Set fan % | `#<any_response>` |
| `START` | Artisan→Roaster | Start roast | `#<any_response>` |
| `STOP` | Artisan→Roaster | Emergency stop | `#<any_response>` |

**Critical implementation detail:** Artisan requires the initialization sequence (`CHAN` → `UNITS` → `FILT`) to complete before it will send control commands. The `#` prefix response is mandatory.

### 2. Serial I/O: Embassy UART Integration

**Recommended pattern:**

```rust
// Using existing embassy-esp32c3 UART
use esp_hal::peripherals::UART0;
use embassy_esp32c3::uart::{Uart, Config};

pub struct ArtisanSerial {
    uart: Uart<'static, UART0>,
    rx_buffer: heapless::Vec<u8, 256>,
}

impl ArtisanSerial {
    pub fn new(uart: Uart<'static, UART0>) -> Self {
        Self {
            uart,
            rx_buffer: heapless::Vec::new(),
        }
    }

    pub async fn read_command(&mut self) -> Result<ArtisanCommand, ParseError> {
        let mut buf = [0u8; 128];
        let len = self.uart.read(&mut buf).await?;
        let cmd_str = core::str::from_utf8(&buf[..len])?;
        parse_artisan_command(cmd_str)
    }

    pub async fn send_response(&mut self, response: &str) -> Result<(), Error> {
        self.uart.write(response.as_bytes()).await?;
        self.uart.write(b"\n").await?;
        Ok(())
    }
}
```

**Existing infrastructure to leverage:**
- `src/hardware/uart/driver.rs` - UART peripheral access
- `src/input/multiplexer.rs` - Command timeout handling (60s)
- `src/input/parser.rs` - Basic command parsing (extends to nom)

### 3. Response Formatting: Extend Existing ArtisanFormatter

**Already implemented in `src/output/artisan.rs`:**

```rust
// READ response format (existing)
pub fn format_read_response(status: &SystemStatus, fan_speed: f32) -> String {
    format!(
        "{:.1},{:.1},{:.1},{:.1}",
        status.env_temp,   // ET
        status.bean_temp,  // BT
        status.ssr_output, // Power (heater)
        fan_speed          // Fan
    )
}

// Add acknowledgment response
pub fn format_ack() -> &'static str { "#OK" }

// Add error response
pub fn format_error(code: &str) -> String { format!("#ERROR:{}", code) }
```

---

## What NOT to Add

### Unnecessary Dependencies

| Rejected | Reason |
|----------|--------|
| `tokio` | Async runtime incompatible with no_std embassy |
| `async-std` | Async runtime incompatible with no_std embassy |
| `serde` | JSON serialization not needed for Artisan protocol |
| `regex` | Parser combinators superior for structured protocols |
| `csv` | CSV library overkill for simple `ET,BT,Power,Fan` format |
| `embedded-nom` | Not actively maintained; use standard nom |
| Any USB stack | Using UART, not USB serial |

### Features to Avoid

| Anti-pattern | Why |
|--------------|-----|
| Dynamic heap allocation | Use `heapless::Vec` for bounded buffers |
| Blocking I/O | Must use async to avoid blocking the executor |
| Global state | Use dependency injection via `ServiceContainer` |
| String formatting in hot paths | Pre-allocate response buffers |

---

## Integration Points

### With Existing Command Multiplexer

```
src/input/multiplexer.rs:60s_timeout
        ↓
src/input/parser.rs:parse_artisan_command()
        ↓
src/control/handlers.rs:ArtisanCommand handler
        ↓
src/output/artisan.rs:format_read_response()
```

### With Hardware UART

```
src/hardware/uart/driver.rs:embassy UART
        ↓
src/input/parser.rs:receive_command()
        ↓
src/output/artisan.rs:send_response()
```

### With System Status

```
src/config/SystemStatus
        ├── env_temp (ET)
        ├── bean_temp (BT)
        ├── ssr_output (heater %)
        └── fan_output (fan %)
              ↓
src/output/artisan.rs:format_read_response()
```

---

## Installation

```bash
# No additional toolchain requirements
# Existing ESP32-C3 toolchain supports nom compilation

cargo add nom --features alloc,default-features=false
```

---

## Sources

- **nom parser crate**: https://docs.rs/nom/8.0.0/nom/ (HIGH confidence)
- **Artisan protocol spec**: https://github.com/greencardigan/TC4-shield (HIGH confidence)
- **Embassy ESP32-C3 UART**: https://github.com/embassy-rs/embassy/tree/master/embassy-esp32c3 (HIGH confidence)
- **ESP32-C3 HAL**: https://docs.rs/esp32c3/0.31.0/esp32c3/ (HIGH confidence)
- **Protocol initialization sequence**: https://homeroasters.org/forum/viewthread.php?thread_id=5818 (MEDIUM confidence - community discussion)
