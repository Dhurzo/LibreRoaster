# Technology Stack: Non-Blocking USB Logging

**Project:** LibreRoaster (ESP32-C3)
**Researched:** 2026-02-05
**Focus:** Non-blocking logging for USB CDC communication in an Embassy/esp-hal environment.

## Recommended Stack Additions

For non-blocking logging on the ESP32-C3 with Embassy, the **`defmt` + `defmt-bbq`** pattern is the prescriptive choice. It separates the log-site execution from the hardware transmission, ensuring that logging high-frequency USB traffic does not stall the roaster's control loops.

### Core Logging Framework
| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `defmt` | `0.3.10` | Logging Interface | Deferred formatting: only sends IDs/data. Formatting happens on host. Minimizes CPU cycles and binary size. |
| `defmt-bbq` | `0.1.0` | Global Logger Shim | Routes `defmt` output into a `bbqueue` instead of writing directly to hardware. Essential for non-blocking behavior. |
| `bbqueue` | `0.5.1` | Lock-free Buffer | Provides a SPSC (Single Producer Single Consumer) queue for encoded log data. Memory-efficient and async-friendly. |

### Hardware Integration (Updates)
| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `esp-hal` | `1.0.0` | Peripheral Drivers | Stable 1.0 release. Use `UsbSerialJtag` or `Uart` with `async` features for the logging drain task. |
| `esp-hal-embassy` | `0.5.0` | Async Runtime | Provides the executor and timer support needed for the background logging drain task. |

## Integration Strategy

The logging system should be implemented as a **Producer-Consumer** pattern:

1.  **Global Logger:** `defmt-bbq` is registered as the `#[defmt::global_logger]`. When `info!`, `debug!`, etc., are called, data is pushed into the `bbqueue`. This is a "near-instant" memory write.
2.  **Drain Task:** A low-priority Embassy task (`logger_task`) holds the `bbqueue::Consumer`. It polls the queue and uses the `esp-hal` async `write_all` method to send bytes to the physical transport.
3.  **Transport Selection:** 
    *   **Development:** Use the internal `UsbSerialJtag` (JTAG-Serial) for logs. Note that if this is also used for Artisan (CDC), you must ensure `defmt` framing is handled or use RTT.
    *   **Production:** Log over `UART0` or use RTT (Real-Time Transfer) via `esp-println`'s RTT feature if a debugger is attached.

## defmt vs log: Decision Matrix

| Criterion | `defmt` (Recommended) | `log` |
|-----------|-----------------------|-------|
| **Blocking** | **Non-blocking** (via `defmt-bbq`) | Typically blocking (formats strings on-chip) |
| **Performance** | High (IDs only) | Low (String formatting is CPU intensive) |
| **Flash Usage** | Minimal (Strings stored in ELF on PC) | Significant (Strings stored in Flash) |
| **RAM Usage** | Low | Higher (Formatting buffers) |
| **Best For** | Real-time control, async Embassy | General CLI tools, legacy compatibility |

## What NOT to Add

- **`esp-println` with default features:** Standard `esp-println` logging is synchronous and will block the Embassy executor, causing jitter in Artisan command parsing.
- **`std::fmt` or `alloc`:** Keep the stack `no_std`. `defmt` handles complex structures without needing `format!`.
- **Custom RingBuffers:** `bbqueue` is already optimized for `defmt`'s variable-length encoding and contiguous slice requirements.

## Installation

```toml
[dependencies]
# Logging
defmt = "0.3.10"
defmt-bbq = "0.1.0"
bbqueue = "0.5.1"

# HAL (ensure async and chip features)
esp-hal = { version = "1.0.0", features = ["esp32c3", "async"] }
esp-hal-embassy = { version = "0.5.0", features = ["esp32c3", "integrated-timers"] }
```

## Sources
- [esp-hal 1.0.0 Release Notes](https://github.com/esp-rs/esp-hal/releases/tag/v1.0.0)
- [Embassy Documentation: Logging Patterns](https://embassy.dev/book/dev/logging.html)
- [defmt-bbq GitHub](https://github.com/knurling-rs/defmt-bbq)
- [Espressif Rust Ecosystem - 2026 Update](https://docs.esp-rs.org/)
