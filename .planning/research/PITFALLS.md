# Domain Pitfalls: Artisan Serial Protocol Implementation

**Project:** LibreRoaster v1.5 — ESP32-C3 Artisan Protocol
**Researched:** February 4, 2026
**Confidence:** MEDIUM (based on ESP32 community sources, Artisan protocol documentation, and embedded serial communication patterns)
**Focus:** Full Artisan serial protocol implementation for ESP32-C3 firmware

---

## Overview

Implementing Artisan serial protocol on ESP32-C3 introduces pitfalls across hardware configuration, protocol timing, buffer management, and integration with existing USB CDC, multiplexer, and ArtisanFormatter components. This document catalogs critical, moderate, and minor pitfalls with detection strategies and prevention approaches organized by implementation phase.

---

## Critical Pitfalls

Pitfalls that cause data corruption, connection failures, or complete protocol breakdown.

### Pitfall 1: UART Buffer Overflow During High-Frequency Updates

**What goes wrong:** The ESP32-C3 UART hardware buffer fills faster than application code can process incoming Artisan commands, causing dropped bytes and corrupted protocol frames.

**Why it happens:** Artisan sends commands at approximately 1Hz to 10Hz depending on configuration. Combined with existing USB CDC traffic and temperature sampling, the UART peripheral may not get sufficient CPU time to empty its hardware FIFO.

**Warning signs:**
- `UART buffer full` errors in serial output
- WDT (Watchdog Timer) timeout panics on CPU1
- Garbled or missing bytes in protocol frames
- Artisan shows "Comm error" intermittently during testing

**Prevention:**
- Implement UART event-driven processing using ESP-IDF `uart_event_t` with dedicated RX buffer
- Set appropriate watermark levels: `uart_set_rx_full_threshold()` at 50-60% of buffer size
- Use FreeRTOS queue to pass received data to processing task, never block in ISR
- Allocate RX buffer size >= 2x maximum expected frame size (Artisan frames are typically 32-64 bytes)
- Monitor `UART_BUFFER_FULL` and `UART_FIFO_OVF` events

**Phase to address:** Protocol Core (Phase 2) — Buffer sizing must be finalized before frame parsing validation.

**Sources:**
- ESP32 GitHub Issue #6326: "UART locks and stops receiving data"
- ESP32 GitHub Issue #10420: "UART Communication Delays and Data Loss"
- PlatformIO Community discussions on ESP32 UART receiving issues

---

### Pitfall 2: Incorrect Baud Rate or Clock Accuracy

**What goes wrong:** UART communication fails or produces garbled data because baud rates don't match between ESP32-C3 and Artisan host, or ESP32 clock drift exceeds tolerance.

**Why it happens:** ESP32-C3 uses reference clock that may have ±2-5% drift depending on temperature and voltage. Artisan expects exact baud rates (115200 is standard for TC4 protocol).

**Warning signs:**
- `E` characters or other garbage in serial output
- "Baud rate mismatch" warnings in Artisan
- Unstable connection that works for a few seconds then fails

**Prevention:**
- Use ESP32-C3's internal PLL-derived UART clock for better accuracy
- Configure baud rate programmatically: `uart_param_config()` with verified rate
- Test with Artisan's built-in baud rate detection if available
- Add small tolerance in Artisan configuration (though Artisan is strict)
- Keep ESP32-C3 away from high-temperature areas of the roaster
- Verify clock accuracy with oscilloscope or logic analyzer if issues persist

**Phase to address:** Protocol Core (Phase 2) — Hardware configuration must be validated before any protocol testing.

---

### Pitfall 3: Blocking Operations in UART Interrupt Handler

**What goes wrong:** Long-running code in UART ISR causes system deadlock or WDT timeout, especially on ESP32-C3's dual-core architecture.

**Why it happens:** UART RX interrupt fires when bytes arrive. If ISR performs string operations, buffer copies, or waits on mutexes, it can block other interrupts or starve CPU core.

**Warning signs:**
- `Guru Meditation Error: Core 1 panic'ed (Interrupt wdt timeout)`
- Temperature readings become erratic during serial activity
- System works with polling, fails with interrupt mode

**Prevention:**
- Keep UART ISR minimal: copy bytes to ring buffer, signal semaphore/xQueue
- Never perform protocol parsing or string operations in ISR
- Use `portDISABLE_INTERRUPTS()` for shortest possible critical section
- On ESP32-C3, ensure UART interrupts run on correct core (CPU0 for most configurations)
- Never use blocking calls (mutex lock, queue send with wait) in ISR context

