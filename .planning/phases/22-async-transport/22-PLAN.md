# Plan: Phase 22 - Async Transport & Metadata

**Phase:** 22
**Goal:** Deliver logs safely to hardware serial with channel metadata.
**Requirement:** LOG-03
**Date:** 2026-02-05

---

## Overview

This phase implements the async transport layer for logging. Building on Phase 21's `defmt` + `bbqueue` foundation, we add channel prefixing and a background drain task that outputs logs to UART0.

---

## Plan 01: Channel Prefixing Module

**Objective:** Create a wrapper module that adds channel prefixes (`[USB]`, `[UART]`) to all log messages.

### Tasks

```xml
<task>
<name>Create logging channel module</name>
<description>
Create src/logging/channel.rs with:
- LogChannel enum (Usb, Uart, System)
- log_channel! macro that accepts channel and message
- Example: log_channel!(Usb, "Command received: {}", cmd)

Implementation pattern:
```rust
#[macro_export]
macro_rules! log_channel {
    (USB, $($arg:tt)*) => {
        defmt::info!("[USB] {}", format!($($arg)*))
    };
    (UART, $($arg:tt)*) => {
        defmt::info!("[UART] {}", format!($($arg)*))
    };
    (SYSTEM, $($arg:tt)*) => {
        defmt::info!("[SYSTEM] {}", format!($($arg)*))
    };
}
```

Format: `[CHANNEL] message` with single space after bracket.
</description>
<autonomous>true</autonomous>
<files_modified>
src/logging/channel.rs
src/logging/mod.rs (add module export)
</files_modified>
</task>

<task>
<name>Update USB tasks to use channel prefix</name>
<description>
Update src/hardware/usb_cdc/tasks.rs to use log_channel! macro:
- Replace defmt::info! calls with log_channel!(USB, ...)
- Add prefix to all Artisan command/received logs

Example transformation:
```rust
// Before
defmt::info!("Artisan command received: {:?}", cmd);

// After
log_channel!(USB, "Command received: {:?}", cmd);
```
</description>
<autonomous>true</autonomous>
<files_modified>
src/hardware/usb_cdc/tasks.rs
</files_modified>
</task>
</task>

<task>
<name>Verify channel prefix compilation</name>
<description>
Run cargo build and verify:
- Log messages compile with prefix format
- No new warnings introduced
- Format is consistent: `[CHANNEL] `

Test output format:
```
[USB] Command received: READ
[SYSTEM] USB reader started
```
</description>
<autonomous>true</autonomous>
<files_modified>
(none - verification only)
</files_modified>
</task>
```

### Verification
- [ ] Channel prefix module compiles without errors.
- [ ] All USB CDC logs show `[USB]` prefix.
- [ ] Format is consistent across all log messages.

### Must Haves
- [ ] `log_channel!` macro accepts USB, UART, SYSTEM channels.
- [ ] Logs are formatted as `[CHANNEL] message`.

---

## Plan 02: UART Transport Layer

**Objective:** Configure UART0 for log output to avoid conflicts with Artisan USB CDC communication.

### Tasks

```xml
<task>
<name>Create logging transport module</name>
<description>
Create src/logging/transport.rs with:
- UART0 initialization at 115200 baud (standard for ESP32-C3)
- Writer struct that implements defmt::Format for output
- Non-blocking write implementation

Pattern:
```rust
pub struct LogTransport {
    uart: Uart<'static, esp_hal::Blocking>,
}

impl LogTransport {
    pub fn new() -> Self {
        let uart = Uart0::new(
            peripherals.UART0,
            pins.gpio20,
            pins.gpio21,
            &Config::default().baudrate(115200),
        );
        Self { uart }
    }

    pub fn write(&mut self, data: &[u8]) {
        // Blocking write to UART0
        self.uart.write(data).ok();
    }
}
```

Note: GPIO20 = UART TX, GPIO21 = UART RX (standard LibreRoaster pinout).
</description>
<autonomous>true</autonomous>
<files_modified>
src/logging/transport.rs
src/logging/mod.rs (add module export)
</files_modified>
</task>

<task>
<name>Integrate UART transport with bbqueue</task>
<description>
Update src/logging/mod.rs to:
- Initialize LogTransport during logging setup
- Wire bbqueue consumer to UART writer
- Handle buffer release properly

This creates the complete Producer → Consumer → UART pipeline.
</description>
<autonomous>true</autonomous>
<files_modified>
src/logging/mod.rs
</files_modified>
</task>

<task>
<name>Verify UART output</name>
<description>
Test UART0 logging output:
1. Connect serial terminal to GPIO20 (TX) at 115200 baud
2. Run application
3. Verify log output appears on terminal
4. Confirm channel prefixes are visible

Example expected output:
```
[SYSTEM] USB reader started
[USB] Command received: READ
```
</description>
<autonomous>false</autonomous>
<files_modified>
(none - hardware verification)
</files_modified>
</task>
```

