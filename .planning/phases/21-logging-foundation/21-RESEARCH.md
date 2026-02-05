# Research: Phase 21 - Logging Foundation

**Phase:** 21 (Logging Foundation)
**Project:** LibreRoaster v1.7 Non-Blocking USB Logging
**Date:** 2026-02-05
**Confidence:** HIGH

---

## Executive Summary

This research outlines the technical approach for implementing a non-blocking logging infrastructure using `defmt` and `bbqueue` for the LibreRoaster ESP32-C3 firmware. The goal is to ensure that logging operations do not block the critical 100ms PID control loop, following the "deferred logging" pattern standard in high-performance embedded Rust systems.

Key findings:
- `defmt` is the industry standard for logging in embedded Rust, offering 10-100x lower overhead than `log`.
- `bbqueue` provides a lock-free SPSC queue ideal for decoupling log production from consumption.
- The ESP32-C3's USB-Serial-JTAG peripheral can be used for log output without blocking the main executor.

---

## 1. Stack Analysis

### 1.1 Logging Framework: `defmt`

`defmt` ("deferred formatting") is the de facto standard logging framework for embedded Rust. Unlike `log`, it defers string formatting to the host side, resulting in:
- **Minimal footprint**: Only format string indices and arguments are stored in the binary.
- **Zero-copy**: No dynamic allocation for string formatting.
- **High performance**: Log calls are mere memory writes.

**Recommended Version:** `defmt = "0.3"` (latest stable as of 2026-02)

**Integration with Embassy:**
```rust
use defmt::{info, debug, warn, error};

// Example in async task:
info!("PID loop completed in {}ms", elapsed_ms);
```

**Key Features:**
- `defmt::println!` for simple output without format strings.
- `defmt::assert!`, `defmt::unreachable!` for assertions.
- Global log level filter at compile time.

### 1.2 Buffer: `bbqueue`

`bbqueue` (BipBuffer Queue) is a lock-free, single-producer, single-consumer (SPSC) circular buffer designed for embedded systems.

**Why `bbqueue` over `heapless::spsc::Queue`:**
- Optimized for DMA and hardware transaction completion.
- No atomic operations required (safe for single-core ESP32-C3).
- Proven track record in embedded Rust applications.

**Recommended Version:** `bbqueue = "0.5"` (or `bbqueue = "0.4"` for no_std)

**Initialization Pattern:**
```rust
use bbqueue::{BBQueue, Producer, Consumer};

static BUFFER: BBQueue<1024> = BBQueue::new(1024);

fn init_logging() -> Result<(Producer, Consumer), ()> {
    BUFFER.try_split().map_err(|_| ())
}
```

### 1.3 ESP32-C3 USB-Serial-JTAG Integration

The ESP32-C3 includes a USB Serial JTAG peripheral that can be used for both debugging and application communication.

**Configuration in `esp-hal`:**
```rust
use esp_hal::usb_serial_jtag::UsbSerialJtag;

let usb_serial = UsbSerialJtag::new(peripherals.USB_DEVICE);
```

**Key Considerations:**
- The USB-Serial-JTAG is shared with JTAG debugging.
- Use `esp-println` or `defmt`-compatible serial output.
- Ensure non-blocking writes to prevent executor stall.

---

## 2. Architecture

### 2.1 Deferred Logging Pattern

The architecture follows the standard deferred logging pattern:

```
┌─────────────────────────────────────────────────────────────┐
│                     APPLICATION TASKS                        │
│  (usb_reader_task, control_task, etc.)                     │
└───────────────────────────┬─────────────────────────────────┘
                            │ defmt::info!()
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                   BBQueue (SPSC Buffer)                      │
│              Producer: Log writes (non-blocking)            │
└───────────────────────────┬─────────────────────────────────┘
                            │ Consumer
                            ▼
┌─────────────────────────────────────────────────────────────┐
│              BACKGROUND LOGGER TASK                          │
│              - Drains buffer                                │
│              - Formats output                                │
│              - Writes to USB/JTAG                            │
└─────────────────────────────────────────────────────────────┘
```