**Safe ISR pattern:**
```c
static void uart_isr_handler(void *arg) {
    uint8_t data[UART_FIFO_LEN];
    int len = uart_read_bytes(UART_NUM, data, UART_FIFO_LEN, 10 / portTICK_PERIOD_MS);
    if (len > 0) {
        // Copy to ring buffer, signal processing task
        for(int i = 0; i < len; i++) {
            ring_buffer_write(data[i]);
        }
        BaseType_t woken = pdFALSE;
        xQueueSendFromISR(rx_queue, &len, &woken);
        if (woken) portYIELD_FROM_ISR();
    }
}
```

**Phase to address:** Protocol Core (Phase 2) — ISR design must be validated before integration testing.

---

### Pitfall 4: Race Condition in Shared Buffer Access

**What goes wrong:** Temperature data being transmitted to Artisan is corrupted because UART TX task and temperature sampling task write to same buffer without synchronization.

**Why it happens:** FreeRTOS tasks run concurrently. If ArtisanFormatter writes a frame while temperature task updates sensor values, partially-updated data gets transmitted.

**Warning signs:**
- Corrupted frames only appear during active roasting (when multiple tasks running)
- Data looks valid but has impossible values (negative temperatures, sudden jumps)
- Failures increase with higher sampling rates

**Prevention:**
- Use FreeRTOS mutexes or semaphores to protect shared ArtisanFormatter buffers
- Consider double-buffering: one buffer for temperature updates, one for transmission
- Use atomic operations for sequence counter updates
- Always copy complete frame before transmission, never transmit in-place

**Phase to address:** Protocol Core (Phase 2) — Task synchronization must be designed before integration with ArtisanFormatter.

---

### Pitfall 5: Incorrect Artisan Protocol Frame Format

**What goes wrong:** Artisan rejects valid-looking data because frame format doesn't match TC4 protocol specification (delimiters, field order, checksum).

**Why it happens:** The Artisan TC4 protocol uses specific character-based formatting with `R` prefix for readings, specific field separators, and line terminators. Minor format errors cause rejection.

**Warning signs:**
- Artisan connects but shows no data
- "Invalid frame" or "Checksum error" messages
- Device appears in Artisan but never receives readings

**Prevention:**
- Study Artisan TC4 protocol specification: uses `R,TTTT,TTTT,TTTT,TTTT,...\r\n` format
- Verify field count matches Artisan configuration (4 channels minimum)
- Include all required fields even if unused channels are 0
- Use `\r\n` (CRLF) as line terminator, not just `\n`
- Validate frame checksum if protocol variant requires it
- Use centralized formatter for consistent output

**Correct frame format:**
```
R,225,224,023,0,0,0,0\r\n
```
- `R` - Reading prefix
- `,` - Field separator
- Four temperature values (integer, deci-degrees Celsius)
- Additional fields for events/switches
- `\r\n` - Line terminator (CRITICAL)

**Phase to address:** Protocol Core (Phase 2) — Frame format validation must be completed before Artisan integration testing.

---

## Moderate Pitfalls

Mistakes that cause delays, degraded performance, or integration issues with existing system.

### Pitfall 6: Conflict Between USB CDC and UART0

**What goes wrong:** USB CDC and UART0 share ESP32-C3's same hardware peripheral, causing conflicts when both are used simultaneously for different purposes.

**Why it happens:** ESP32-C3 UART0 TX/RX pins can route to either USB CDC (USB_JTAG serial) or GPIO pins. Default configurations may conflict.

**Warning signs:**
- "Port in use" errors in Arduino IDE or terminal
- Artisan cannot open serial port while Serial Monitor open
- Garbled output when both interfaces attempt communication

**Prevention:**
- For v1.5, prefer dedicated UART pins (not USB CDC pins) for Artisan protocol
- Use ESP32-C3's UART1 or UART2 for Artisan, keep UART0 for debugging
- Configure `usb_serial_jtag` driver appropriately if using USB
- Consider separate USB endpoints for debugging vs Artisan traffic
- Document pin assignments clearly in hardware documentation

**Phase to address:** Protocol Core (Phase 2) — Hardware pin assignment must be finalized before protocol implementation.

---

### Pitfall 7: ArtisanFormatter Integration Side Effects

**What goes wrong:** Modifying ArtisanFormatter to support new protocol breaks existing USB CDC output or multiplexer routing.

**Why it happens:** Existing system was designed for USB CDC output. Adding Artisan protocol requires shared formatting logic that may affect both paths.

**Warning signs:**
- USB CDC shows wrong format after Artisan changes
- Multiplexer output shows Artisan frames instead of debug data
- Temperature display works but logging fails

