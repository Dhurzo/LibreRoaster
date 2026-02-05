# Feature Landscape: USB Communication Logging

**Domain:** Real-time Firmware Logging (ESP32-C3)
**Researched:** 2026-02-05

## Table Stakes

Features users expect. Missing = product feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **Non-blocking Logging** | PID loop (100ms) must never wait for logs. | High | Critical for roaster stability. |
| **Log Level Control** | Filtering (Info/Debug/Error) to manage volume. | Low | Supported natively by `defmt`. |
| **USB-Serial-JTAG Output** | Standard transport for logs on ESP32-C3. | Low | Uses internal C3 hardware. |
| **Bi-directional Monitoring** | See both Artisan requests and device responses. | Medium | Requires hooks in the command multiplexer. |

## Differentiators

Features that set product apart. Not expected, but valued.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Smart Filtering** | Suppresses repetitive `READ` polling to keep logs clean. | Medium | Only logs state changes or handshakes. |
| **Channel Tagging** | Distinguish between USB CDC and UART0 traffic. | Low | Essential for dual-channel Artisan setup. |
| **Millisecond Timestamps** | Precise timing for Artisan command/response latency. | Low | Integrated with `embassy-time`. |

## Anti-Features

Features to explicitly NOT build. Common mistakes in this domain.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **Blocking USB Write** | Stalls executor if serial monitor is closed. | Use `defmt-bbq` with a drop-oldest policy. |
| **On-chip String Formatting** | Heavy CPU/Flash usage. | Use `defmt` bit-packed logging (deferred formatting). |
| **Raw `println!` Sniffing** | Causes jitter and interleaving issues. | Use a dedicated async logger task. |

## Feature Dependencies

```
Async Logger (defmt-bbq) → Transport (UsbSerialJtag) → Command Hooks → Smart Filtering
```

## MVP Recommendation

For v1.7, prioritize:
1. **Non-blocking Async Logger:** `defmt-bbq` foundation to protect PID timing.
2. **Communication Sniffer:** Hooks in `ArtisanCommand` multiplexer to log RX/TX bytes.
3. **Smart Filter:** Initial implementation to suppress `READ` command flood.

## Sources

- [esp-hal 1.0.0 Feature Guide](https://docs.esp-rs.org/esp-hal/)
- [Artisan Protocol Documentation](https://artisan-roaster-scope.blogspot.com/)
- [Embedded Logging Best Practices](https://interrupt.memfault.com/blog/increasing-firmware-reliability-with-logging)
