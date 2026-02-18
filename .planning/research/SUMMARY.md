# Project Research Summary: v3.0 Safety Fixes

**Project:** LibreRoaster
**Domain:** Embedded Safety-Critical Control Systems (ESP32-C3 Coffee Roaster Firmware)
**Researched:** 2026-02-18
**Focus:** Architecture integration for Critical Safety Fixes
**Confidence:** HIGH

## Executive Summary

This architecture research focuses on how v3.0 Critical Safety Fixes integrate with the existing LibreRoaster ESP32-C3 firmware. The system implements a layered async architecture using embassy-rs with distributed safety mechanisms rather than a centralized safety component. Safety fixes must work within the existing handler chain pattern, using the ServiceContainer's critical_section for atomic state updates, and extending the dual-verification pattern already present at control-hardware boundaries.

Key integration points identified:
1. **Handler Chain** — RoasterControl::process_command() routes all commands through SafetyCommandHandler first
2. **Cycle Guard** — SsrCycleGuard enforces 1000ms minimum SSR cycle time per datasheet
3. **Temperature Safety** — Instant threshold checks and timeout tracking in control loop
4. **Hardware Status** — Graceful degradation when SSR heat source not detected

## Key Findings

### Architecture Integration Points

| Integration Point | Location | Safety Role |
|-------------------|----------|-------------|
| Handler Chain Extension | `RoasterControl::process_command()` | Command routing with safety-first ordering |
| SystemStatus Fields | `src/config/constants.rs` | New safety fields with fail-safe defaults |
| Emergency Shutdown | `RoasterControl::emergency_shutdown()` | Final safety measure (zero-heat, full-fan) |
| SSR Monitor | `SsrControlSimple` in `src/hardware/ssr.rs` | Duty verification and retry logic |

### Critical Patterns to Follow

1. **Safety First in Handler Chain** — SafetyCommandHandler must remain first in the chain
2. **Dual Verification** — Both command-level AND hardware-level validation
3. **Fail-Safe Defaults** — New state fields initialize to safe values (0%, OFF, disabled)
4. **Graceful Degradation** — Check hardware status before enabling heating
5. **Atomic State Updates** — Use ServiceContainer::with_roaster() for multi-field updates

### Critical Anti-Patterns to Avoid

1. **Bypassing Handler Chain** — Direct hardware calls skip safety checks
2. **Non-Atomic Updates** — Multi-field safety state must update atomically
3. **Ignoring Hardware Status** — Must check SSR availability before heating
4. **Swallowing Safety Errors** — Safety errors require immediate response, not logging

## Implications for Roadmap

Based on research, suggested phase structure for v3.0:

### Phase 1: Safety Handler Extensions
**Rationale:** All safety fixes flow through the handler chain; extending it is foundational
**Delivers:** New safety validation logic in RoasterControl, extended SafetyCommandHandler
**Addresses:** Input validation, command range checking
**Avoids:** Anti-pattern 1 (bypassing handler chain)

### Phase 2: Hardware Boundary Fixes
**Rationale:** SSR reliability fixes require hardware-level verification
**Delivers:** Extended duty verification, enhanced status reporting in SSR driver
**Addresses:** SSR hardware monitoring, retry logic
**Avoids:** Anti-pattern 3 (ignoring hardware status)

### Phase 3: State Management
**Rationale:** Safety state tracking requires atomic updates
**Delivers:** New SystemStatus fields, atomic update patterns
**Addresses:** Safety state telemetry, fault tracking
**Avoids:** Anti-pattern 2 (non-atomic updates)

### Phase Ordering Rationale
- Handler extensions gate all command processing → must be first
- Hardware fixes depend on understanding handler flow → second
- State management builds on both previous phases → third

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Verified via source code analysis of existing codebase |
| Features | HIGH | Mapped from existing control flow and handlers |
| Architecture | HIGH | Direct code path analysis of safety-critical sections |
| Pitfalls | HIGH | Based on embedded best practices applied to this specific codebase |

**Overall confidence:** HIGH

### Gaps to Address

- **ESP32 Hardware Watchdog:** May need research if software-based SsrCycleGuard is insufficient for v3.0 safety requirements
- **Rate-of-change Temperature Limits:** Could require additional phase if instant-threshold checks are insufficient

## Sources

### Primary (HIGH confidence)
- LibreRoaster source code (`src/control/roaster_refactored.rs`, `src/control/handlers.rs`, `src/hardware/ssr.rs`)
- ESP-IDF Watchdog Timer documentation (https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/wdts.html)
- Embassy-rs framework (https://github.com/embassy-rs/embassy)

### Secondary (MEDIUM confidence)
- Embedded safety design principles (https://incompliancemag.com/implementing-robust-watchdog-timers-for-embedded-systems/)

---

*Research completed: 2026-02-18*
*Ready for roadmap: yes*
