# Research: Phase 23 - USB Traffic Sniffing

**Phase:** 23 (USB Traffic Sniffing)
**Project:** LibreRoaster v1.7 Non-Blocking USB Logging
**Date:** 2026-02-05
**Confidence:** HIGH

---

## Executive Summary

This phase implements the instrumentation of Artisan communication channels to capture and log all USB CDC traffic. Using the infrastructure from Phases 21-22, we add logging calls to the existing USB reader and writer tasks.

Key findings:
- Minimal research needed - this is straightforward instrumentation
- Use existing `log_channel!` macro from Phase 22
- Place log calls at strategic points in usb_reader_task and usb_writer_task
- No new dependencies or architecture changes needed

---

## 1. Implementation Approach

### 1.1 USB Reader Task Instrumentation

In `src/hardware/usb_cdc/tasks.rs`, the `usb_reader_task` processes incoming Artisan commands:

```rust
// Current flow:
loop {
    if let Some(usb) = get_usb_cdc_driver() {
        match usb.read_bytes(&mut rbuf).await {
            Ok(len) if len > 0 => {
                process_usb_command_data(&rbuf[..len]);
            }
            _ => {}
        }
    }
    Timer::after(Duration::from_millis(10)).await;
}
```

**Add logging:**
```rust
loop {
    if let Some(usb) = get_usb_cdc_driver() {
        match usb.read_bytes(&mut rbuf).await {
            Ok(len) if len > 0 => {
                // Log raw bytes received
                let raw_cmd = core::str::from_utf8(&rbuf[..len]).unwrap_or("[binary]");
                log_channel!(USB, "RX: {}", raw_cmd.trim_end());
                process_usb_command_data(&rbuf[..len]);
            }
            _ => {}
        }
    }
    Timer::after(Duration::from_millis(10)).await;
}
```

### 1.2 USB Writer Task Instrumentation

In `src/hardware/usb_cdc/tasks.rs`, the `usb_writer_task` sends responses:

```rust
loop {
    if let Ok(data) = output_channel.try_receive() {
        if let Some(usb) = get_usb_cdc_driver() {
            let mut bytes = data.as_bytes().to_vec();
            bytes.extend_from_slice(b"\r\n");
            
            // Log response before sending
            log_channel!(USB, "TX: {}", data);
            
            if let Err(e) = usb.write_bytes(&bytes).await {
                warn!("USB CDC write error: {:?}", e);
            }
        }
    }
    Timer::after(Duration::from_millis(5)).await;
}
```

### 1.3 Response to Artisan

The `output_channel` receives formatted responses from the Artisan formatter:

```rust
// Example response string:
// "185.2,192.3,-1.0,-1.0,24.5,45,75"

log_channel!(USB, "TX: {}", response_string);
```

---

## 2. Log Format Specification

### 2.1 Receive (RX) Logs

```
[USB] RX: READ\r\n
[USB] RX: OT1 75\r\n
[USB] RX: START\r\n
```

### 2.2 Transmit (TX) Logs

```
[USB] TX: 185.2,192.3,-1.0,-1.0,24.5,45,75
[USB] TX: ERR 1 Unknown command
```

### 2.3 Error Logs

```
[USB] RX: [parse error]
[USB] TX ERROR: transmission failed
```

---

## 3. Integration Points

### 3.1 Modified Files

1. `src/hardware/usb_cdc/tasks.rs`
   - Add logging to `usb_reader_task`
   - Add logging to `usb_writer_task`

### 3.2 Reused Components

- `log_channel!` macro from `src/logging/channel.rs`
- `defmt::info!` underlying implementation
- Non-blocking behavior preserved (logs write to bbqueue, don't block)

---

## 4. Success Criteria Validation

| Criterion | How to Verify |
|-----------|---------------|
| All commands logged | Watch UART0 terminal during Artisan session |
| All responses logged | Same as above |
| No Artisan impact | Verify Artisan continues to poll and receive data |
| Format correct | Check log output matches specification |

---

## 5. Recommendations

1. **Start simple**: Log raw ASCII first, add parsed commands later if needed
2. **Test with READ command**: Most frequent, good for validation
3. **Watch for performance**: Ensure logs don't slow down communication
4. **Use trim_end()**: Remove trailing `\r\n` from logs for readability

---

*Research completed: 2026-02-05*
*Next: Proceed to planning Phase 23 tasks*
