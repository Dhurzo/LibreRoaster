# LibreRoaster

## What This Is

ESP32-C3 firmware for coffee roaster control with ARTISAN+ serial protocol compatibility. Allows Artisan coffee roasting software to read temperature data and control heater/fan output via UART.

## Core Value

Artisan can read temperatures and control heater/fan during a roast session.

## Current Milestone: v1.1 Cleanup

**Goal:** Remove unused code, consolidate duplicates, and fix warnings for a cleaner codebase.

**Target cleanup:**
- Delete unused output modules (serial.rs, uart.rs, manager.rs, scheduler.rs)
- Delete unused control modules (command_handler.rs, pid.rs, abstractions_tests.rs)
- Consolidate OutputManager trait
- Simplify error handling
- Fix all compiler warnings

## Requirements

### Validated

- ✓ ARTISAN+ command parsing (OT1, IO3) — v1.0
- ✓ Parser boundary value handling (0, 100) — v1.0
- ✓ ArtisanFormatter READ response format — v1.0
- ✓ MutableArtisanFormatter CSV output — v1.0
- ✓ ROR calculation from BT history — v1.0
- ✓ Integration test infrastructure — v1.0
- ✓ Mock UART driver — v1.0
- ✓ Example file with correct API usage — v1.0

### Active

- [ ] Remove unused output modules (serial.rs, uart.rs, manager.rs, scheduler.rs)
- [ ] Remove unused control modules (command_handler.rs, pid.rs, abstractions_tests.rs)
- [ ] Consolidate OutputManager trait in control/abstractions.rs
- [ ] Simplify error handling architecture
- [ ] Fix unused import warnings
- [ ] Verify build succeeds after cleanup

### Out of Scope

- Hardware testing (actual ESP32 + roaster)
- New features or functionality changes

## Context

Brownfield ESP32-C3 Rust embedded project using embassy-rs framework.

**v1.0 shipped:** Comprehensive test infrastructure for ARTISAN+ protocol.

**Codebase issues identified:**
- 4 unused output modules (~700 lines
- 3 unused control modules
- Duplicate OutputManager trait
- Multiple error types

## Constraints

- **Protocol**: ARTISAN+ standard serial protocol
- **Baud rate**: 115200 (typical for Artisan)
- **Pins**: UART_TX=20, UART_RX=21
- **Commands**: READ, START, STOP, OT1 (0-100), IO3 (0-100)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| UART for Artisan communication | Standard approach for ESP32 artisan integration | ✓ Verified structurally |
| Test boundary values (0, 100) | Critical for safety - heater/fan edge cases | ✓ Implemented |
| Mock UART for integration tests | Hardware not available, enables confidence | ✓ Implemented |
| Standalone Rust verification | Embedded test framework unavailable | ✓ Worked around |
| Fixed format_artisanLine comma bug | CSV compliance required | ✓ Fixed |

---

*Last updated: 2026-02-04 after v1.0 milestone*
