# Research: Phase 22 - Async Transport & Metadata

**Phase:** 22 (Async Transport & Metadata)
**Project:** LibreRoaster v1.7 Non-Blocking USB Logging
**Date:** 2026-02-05
**Confidence:** HIGH

---

## Executive Summary

This research outlines the technical approach for implementing the async log transport layer and channel metadata prefixing for LibreRoaster. Building on the Phase 21 foundation (`defmt` + `bbqueue`), this phase creates a background task that drains the log buffer and outputs to hardware, with channel tags for visibility.

Key findings:
- Embassy provides native support for background tasks with configurable priority.
- Channel prefixing can be implemented via a lightweight wrapper around `defmt` macros.
- UART0 is preferred over USB-Serial-JTAG for log output to avoid conflicts with Artisan communication.

---

## 1. Transport Analysis

### 1.1 UART0 vs USB-Serial-JTAG

**UART0 (Recommended):**
- GPIO20 (TX) and GPIO21 (RX)
- Standard serial output
- No conflict with Artisan USB CDC communication
- Can be used simultaneously with USB CDC

**USB-Serial-JTAG:**
- Shares USB peripheral with Artisan communication
- Potential for conflicts during high-throughput logging
- May interfere with Artisan connection stability

**Recommendation:** Use UART0 for log output to maintain separation of concerns.

### 1.2 esp-println Configuration

```rust
use esp_println::println;

fn init_uart_logging() {
    // UART0 at 115200 baud
    // Compatible with defmt format strings
}
```

---

## 2. Channel Prefix Implementation

### 2.1 Lightweight Wrapper Pattern

Create a module `src/logging/channel.rs`:

```rust
use defmt::{info, debug, warn, error};

pub enum LogChannel {
    Usb,
    Uart,
    System,
}

#[macro_export]
macro_rules! log_usb {
    ($($arg:tt)*) => {
        info!("[USB] {}", format!($($arg)*))
    }
}

pub fn log_with_channel(channel: LogChannel, message: &str) {
    let prefixed = format!("[{:?}] {}", channel, message);
    defmt::info!("{}", prefixed);
}
```

### 2.2 Alternative: Compile-Time Prefixing

```rust
#[macro_export]
macro_rules! log_channel {
    (USB, $($arg:tt)*) => { defmt::info!("[USB] {}", format!($($arg)*)) };
    (UART, $($arg:tt)*) => { defmt::info!("[UART] {}", format!($($arg)*)) };
    (SYSTEM, $($arg:tt)*) => { defmt::info!("[SYSTEM] {}", format!($($arg)*)) };
}
```

---

## 3. Async Drain Task Implementation

### 3.1 Embassy Task Pattern

```rust
use embassy_executor::task;
use embassy_time::Duration;

#[task]
async fn log_drain_task() {
    let consumer = get_log_consumer(); // From Phase 21 infrastructure

    loop {
        // Process available log messages
        if let Some(msg) = consumer.read() {
            // Format and output to UART0
            uart0_write(msg);

            // Release the buffer slot
            consumer.release(msg.len());
        }

        // Yield to other tasks (low priority)
        embassy_time::Timer::after(Duration::from_millis(10)).await;
    }
}

fn uart0_write(msg: &[u8]) {
    // Write to UART0 peripheral
    // Using esp_hal::uart::Uart
}
```

### 3.2 Priority Configuration

The drain task should run at lowest priority to avoid blocking control loops:

```rust
// In main.rs or lib.rs initialization
spawner.spawn(log_drain_task()).unwrap();
```

**Priority Guidelines:**
- **Highest**: PID control loop (if exists)
- **Medium**: Artisan communication tasks
- **Low**: Logging drain task

---

## 4. Architecture

### 4.1 Updated Logging Flow

```
┌─────────────────────────────────────────────────────────────┐
│                     APPLICATION TASKS                        │
│  (usb_reader_task, control_task, etc.)                     │
└───────────────────────────┬─────────────────────────────────┘
                            │ defmt::info!() / log_channel!(...)
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                   BBQueue (SPSC Buffer)                      │
│              Producer: Log writes (non-blocking)            │
└───────────────────────────┬─────────────────────────────────┘
                            │ Consumer
                            ▼
┌─────────────────────────────────────────────────────────────┐
│              BACKGROUND LOGGER TASK                          │
│              - Reads from bbqueue consumer                   │
│              - Adds channel prefix                          │
│              - Writes to UART0 (115200 baud)                │
│              - Runs at LOW priority                         │
└─────────────────────────────────────────────────────────────┘
```

---

## 5. Implementation Details

### 5.1 Cargo.toml Dependencies

```toml
[dependencies]
esp-hal = "0.20"  # Already included, provides UART access
embassy-executor = "0.6"  # Already included
embassy-time = "0.18"  # Already included
```

### 5.2 UART0 Configuration

```rust
// In src/hardware/uart/mod.rs or new src/logging/transport.rs

pub fn init_uart_logging() {
    let uart0 = Uart0::new(
        peripherals.UART0,
        pins.gpio20,
        pins.gpio21,
        &Config::default().baudrate(115200),
    );
}
```

### 5.3 Integration with Phase 21

1. **Phase 21** provides:
   - `BBQueue` global buffer
   - `Producer` handle for writing logs
   - `defmt::info!` macros

2. **Phase 22** adds:
   - `Consumer` handle for reading logs
   - Background drain task
   - UART0 output driver
   - Channel prefix wrapper

---

## 6. Success Criteria Validation

| Criterion | How to Verify |
|-----------|---------------|
| Async drain task runs | Check that logs appear in UART0 terminal |
| Channel prefix visible | Logs show `[USB]`, `[UART]` prefixes |
| No blocking behavior | PID/control tasks remain responsive |
| Stable under load | Heavy logging doesn't crash or hang |

---

## 7. References

- Embassy executor tasks: https://embassy.dev/book/dev/
- ESP32-C3 UART: https://docs.espressif.com/projects/esp-idf/en/latest/api-reference/peripherals/uart.html
- Non-blocking patterns: https://github.com/embassy-rs/embassy

---

## 8. Recommendations

1. **Start with UART0**: Separate logs from Artisan USB traffic.
2. **Keep drain task simple**: Read buffer → Add prefix → Write to UART.
3. **Use low priority**: Ensure control tasks always have CPU time.
4. **Test with Phase 23**: Validate end-to-end after implementing Artisan sniffing.

---

*Research completed: 2026-02-05*
*Next: Proceed to planning Phase 22 tasks*
