# LibreRoaster

## What This Is

ESP32-C3 firmware for coffee roaster control with ARTISAN+ serial protocol compatibility. Allows Artisan coffee roasting software to read temperature data and control heater/fan output via UART.

## Core Value

Artisan can read temperatures and control heater/fan during a roast session.

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

_(None — awaiting next milestone)_

### Out of Scope

- Hardware testing (actual ESP32 + roaster)
- Firmware modifications or new features

## Context

Brownfield ESP32-C3 Rust embedded project using embassy-rs framework.

**v1.0 shipped:** Comprehensive test infrastructure for ARTISAN+ protocol.

Key files:
- `src/input/parser.rs` — 13 tests
- `src/output/artisan.rs` — 9 tests + bug fix
- `tests/artisan_integration_test.rs` — 8 tests
- `tests/mock_uart.rs` — 7 tests
- `examples/artisan_test.rs` — 179 lines

**Note:** Integration tests verified structurally. Requires ESP32-C3 toolchain to execute.

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