**Prevention:**
- Create separate Artisan-specific formatter class
- Extract common temperature formatting to shared utility
- Use polymorphism or strategy pattern for protocol selection
- Add integration tests covering both output paths
- Use feature flags to enable Artisan formatting only when needed

**Phase to address:** Integration (Phase 3) — Must be validated after individual component tests pass.

---

### Pitfall 8: Temperature Data Staleness During Artisan Communication

**What goes wrong:** Artisan displays old temperature values because communication blocking delays new sensor reads.

**Why it happens:** If ArtisanFormatter blocks while sending frames, temperature sampling task is delayed, causing gaps in roast profile.

**Warning signs:**
- Temperature readings update slowly on Artisan display
- Serial log shows timestamps with gaps
- Round-robin sensor reading takes longer than expected

**Prevention:**
- Use non-blocking or buffered transmission
- Prioritize temperature sampling over Artisan TX
- Use separate tasks: one for sampling, one for transmission
- Consider Artisan's polling rate and match or exceed it
- Implement watchdog to detect stalled sampling

**Phase to address:** Integration (Phase 3) — Performance must be validated under load.

---

### Pitfall 9: Missing Newline/Terminator Handling

**What goes wrong:** Incomplete frame transmission causes Artisan to wait indefinitely for line terminator, showing "Connecting..." forever.

**Why it happens:** UART sends bytes asynchronously. If transmission is interrupted or buffer clears before complete frame, Artisan receives partial data.

**Warning signs:**
- Artisan shows "Connecting..." with no timeout
- Serial monitor shows incomplete lines
- First byte of frame appears, rest missing

**Prevention:**
- Always verify complete frame transmission before clearing buffer
- Implement frame timeout: if no terminator within N ms, discard partial frame
- Use line-buffered parsing: accumulate until `\r\n` received
- Add guard timeout for incomplete frames (2-3 seconds max)

**Phase to address:** Protocol Core (Phase 2) — Frame validation must handle edge cases.

---

### Pitfall 10: Hardware Flow Control Not Configured

**What goes wrong:** High-volume data transmission causes buffer overflow when hardware flow control (RTS/CTS) is disabled.

**Why it happens:** Artisan protocol can generate sustained data flow. Without flow control, receiver may be overwhelmed at high baud rates.

**Warning signs:**
- Errors increase with longer roast sessions
- Higher baud rates cause more problems
- Consistent data loss at specific points in roast

**Prevention:**
- Enable hardware flow control if supported by both ESP32-C3 and Artisan
- `uart_set_hw_flow_ctrl(UART_NUM, UART_HW_FLOW_CTS, threshold)`
- If hardware flow control unavailable, implement software XON/XOFF
- Or reduce data rate to within reliable capacity

**Phase to address:** Protocol Core (Phase 2) — Test with realistic data rates.

---

### Pitfall 11: Thermocouple Polarity Reversal in Protocol

**What goes wrong:** Negative temperature values appear positive because Artisan expects different sign convention.

**Why it happens:** Some thermocouple types and Artisan configurations have different interpretations of temperature values.

**Warning signs:**
- Negative temperatures appear as 65535 or similar
- Room temperature shows 65535 instead of ~22C
- Temperature never drops after roast ends

**Prevention:**
- Verify Artisan's temperature format (integer deci-degrees: 225 = 22.5°C)
- Check thermocouple type and linearization
- Implement proper sign extension for 16-bit values
- Test with ice bath and boiling water for calibration
- Handle negative temperatures correctly (Artisan TC4 uses signed integers)

**Phase to address:** Testing (Phase 4) — Validate temperature accuracy with known references.

---

### Pitfall 12: ESP32-C3 Deep Sleep Breaking Connection

**What goes wrong:** Entering light/deep sleep for power saving causes Artisan to disconnect and not reconnect.

**Why it happens:** ESP32-C3 may enter low-power modes between roasts. Artisan expects continuous connection.

**Warning signs:**
- Connection lost after several minutes of idle
- ESP32-C3 enters light/deep sleep as expected
- Manual reconnection works but automatic fails

**Prevention:**
- Disable automatic sleep during Artisan connection
- Use `esp_pm_configure()` to prevent sleep while connected
- Implement graceful disconnect: send Artisan event before sleep
- Wake from sleep on Artisan connection attempt

**Phase to address:** Integration (Phase 3) — Power management must consider protocol state.

---

## Minor Pitfalls

Mistakes that cause annoyance but are fixable with configuration or small code changes.

### Pitfall 13: Artisan Channel Configuration Mismatch