**Key Properties:**
- **Non-blocking**: `defmt!` macros write to the buffer and return immediately.
- **Thread-safe**: Single-producer guarantee (one executor thread).
- **Lossy on overflow**: Oldest logs are dropped when buffer is full (configurable).

### 2.2 Global Buffer Initialization

The `bbqueue` should be initialized as a static:

```rust
use bbqueue::BBQueue;
use core::sync::atomic::{AtomicBool, Ordering};

static LOG_BUFFER: BBQueue<2048> = BBQueue::new(2048);
static LOG_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn init_logging() -> Result<(), ()> {
    if LOG_INITIALIZED.load(Ordering::SeqCst) {
        return Ok(()); // Already initialized
    }
    // Initialization logic
    LOG_INITIALIZED.store(true, Ordering::SeqCst);
    Ok(())
}
```

---

## 3. Implementation Details

### 3.1 Cargo.toml Dependencies

```toml
[dependencies]
defmt = "0.3"
defmt-rtt = "0.4"  # For RTT output
bbqueue = "0.5"
esp-println = { version = "0.11", features = ["defmt"] }
```

### 3.2 Build Configuration

**Memory-Optimized Build:**
```toml
[profile.release]
opt-level = "z"  # Optimize for size
lto = true       # Link-time optimization
```

### 3.3 Integration Points

1. **Cargo.toml**: Add `defmt` and `bbqueue` dependencies.
2. `src/main.rs`: Initialize logging system at startup.
3. `src/hardware/usb_cdc/tasks.rs`: Add log statements for Artisan communication.
4. `src/lib.rs`: Ensure `defmt-rtt` is initialized for debug output.

---

## 4. Non-Blocking Verification Strategy

To verify that logging does not block the PID loop:

### 4.1 Instrumented Test

```rust
#[test]
fn test_logging_does_not_block_pid() {
    let start = embassy_time::Instant::now();
    for i in 0..1000 {
        info!("Log iteration {}", i);
    }
    let elapsed = start.duration_since_embassy().as_millis();
    assert!(elapsed < 10, "Logging blocked for {}ms", elapsed);
}
```

### 4.2 Logic Analyzer Verification

Use a logic analyzer to verify that:
- GPIO toggle intervals remain consistent during heavy logging.
- No dropped USB packets during log bursts.

---

## 5. Potential Risks and Mitigations

### 5.1 Buffer Overflow

**Risk**: If the log producer outpaces the consumer, the buffer fills up.
**Mitigation**: Configure `bbqueue` size based on expected log volume. Default to 2KB.

### 5.2 USB Write Blocking

**Risk**: Writing to USB-Serial-JTAG may block if the host is not reading.
**Mitigation**: Use non-blocking writes and implement a timeout or drop policy.

### 5.3 Memory Overhead

**Risk**: Large `defmt` string tables increase binary size.
**Mitigation**: Use `log` level filtering to exclude low-priority logs from release builds if needed.

---

## 6. Success Criteria Validation

| Criterion | How to Verify |
|-----------|---------------|
| `defmt` integrated | `cargo build` succeeds with `defmt::info!` macros |
| `bbqueue` initialized | Static buffer allocates correctly in flash |
| Non-blocking writes | Unit test confirms <1ms for 1000 log calls |
| PID loop unaffected | Logic analyzer shows consistent 100ms intervals |

---

## 7. References

- `defmt` crate: https://docs.rs/defmt/0.3.5/defmt/
- `bbqueue` crate: https://docs.rs/bbqueue/0.5.0/bbqueue/
- Embassy executor: https://embassy.dev/book/dev/
- ESP32-C3 datasheet: https://www.espressif.com/sites/default/files/documentation/esp32-c3_datasheet_en.pdf
- Non-blocking logging patterns: https://github.com/ferrous-systems/defmt-rtt

---

## 8. Recommendations

1. **Start with 2KB buffer**: Sufficient for initial testing; increase if needed.
2. **Use `defmt-rtt` for development**: Enables RTT output without USB.
3. **Add log statements incrementally**: Verify non-blocking behavior at each step.
4. **Benchmark before optimizing**: Measure actual PID jitter before tuning buffer size.

---

*Research completed: 2026-02-05*
*Next: Proceed to planning Phase 21 tasks*
