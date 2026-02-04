# LibreRoaster

## What This Is

ESP32-C3 firmware for coffee roaster control with ARTISAN+ serial protocol compatibility. Allows Artisan coffee roasting software to read temperature data and control heater/fan output via UART.

## Core Value

Artisan can read temperatures and control heater/fan during a roast session.

## Requirements

### Validated

(None yet — test deployment to verify)

### Active

- [ ] Verify ARTISAN+ command parsing works correctly
- [ ] Verify Artisan output formatting matches protocol
- [ ] Verify READ command response format
- [ ] Verify heater control (OT1) parsing and limits
- [ ] Verify fan control (IO3) parsing and limits
- [ ] Verify emergency stop (STOP) command
- [ ] Verify roast start (START) command

### Out of Scope

- Hardware testing (actual ESP32 + roaster)
- Firmware modifications or new features

## Context

Brownfield ESP32-C3 Rust embedded project using embassy-rs framework. ARTISAN+ integration exists in:
- `src/input/parser.rs` — Command parsing
- `src/output/artisan.rs` — Response formatting
- `src/hardware/uart/` — UART communication

Existing test coverage in parser.rs is minimal. Example at `examples/artisan_test.rs` has incorrect method signatures.

## Constraints

- **Protocol**: ARTISAN+ standard serial protocol
- **Baud rate**: 115200 (typical for Artisan)
- **Pins**: UART_TX=20, UART_RX=21
- **Commands**: READ, START, STOP, OT1 (0-100), IO3 (0-100)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| UART for Artisan communication | Standard approach for ESP32 artisan integration | — Pending verification |

---

*Last updated: 2026-02-04 after initial project setup*