### Verification
- [ ] UART0 configured at 115200 baud.
- [ ] Logs appear on serial terminal.
- [ ] No conflicts with Artisan USB CDC communication.

### Must Haves
- [ ] UART0 TX/RX configured on GPIO20/GPIO21.
- [ ] Log output visible on serial console.
- [ ] Artisan communication unaffected.

---

## Plan 03: Async Drain Task Integration

**Objective:** Create background task that drains the bbqueue and outputs logs to UART, ensuring non-blocking behavior.

### Tasks

```xml
<task>
<name>Create async drain task</name>
<description>
Create src/logging/drain_task.rs with:
- embassy_executor::task for background processing
- Continuous loop that reads from bbqueue consumer
- Calls LogTransport::write for each log message
- Low priority (runs when other tasks are idle)

Pattern:
```rust
#[embassy_executor::task]
async fn log_drain_task() {
    let consumer = get_log_consumer(); // From Phase 21

    loop {
        if let Some(msg) = consumer.read() {
            // Write to UART0
            let transport = get_log_transport();
            transport.write(msg);

            // Release buffer slot
            consumer.release(msg.len());
        }

        // Yield to other tasks (low priority)
        Timer::after(Duration::from_millis(10)).await;
    }
}
```

Key: Task must yield frequently to avoid blocking control tasks.
</description>
<autonomous>true</autonomous>
<files_modified>
src/logging/drain_task.rs
src/main.rs (spawn drain task)
</files_modified>
</task>

<task>
<name>Test end-to-end logging</name>
<description>
Verify complete logging pipeline:
1. Application starts
2. Log messages are written via defmt macros
3. Messages accumulate in bbqueue
4. Drain task reads and outputs to UART0
5. UART0 terminal shows prefixed logs

Test sequence:
```rust
log_channel!(SYSTEM, "Application started");
log_channel!(USB, "Command received: READ");
log_channel!(SYSTEM, "Temperature: {}C", temp);
```

Expected UART0 output:
```
[SYSTEM] Application started
[USB] Command received: READ
[SYSTEM] Temperature: 185.5C
```
</description>
<autonomous>false</autonomous>
<files_modified>
(none - integration verification)
</files_modified>
</task>

<task>
<name>Verify system stability</name>
<description>
Verify that logging doesn't affect system stability:
1. Heavy logging load (1000+ messages)
2. Verify executor remains responsive
3. Check no dropped messages (buffer overflow)
4. Confirm Artisan communication still works

Document findings in src/logging/STABILITY.md
</description>
<autonomous>false</autonomous>
<files_modified>
src/logging/STABILITY.md
</files_modified>
</task>
```

### Verification
- [ ] Async drain task runs without blocking.
- [ ] Logs appear on UART0 terminal.
- [ ] System remains stable under heavy logging load.

### Must Haves
- [ ] Background task drains bbqueue to UART0.
- [ ] All log messages include channel prefixes.
- [ ] Executor stability verified under load.

---

## Dependencies Between Plans

```
Plan 01 (Channel Prefixing)
    │
    ▼
Plan 02 (UART Transport) ← Requires Plan 01
    │
    ▼
Plan 03 (Drain Task) ← Requires Plan 02
```

---

## Waves

| Wave | Plans | What it builds |
|------|-------|----------------|
| 1 | 01 | Channel prefixing module |
| 2 | 02 | UART transport layer |
| 3 | 03 | Async drain task integration |

---

## Summary

Phase 22 completes the logging infrastructure by adding:

1. **Channel Prefixing**: Logs are tagged with their source (`[USB]`, `[UART]`, `[SYSTEM]`)
2. **UART Transport**: Logs output via UART0 to avoid USB conflicts
3. **Async Drain Task**: Background task handles output without blocking

Phase 23 will implement Artisan traffic sniffing using these foundations.

---

*Plan created: 2026-02-05*