**What goes wrong:** Artisan configured for 4 channels but ESP32-C3 only sends 2, causing display errors or ignored readings.

**Why it happens:** TC4 protocol supports up to 4 temperature channels. Artisan configuration must match actual data sent.

**Warning signs:**
- Artisan shows "0" or "--" for expected channels
- Temperature graph has unused channels
- Events not triggering on expected channels

**Prevention:**
- Document which channels are used (BT, ET, ambient, etc.)
- Match Artisan's "Channel Settings" to actual data
- Send all 4 channels even if some are unused (use 0 or valid placeholder)
- Add configuration documentation for recommended Artisan settings

**Phase to address:** Testing (Phase 4) — Verify end-to-end with actual Artisan configuration.

---

### Pitfall 14: Float Precision and Formatting Inconsistencies

**What goes wrong:** Temperature values appear with different precision or format across Artisan, causing parsing issues.

**Why it happens:** Platform-specific float formatting (locale, precision, sign handling) produces inconsistent output.

**Warning signs:**
- Artisan shows slightly different values than expected
- Snapshot tests fail on whitespace/precision differences
- Format varies between commands or restarts

**Prevention:**
- Centralize formatter with explicit format spec (precision, sign, padding)
- Use `snprintf()` with consistent format strings
- Enforce deci-degrees integer format for TC4 protocol
- Add golden snapshots for formatter output
- Use locale-invariant formatting (always use `.` for decimal)

**Phase to address:** Protocol Core (Phase 2) — Formatter hardening.

---

### Pitfall 15: Error Response Deviation from Spec

**What goes wrong:** Error codes/messages don't match Artisan expectations, causing confusing UI or retry loops.

**Why it happens:** Error handling implemented without consulting Artisan error protocol specification.

**Warning signs:**
- Artisan shows cryptic error messages
- Device appears frozen on error
- Unclear recovery path for user

**Prevention:**
- Define error schema per command (codes/messages)
- Add explicit handling for malformed frames and out-of-range values
- Return standardized error responses Artisan can interpret
- Add tests for all error paths

**Phase to address:** Protocol Core (Phase 2) — Error handling standardization.

---

## Phase-Specific Warning Summary

| Phase | Primary Pitfalls to Address |
|-------|------------------------------|
| **Protocol Core (2)** | Buffer overflow, Baud rate, ISR safety, Frame format, Flow control, Error handling |
| **Integration (3)** | USB CDC conflict, Formatter integration, Sleep modes, Data staleness |
| **Testing (4)** | Channel configuration, Temperature polarity, End-to-end validation, Error paths |

---

## Testing Strategy for Pitfall Detection

**Unit tests:**
- UART buffer overflow handling with configurable buffer sizes
- Frame parsing with malformed input, partial frames
- Race condition stress tests with multiple concurrent tasks
- Formatter output consistency across commands

**Integration tests:**
- ArtisanFormatter + Artisan protocol output verification
- Temperature sampling during sustained TX (load testing)
- USB CDC + Artisan UART simultaneous operation
- Mock UART with backpressure and timing simulation

**System tests:**
- 2+ hour continuous roast simulation
- Temperature cycling (-20°C to 300°C)
- Power cycle recovery and reconnection
- Artisan reconnection after disconnect
- All Artisan commands tested end-to-end

---

## Integration with Existing System

### USB CDC Considerations
- UART0 conflict resolved by using UART1/UART2 for Artisan
- Separate debug output from Artisan protocol traffic
- Consider virtual COM port limitations

### Multiplexer Considerations
- Artisan frames must be clearly distinguished from debug frames
- Protocol selection handled by multiplexer
- Buffer state preserved during protocol switches

### ArtisanFormatter Considerations
- Extract common formatting to shared utilities
- Maintain backward compatibility with existing USB CDC output
- Use strategy pattern for protocol-specific formatting

---

## Sources

- Artisan Scope Official Documentation: https://artisan-scope.org/devices/arduino/
- Artisan TC4 Protocol Reference: https://github.com/greencardigan/TC4-shield
- ESP32 UART Reference (ESP-IDF): https://docs.espressif.com/projects/esp-idf/en/latest/api-reference/peripherals/uart.html
- ESP32 GitHub Issues: #6326 (UART locks), #10420 (data loss), #746 (buffer overflow)
- PlatformIO Community: ESP32 UART receiving issues discussions
- ESP32 Design Mistakes (Predictable Designs): https://predictabledesigns.com/9-esp32-design-mistakes-that-kill-your-product/
- ESP32 UART Events (SaludPCB): https://saludpcb.com/esp32-uart-events-smarter-serial-communication/
