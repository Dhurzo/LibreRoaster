# Project Research Summary

**Project:** LibreRoaster (ESP32-C3)
**Domain:** Non-Blocking USB Logging for Real-Time Control
**Researched:** 2026-02-05
**Confidence:** HIGH

## Executive Summary

LibreRoaster v1.7 requires a robust, non-blocking logging system to monitor USB communication without compromising the 100ms PID control loop. Standard synchronous logging (like `esp-println`) poses a critical risk: if the USB buffer fills or a serial monitor is disconnected, the entire firmware can stall, potentially leading to dangerous roasting conditions.

The recommended approach is to adopt a **Producer-Consumer architecture** using `defmt` for deferred formatting and `defmt-bbq` as a lock-free buffer. This setup ensures that log "writes" are near-instantaneous memory operations. A low-priority background task handles the actual hardware transmission, allowing the high-priority PID and serial parser tasks to proceed without interruption. To manage log volume, "Smart Filtering" will be implemented to suppress repetitive Artisan polling commands.

## Key Findings

### Recommended Stack

The stack moves away from synchronous blocking logs toward an async-native, deferred-formatting ecosystem.

**Core technologies:**
- **defmt (0.3.10):** Logging Interface — Minimizes CPU and binary size by deferring string formatting to the host PC.
- **defmt-bbq (0.1.0):** Global Logger Shim — Decouples log sites from hardware by routing logs into a lock-free queue.
- **bbqueue (0.5.1):** Lock-free Buffer — Provides the underlying SPSC queue for high-performance, non-blocking data transfer.
- **esp-hal (1.0.0):** Peripheral Drivers — Utilizes the latest stable async drivers for non-blocking hardware I/O.

### Expected Features

**Must have (table stakes):**
- **Non-blocking Logging** — PID loop must never wait for logs; users expect roaster stability regardless of logging state.
- **Log Level Control** — Standard Info/Debug/Error filtering to manage log verbosity.
- **USB-Serial-JTAG Output** — Native support for the ESP32-C3's internal debug peripheral.

**Should have (competitive):**
- **Smart Filtering** — Suppression of repetitive `READ` commands to keep logs focused on meaningful state changes.
- **Millisecond Timestamps** — High-precision timing integrated with `embassy-time` for latency analysis.

**Defer (v2+):**
- **Remote Log Streaming** — Streaming logs over Wi-Fi/UDP is out of scope for the current USB-focused improvement.

### Architecture Approach

The architecture follows a strictly decoupled Producer-Consumer pattern where the logging system is an observer of the serial traffic, not a participant in the critical path.

**Major components:**
1. **defmt Macros** — Fast log producers embedded in the Serial Reader/Writer tasks.
2. **defmt-bbq (BBQueue)** — Centralized buffer managing the life-cycle of log data.
3. **Async Logger Task** — A low-priority consumer that drains the queue and writes to the hardware transport async.

### Critical Pitfalls

1. **Synchronous Blocking in `esp-println`** — Prevented by strictly using `defmt-bbq` and avoiding standard `println!`.
2. **Shared Resource Conflict (UsbSerialJtag)** — Avoided by using a dedicated transport (UART0) or protocol-aware multiplexing to prevent interleaving logs with Artisan data.
3. **BBQueue Overrun** — Mitigated by "Smart Filtering" and a "Drop-Oldest" policy to ensure the system remains responsive even under heavy log load.

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Logging Foundation
**Rationale:** Establishing the non-blocking buffer and `defmt` integration is the prerequisite for all subsequent features.
**Delivers:** Working `defmt` setup over `bbqueue` with a basic async drain task.
**Addresses:** Non-blocking Logging.
**Avoids:** Synchronous Blocking Pitfall.

### Phase 2: Transport Configuration
**Rationale:** The ESP32-C3 has specific hardware constraints for USB-Serial-JTAG that must be handled before attaching real traffic sniffers.
**Delivers:** Stable log output over a designated channel (USB-JTAG or UART0) without interfering with Artisan.
**Uses:** `esp-hal` 1.0.0 async drivers.
**Implements:** Async Logger Task.

### Phase 3: Communication Sniffer Integration
**Rationale:** Once the transport is safe, we can hook into the existing Artisan command multiplexer.
**Delivers:** Real-time visibility into RX/TX bytes for Artisan commands.
**Addresses:** Bi-directional Monitoring.
**Implements:** `defmt` log sites in Reader/Writer tasks.

### Phase 4: Smart Filtering & Polish
**Rationale:** High-frequency polling in Artisan will flood the logs; filtering is needed for usability.
**Delivers:** Logic to suppress repetitive `READ` polling logs.
**Addresses:** Smart Filtering.
**Avoids:** BBQueue Overrun Pitfall.

### Phase Ordering Rationale

- **Dependencies:** The async runtime and `bbqueue` must exist before any logging can happen safely.
- **Grouping:** Transport setup is grouped separately because it involves hardware-specific configuration (C3 JTAG vs UART) which is distinct from the logical sniffer implementation.
- **Safety:** By building the "Smart Filter" last, we ensure the system is already non-blocking and safe, even if it is "chatty" initially.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 2:** Hardware multiplexing vs RTT. Need to confirm if Artisan and logs can coexist on one USB port via different frames or if a physical UART is required.

Phases with standard patterns (skip research-phase):
- **Phase 1:** `defmt-bbq` setup is a well-documented embedded Rust pattern.
- **Phase 3:** Adding log macros to existing tasks is straightforward instrumentation.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | `defmt` + `bbqueue` is the industry standard for this use case. |
| Features | HIGH | Based on known pain points in the current blocking implementation. |
| Architecture | HIGH | Producer-Consumer pattern is naturally suited for async logging. |
| Pitfalls | HIGH | ESP32-C3 JTAG and blocking issues are well-documented. |

**Overall confidence:** HIGH

### Gaps to Address

- **Hardware Conflict:** Need to decide on the default log transport (UART vs USB) based on the user's hardware setup. This should be handled via a feature flag or config during implementation.

## Sources

### Primary (HIGH confidence)
- [esp-hal 1.0.0 Release Notes](https://github.com/esp-rs/esp-hal/releases/tag/v1.0.0) — Stable async driver patterns.
- [defmt Documentation](https://defmt.ferrous-systems.com/) — Logging framework specifics.
- [defmt-bbq GitHub](https://github.com/knurling-rs/defmt-bbq) — Non-blocking bridge implementation.

### Secondary (MEDIUM confidence)
- [Artisan Protocol Documentation](https://artisan-roaster-scope.blogspot.com/) — Command structures for filtering.

---
*Research completed: 2026-02-05*
*Ready for roadmap: yes*
