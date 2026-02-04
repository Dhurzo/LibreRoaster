# LibreRoaster

## What This Is

ESP32-C3 firmware for coffee roaster control with ARTISAN+ serial protocol compatibility. Allows Artisan coffee roasting software to read temperature data and control heater/fan output via UART.

## Core Value

Artisan can read temperatures and control heater/fan during a roast session.

## Current State

v1.2 shipped: core Artisan command hardening, deterministic formatting, and mock UART end-to-end integration tests. Host-target testing is enabled via an `embedded` feature gate and host stubs for embedded-only modules.

## Next Milestone Goals

TBD — define with `/gsd-new-milestone`.

<details>
<summary>v1.2 Milestone Details (Archived)</summary>

**Goal:** Polish Artisan integration with full protocol coverage, robust formatting, and verified end-to-end responses.

**Target features:**
- Complete remaining Artisan/Artisan+ command coverage, including status/error responses
- Harden Artisan formatter against edge cases and invalid input for strict spec compliance
- Add end-to-end integration tests (mock UART) for command → response flow
</details>

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
- ✓ Unused output modules removed (serial, uart, manager, scheduler) — v1.1 cleanup
- ✓ Unused control modules removed (command_handler, pid, abstractions_tests) — v1.1 cleanup
- ✓ OutputManager trait consolidated and exports cleaned — v1.1 cleanup
- ✓ Build verified after cleanup — v1.1 cleanup
- ✓ Core command hardening with explicit ERR handling — v1.2
- ✓ Deterministic formatter outputs and ERR schema — v1.2
- ✓ Mock UART end-to-end integration tests — v1.2

### Active

(None — define next milestone requirements)

### Out of Scope

- Hardware testing (actual ESP32 + roaster)
- Roast automation or new device features beyond Artisan protocol polish

## Context

Brownfield ESP32-C3 Rust embedded project using embassy-rs framework.

**v1.0 shipped:** Comprehensive test infrastructure for ARTISAN+ protocol.

**v1.1 cleanup:** Removed unused modules and consolidated output abstractions; build verified clean post-removal.

**Current focus:** Planning the next milestone and defining the next protocol goals.

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
| Standardize `ERR <code> <message>` schema | Stable parsing for Artisan clients and tests | ✓ Implemented |
| Gate embedded binary behind `embedded` feature | Enable host-target integration tests | ✓ Implemented |

---

*Last updated: 2026-02-04 after v1.2 milestone completion*
