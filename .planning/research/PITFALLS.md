# Domain Pitfalls: USB Communication Logging on ESP32-C3

**Project:** LibreRoaster v1.7 — USB Communication Logging
**Researched:** 2026-02-05
**Confidence:** HIGH
**Focus:** Non-blocking logging for ESP32-C3 using `esp-hal` and `defmt-bbq`.

## Critical Pitfalls

### Pitfall 1: Synchronous Blocking in `esp-println`
**What goes wrong:** The entire roaster control loop stalls when the USB buffer is full or the serial monitor is not consuming data.

**Why it happens:** The default `defmt` implementation in `esp-println` is synchronous. If the hardware USB-Serial-JTAG FIFO (64 bytes) fills up, `esp-println` will spin until space becomes available. In a real-time system, this "spin" can exceed 100ms, missing PID cycles.

**Prevention:** 
- Use **`defmt-bbq`** to decouple logging from hardware writes.
- Never use `println!` or standard `esp-println` logs in time-critical paths.

### Pitfall 2: Shared Resource Conflict (UsbSerialJtag)
**What goes wrong:** Artisan protocol communication fails or returns garbage when logging is enabled.

**Why it happens:** The ESP32-C3 has a single **USB-Serial-JTAG** peripheral. If the application uses it as a serial port for Artisan (via `esp-hal::UsbSerialJtag`) and simultaneously uses it for `defmt` logging (via a background task), the streams will be interleaved. Since `defmt` is binary-encoded and Artisan is ASCII, the Artisan software on the host will likely crash or report communication errors.

**Prevention:**
- Use a dedicated transport for logs (e.g., UART0 pins) if USB CDC is the primary control channel.
- Alternatively, wrap logs in a protocol the host can ignore, or use RTT which is separate from the Serial interface on the C3.

### Pitfall 3: BBQueue Overrun and Log Dropping
**What goes wrong:** Critical debug information is missing during a crash or high-load event.

**Why it happens:** When logging high-frequency USB communication (sniffing), the log volume can exceed the transmission speed of the USB-Serial-JTAG interface. If the `BBQueue` fills up, `defmt-bbq` will drop logs to prevent blocking the producer.

**Prevention:**
- Implement **Smart Filtering** to suppress repetitive `READ` commands.
- Size the `BBQueue` appropriately for expected bursts (e.g., 4KB-8KB).
- Use `defmt::trace!` for low-priority sniffer data and `defmt::info!` for critical system events.

## Moderate Pitfalls

### Pitfall 4: Critical Section Jitter
**What goes wrong:** PID timing fluctuates (jitter) when logs are submitted.

**Why it happens:** Even with `defmt-bbq`, submitting a log requires a short critical section to reserve space in the queue. While very fast (sub-microsecond), frequent calls can accumulate and affect the Embassy executor's wakeup latency.

**Prevention:**
- Batch log submissions if possible.
- Ensure the Embassy timer group has a high enough priority.

### Pitfall 5: Watchdog Resets during Large Log Flushes
**What goes wrong:** The system resets during heavy logging.

**Why it happens:** If the background `logger_task` runs at too high a priority and attempts to flush a large `BBQueue` in a single tight loop, it might starve the Task Watchdog (TWDT) or the PID loop.

**Prevention:**
- Use `Timer::after_ticks(0).await` or `yield_now().await` inside the logger loop to allow other tasks to run between packet writes.

## Phase-Specific Warnings

| Phase | Likely Pitfall | Mitigation |
|-------|---------------|------------|
| **1: Foundation** | Synchronous Blocking | Register `defmt-bbq` immediately; verify `defmt` doesn't use `esp-println`. |
| **2: Transport** | Shared Port Conflict | Configure `logger_task` to use an alternative UART or handle multiplexing. |
| **3: Integration** | BBQueue Overrun | Implement `READ` command filtering in the sniffer. |

## Sources
- [esp-hal Documentation: USB-Serial-JTAG](https://docs.esp-rs.org/esp-hal/esp-hal/1.0.0/esp32c3/esp_hal/usb_serial_jtag/index.html)
- [defmt-bbq: Non-blocking patterns](https://github.com/knurling-rs/defmt-bbq)
- [ESP32-C3 Technical Reference Manual](https://www.espressif.com/sites/default/files/documentation/esp32-c3_technical_reference_manual_en.pdf)
