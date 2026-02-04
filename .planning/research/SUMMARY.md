# LibreRoaster v1.5 Research Summary

**Project:** Full Artisan Serial Protocol Implementation  
**Date:** February 4, 2026  
**Confidence:** HIGH - Protocol well-documented with multiple reference implementations

---

## Executive Summary

LibreRoaster v1.5 aims to implement full bidirectional Artisan serial protocol compatibility on ESP32-C3 firmware. Artisan is the dominant open-source coffee roasting software, communicating via a master-slave polling model at 115200 baud. The existing codebase provides solid foundations including Embassy async task framework, command multiplexer, ArtisanFormatter for READ responses, and RoasterControl with ArtisanCommandHandler.

**Key Research Findings:**

1. **Stack Strategy:** Add `nom` 8.x for robust parser combinator support; existing Embassy+esp-hal stack is fully compatible
2. **Critical Path:** Initialization sequence (CHAN → UNITS → FILT) must complete with `#` acknowledgment before Artisan polls temperatures
3. **Architecture Leverage:** Extend existing parser and formatter rather than replacing them; maintain separation of concerns
4. **Primary Risks:** UART buffer overflow, ISR blocking, and frame format errors are the highest-impact pitfalls

The recommended approach is a phased implementation starting with protocol foundation (handshake + acknowledgments), followed by extended command parsing, response enhancement, PID integration, and streaming optimization. This minimizes risk while building toward full Artisan compatibility.

---

## Key Findings

### From STACK.md: Technology Recommendations

| Component | Recommendation | Rationale |
|-----------|----------------|-----------|
| **Parser Library** | `nom` 8.x | Industry-standard Rust parsing library with no_std support, zero-copy parsing, compositional parsers, and precise error locations |
| **Existing Stack** | Keep as-is | esp-hal, embassy-executor, embassy-time, heapless, embedded-hal are all compatible with no changes |
| **What NOT to Add** | tokio, async-std, serde, regex | Incompatible with no_std embassy, or overkill for ASCII protocol |

**Critical Integration Points:**
- `src/input/parser.rs` → extend with nom-based Artisan command parsing
- `src/output/artisan.rs` → extend ArtisanFormatter with acknowledgment responses
- `src/input/multiplexer.rs` → add initialization state machine

### From FEATURES.md: Feature Landscape

**Table Stakes (MVP Required):**
- Initialization handshake (CHAN → UNITS → FILT with `#` acknowledgment)
- READ command response (ET,BT,ET2,BT2,ambient,fan,heater in CSV format)
- OT1 heater control and IO3 fan control (existing)
- 115200 baud, 8N1, line-oriented protocol with newline termination
- Temperature unit conversion (F/C) and channel mapping support

**Differentiators (Post-MVP):**
- Extended command set (OT2, UP/DOWN, PID mode)
- Rate-of-Rise reporting
- WebSocket bridge for network-based Artisan connection
- MODBUS support for commercial roaster integration
- Autonomous PID control without Artisan connected

**Anti-Features to Avoid:**
- Missing `#` acknowledgment for CHAN (Artisan will hang)
- Response times exceeding 100ms (causes missed samples)
- Using `0` for unused temperature channels (use `-1`)
- Arbitrary serial output breaking line-oriented parsing

### From ARCHITECTURE.md: Structural Analysis

**Existing Architecture Strengths:**
- Embassy async tasks for UART I/O with proper concurrency
- Command multiplexer handling dual-channel (UART + USB CDC)
- Clear separation: Parser → Handler → Formatter → Output
- ServiceContainer for dependency injection

**New Components Required:**
1. `ArtisanProtocolState` - Initialization state machine (ExpectingChan → ExpectingUnits → ExpectingFilt → Ready)
2. `ArtisanResponseBuilder` - Centralized response formatting
3. `ArtisanParser` - Extended delimiter support (`,`, `;`, space, `=`)
4. Extended `ArtisanCommand` enum - PID commands, OT2, DCFAN, channel mapping

**Data Flow (Enhanced):**
```
Artisan → CHAN;1200 → ProtocolState → #ACK → Artisan
Artisan → UNITS;C → ProtocolState → (no response)
Artisan → READ → Parser → Handler → ArtisanFormatter → Response
```

### From PITFALLS.md: Critical Risks

