# Phase 10: Mock UART Integration - Research

**Researched:** 2026-02-04
**Domain:** Rust embedded mock UART integration testing
**Confidence:** HIGH

## Summary

This phase is about validating end-to-end UART command flows in a host-friendly test harness that exercises the real parsing, command handling, and response formatting pipelines. The existing code already defines the UART reader/writer tasks, command parsing, service container channels, and roaster state transitions; the missing piece is wiring those pieces together in integration-style tests that simulate UART RX/TX and assert state changes and output responses.

The standard approach in this codebase is to pass received bytes into `process_command_data` (which handles CR-terminated commands and enqueues parsed commands), then drive `RoasterControl::process_artisan_command` through the service container channel, and finally read output responses from the output channel (or through a mock UART driver). STOP must use the safe shutdown sequence that disables streaming and zeroes outputs.

**Primary recommendation:** Build mock UART tests around the existing channel-based pipeline (UART RX → `process_command_data` → artisan channel → `process_artisan_command` → output channel), and assert responses using `ArtisanFormatter`/`ERR <code> <message>` without custom formatting.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| heapless | 0.8.0 | Fixed-capacity strings/vectors | Used in UART parsing and output buffers (`heapless::String`, `Vec`) |
| embassy-sync | 0.6.1 | Channels/Pipes for UART command and output queues | Used by `ServiceContainer` and UART tasks |
| embassy-time | 0.5.0 | Timing in async tasks | Used by UART and control loop tasks |
| embedded-io | 0.7.1 | UART write trait | Used by `UartDriver::write_bytes` |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| esp-hal | ~1.0 | UART driver on ESP32-C3 | Embedded target only (`riscv32`) |
| log | 0.4.27 | Logging | Assertions and diagnostics in tasks/tests |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Custom test UART protocol | `tests/mock_uart.rs` | Existing mock already simulates RX/TX buffers and is aligned with code conventions |

**Installation:**
```bash
cargo test --tests
```

## Architecture Patterns

### Recommended Project Structure
```
src/
├── hardware/uart/   # UART driver + reader/writer tasks
├── input/           # Artisan command parsing
├── application/     # Control loop + output dispatch
├── control/         # Roaster state machine and handlers
└── output/          # Artisan response formatting
tests/
├── mock_uart.rs     # Mock UART integration tests
└── command_errors.rs
```

### Pattern 1: UART RX → Parse → Channel
**What:** Read CR-terminated data, parse into `ArtisanCommand`, enqueue to artisan channel, emit ERR on failures.
**When to use:** Any test that simulates UART input and expects correct command routing.
**Example:**
```rust
// Source: src/hardware/uart/tasks.rs
pub(crate) fn process_command_data(data: &[u8]) {
    let mut command = Vec::<u8, 64>::new();
    for &byte in data {
        if byte == 0x0D {
            handle_complete_command(&command);
            return;
        }
        if command.push(byte).is_err() {
            send_parse_error(ParseError::InvalidValue);
            return;
        }
    }
}
```

### Pattern 2: Command Handling → State + Output Channel
**What:** Process artisan command, update state/outputs, and enqueue responses (READ or ERR) to the output channel.
**When to use:** End-to-end tests verifying command effects and response generation.
**Example:**
```rust
// Source: src/application/tasks.rs
if let Ok(command) = cmd_channel.try_receive() {
    let _ = ServiceContainer::with_roaster(|roaster| {
        match roaster.process_artisan_command(command) {
            Ok(()) => {
                if let crate::config::ArtisanCommand::ReadStatus = command {
                    let status = roaster.get_status();
                    let response = ArtisanFormatter::format_read_response(
                        &status,
                        roaster.get_fan_speed(),
                    );
                    if let Ok(line) = String::<128>::try_from(response.as_str()) {
                        let _ = output_channel.try_send(line);
                    }
                }
            }
            Err(err) => { send_handler_error(output_channel, &err); }
        }
    });
}
```

### Pattern 3: STOP Safe Shutdown
**What:** STOP disables continuous output, disables PID, clears manual outputs, zeroes SSR/fan, and resets state.
**When to use:** INT-03 sequences; STOP must leave outputs safe.
**Example:**
```rust
// Source: src/control/roaster_refactored.rs
fn stop_streaming(&mut self) -> Result<(), RoasterError> {
    self.temp_handler.get_output_manager_mut().disable_continuous_output();
    self.temp_handler.disable_pid();
    self.status.pid_enabled = false;
    self.status.artisan_control = false;
    self.artisan_handler.clear_manual();
    self.status.ssr_output = 0.0;
    self.status.fan_output = 0.0;
    self.state = crate::config::constants::RoasterState::Idle;
    self.status.state = self.state;
    self.heater.set_power(0.0).map_err(|_| RoasterError::HardwareError)?;
    self.fan.set_speed(0.0).map_err(|_| RoasterError::HardwareError)?;
    Ok(())
}
```