**Critical Pitfalls (Must Prevent):**
1. **UART Buffer Overflow** - Process bytes faster than they arrive using event-driven RX with proper buffer sizing
2. **Blocking in ISR** - Keep UART ISR minimal: copy bytes, signal semaphore, return immediately
3. **Baud Rate Mismatch** - Verify ESP32-C3 clock accuracy; use exact 115200 configuration
4. **Race Conditions** - Protect shared ArtisanFormatter buffers with mutexes
5. **Incorrect Frame Format** - Use `R,TTTT,TTTT,...\r\n` format; always terminate with CRLF

**Moderate Pitfalls:**
- USB CDC conflict with UART0 (use UART1/UART2 for Artisan)
- Temperature data staleness during serial transmission
- Deep sleep breaking Artisan connection (disable during active roast)

---

## Implications for Roadmap

### Suggested Phase Structure

**Phase 1: Protocol Foundation (Week 1)**
- Implement initialization state machine (CHAN → UNITS → FILT)
- Add acknowledgment response system (`#` prefix)
- Create error response format (`ERR code message`)
- Validate UART configuration with Artisan connection

**Phase 2: Extended Command Parsing (Week 2)**
- Integrate `nom` parser with multi-delimiter support
- Add PID command variants (PID;ON, PID;SV, PID;T;...)
- Add OT2 and DCFAN command support
- Extend ArtisanCommand enum with new variants

**Phase 3: Response Enhancement (Week 3)**
- Create ArtisanResponseBuilder for centralized formatting
- Implement ambient temperature in READ responses
- Enhance dual_output_task for acknowledgment routing
- Add Fahrenheit conversion support

**Phase 4: PID Integration (Week 4)**
- Extend ArtisanCommandHandler with PID state
- Implement PID enable/disable and setpoint commands
- Add PID tuning parameter support
- Integrate with existing temperature control algorithms

**Phase 5: Streaming Optimization (Week 5)**
- Optimize CSV formatting for 10Hz streaming
- Implement backpressure handling
- Add streaming start/stop acknowledgment
- Long-duration stability testing

### Research Flags

| Phase | Needs Deeper Research | Standard Patterns |
|-------|----------------------|-------------------|
| Phase 1 | None (protocol spec well-documented) | Initialization sequence, ACK format |
| Phase 2 | None (nom well-documented) | Parser combinator patterns |
| Phase 3 | PID algorithm details from existing codebase | Response builder patterns |
| Phase 4 | YES - PID integration with Artisan requires existing PID implementation review | Extended command patterns |
| Phase 5 | None (standard streaming patterns) | Performance optimization |

### Dependencies Between Phases

```
Phase 1 (Foundation) ──► Phase 2 (Parsing) ──► Phase 3 (Responses)
        │                    │                      │
        ▼                    ▼                      ▼
   Must work first      Requires ACK            Requires parsing
                                              and responses

Phase 3 ──► Phase 4 (PID) ──► Phase 5 (Optimization)
    │            │
    ▼            ▼
Requires    Requires all
parsing     previous phases
```

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| **Stack** | HIGH | nom well-documented, existing stack verified compatible |
| **Features** | HIGH | Protocol specification clear, multiple reference implementations |
| **Architecture** | HIGH | Codebase analyzed, clear integration points identified |
| **Pitfalls** | MEDIUM | Based on ESP32 community sources and embedded patterns; some issues may be ESP32-C3 specific |

**Gaps to Address:**
- PID algorithm details from existing codebase (Phase 4 planning)
- USB CDC pin conflicts with UART0 (needs hardware verification)
- Thermocouple polarity and sign convention (testing phase)

---

## Sources

- **nom parser crate:** https://docs.rs/nom/8.0.0/nom/
- **Artisan protocol spec:** https://github.com/greencardigan/TC4-shield
- **Artisan Official Documentation:** https://artisan-scope.org/devices/arduino/
- **TC4-Emulator Reference:** https://github.com/FilePhil/TC4-Emulator
- **aArtisanQ PID Firmware:** https://github.com/greencardigan/TC4-shield/tree/master/applications/Artisan/aArtisan
- **ESP32 UART Reference:** https://docs.espressif.com/projects/esp-idf/en/latest/api-reference/peripherals/uart.html
- **ESP32 GitHub Issues:** #6326, #10420 (UART data loss issues)