### Anti-Patterns to Avoid
- **Bypassing `process_command_data`:** Skips ERR schema and CR-terminated parsing; use the existing parser path instead.
- **Custom response formatting:** READ/ERR formatting is centralized; use `ArtisanFormatter` and `ParseError` tokens.

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Parsing Artisan commands | Custom split/trim logic | `parse_artisan_command` | Ensures canonical errors and bounds checks |
| ERR responses | Ad-hoc strings | `ParseError::code/message` + `send_parse_error` | Standardized `ERR <code> <message>` tokens |
| READ responses | Manual CSV formatting | `ArtisanFormatter::format_read_response` | Fixed field order/precision in v1.2 |
| UART output writes | Direct UART writes in tests | Output channel + mock UART | Matches production flow and CRLF handling |

**Key insight:** The integration tests should exercise the same parsing, command routing, and formatting paths that production uses, otherwise they won’t validate protocol correctness.

## Common Pitfalls

### Pitfall 1: Missing CR (\r) terminator in UART RX
**What goes wrong:** `process_command_data` never calls `handle_complete_command`, so no command is enqueued.
**Why it happens:** The parser expects byte `0x0D` as the command terminator.
**How to avoid:** In mock UART RX, always include `\r` (or `\r\n`) per command when using `process_command_data`.
**Warning signs:** No output/ERR and no commands dequeued from the artisan channel.

### Pitfall 2: Double CRLF in output assertions
**What goes wrong:** Tests assert on strings with unexpected extra CRLF.
**Why it happens:** Output channel holds raw lines; CRLF is appended by UART writer (`artisan_output_task`) or by `send_response`.
**How to avoid:** When reading output channel directly, compare without CRLF; only include CRLF when simulating UART TX.
**Warning signs:** Assertions fail only due to trailing `\r\n`.

### Pitfall 3: Host tests invoking ESP-only UART driver
**What goes wrong:** `cargo test` fails on x86 due to `esp-hal` dependencies.
**Why it happens:** `UartDriver` is defined for `riscv32`; host tests must avoid direct use of embedded peripherals.
**How to avoid:** Use mock UART in `tests/mock_uart.rs`, channel-based flows, and stubbed `RoasterControl` dependencies.
**Warning signs:** Build errors referencing `esp_hal` or missing target-specific crates.

### Pitfall 4: STOP not leaving outputs safe
**What goes wrong:** Fan/SSR outputs or streaming remain enabled after STOP.
**Why it happens:** Tests only check command parsing, not post-command state.
**How to avoid:** After STOP, assert `ssr_output == 0.0`, `fan_output == 0.0`, `pid_enabled == false`, `artisan_control == false`, and streaming disabled.
**Warning signs:** Continuous output still enabled or non-zero outputs after STOP.

## Code Examples

### Parse + ERR schema
```rust
// Source: src/input/parser.rs
pub fn parse_artisan_command(command: &str) -> Result<ArtisanCommand, ParseError> {
    let trimmed = command.trim();
    if trimmed.is_empty() { return Err(ParseError::EmptyCommand); }
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

### READ response format
```rust
// Source: src/output/artisan.rs
let response = ArtisanFormatter::format_read_response(&status, fan_speed);
// Output: "ET,BT,Power,Fan" with fixed one-decimal precision
```

### Mock UART integration harness
```rust
// Source: tests/mock_uart.rs
let mut mock = MockUartDriver::new("READ\r\n");
let mut buffer = [0u8; 64];
let bytes_read = mock.read_bytes(&mut buffer).unwrap();
let command_str = core::str::from_utf8(&buffer[..bytes_read]).unwrap();
let parsed = parse_artisan_command(command_str.trim_end());
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Direct UART write per command | Channel-based command/output pipeline (`ServiceContainer` + output task) | Unknown | Makes end-to-end testing possible via channels and mocks |
| Ad-hoc error strings | Standard `ERR <code> <message>` tokens | Phase 9 | Stable parsing of errors across tests |

**Deprecated/outdated:**
- Direct UART writes in tests: bypasses output channel CRLF handling and ERR schema.

## Open Questions

1. **Host-compatible test harness for ESP-only dependencies**
   - What we know: `cargo test` fails on x86 due to `esp-hal`/embedded target requirements.
   - What's unclear: Whether existing `tests` are expected to run under `--features test` or via a dedicated host stub module.
   - Recommendation: Plan tests to avoid `esp-hal` types and use stubs/mocks; document the command for host test execution.

## Sources

### Primary (HIGH confidence)
- `src/hardware/uart/tasks.rs` - UART parsing pipeline and ERR emission
- `src/application/tasks.rs` - command handling and output channel usage
- `src/control/roaster_refactored.rs` - state transitions and STOP safe shutdown
- `src/input/parser.rs` - Artisan command parsing and ERR tokens
- `src/output/artisan.rs` - READ response formatting
- `tests/mock_uart.rs` - existing mock UART harness
- `tests/command_errors.rs` - ERR schema validation
- `tests/command_idempotence.rs` - START/STOP/OT1/IO3 state checks
- `Cargo.toml` - standard stack versions

### Secondary (MEDIUM confidence)
- None

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - versions sourced from `Cargo.toml`
- Architecture: HIGH - based on current task flow and control logic
- Pitfalls: MEDIUM - derived from code expectations and known host test blocker

**Research date:** 2026-02-04
**Valid until:** 2026-03-06
