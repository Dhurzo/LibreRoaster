# LibreRoaster

## What This Is

ESP32-C3 firmware for coffee roaster control with ARTISAN+ serial protocol compatibility. Allows Artisan coffee roasting software to read temperature data and control heater/fan output via UART or USB CDC.

## Core Value

Artisan can read temperatures and control heater/fan during a roast session via serial connection.

## Current Milestone: Planning Next Milestone

**Goal:** Define v2.6 requirements and roadmap

## Last Shipped: v2.5 Artisan Edge Cases (2026-02-17)

v2.5 fixes Artisan READ framing edge cases, corrects ROR timing/reset behavior, and adds regression coverage for both.

## Next Milestone

TBD (define in next milestone planning)

## Next Milestone Goals

- ROR resets cleanly on START/STOP and formatter reset (ROR-02)
- Dual output routing tests validate terminator behavior across USB CDC and UART (TEST-02)
- Protocol fuzz tests for malformed command framing (TEST-03)

## Current State

v2.5 shipped: exact READ framing with single CRLF, ROR timing stabilized, and regression tests added.
Tech debt: integration tests still call format_read_response instead of runtime format_read_response_full.

<details>
<summary>Previous State</summary>

v2.0 Code Quality Audit — Complete. Technical debt inventory finished with 31 issues identified (1 High, 7 Medium, 23 Low).

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
- ✓ Unused output modules removed — v1.1 cleanup
- ✓ Unused control modules removed — v1.1 cleanup
- ✓ OutputManager trait consolidated — v1.1 cleanup
- ✓ Build verified after cleanup — v1.1 cleanup
- ✓ Core command hardening with explicit ERR handling — v1.2
- ✓ Deterministic formatter outputs and ERR schema — v1.2
- ✓ Mock UART end-to-end integration tests — v1.2
- ✓ Dual-channel Artisan support (USB CDC + UART0) — v1.3
- ✓ Command multiplexer with 60s timeout — v1.3
- ✓ USB CDC port appears and Artisan can connect — v1.3
- ✓ Initialization handshake (CHAN→UNITS→FILT) — v1.5
- ✓ READ command with 7-value telemetry — v1.5
- ✓ UP/DOWN incremental heater control — v1.5
- ✓ Comprehensive error handling (ERR format) — v1.5
- ✓ Parser recovery for partial commands — v1.5
- ✓ Complete documentation update — v1.6
- ✓ Non-blocking logging infrastructure — v1.7
- ✓ Defmt + bbqueue foundation — v1.7
- ✓ UART drain task for async logging — v1.7
- ✓ USB traffic sniffing with log_channel! macro — v1.7
- ✓ Flash instructions for ESP32-C3 — v1.8
- ✓ Artisan connection setup guide — v1.8
- ✓ Command reference for end users — v1.8
- ✓ UART logging usage guide — v1.8
- ✓ Troubleshooting common issues — v1.8
- ✓ Quick start reference card — v1.8
- ✓ Clippy configuration for embedded Rust — v2.0
- ✓ cargo-geiger unsafe code baseline (22 blocks) — v2.0
- ✓ Code quality issues inventory (31 issues) — v2.0
- ✓ Severity classification and remediation priorities — v2.0
- ✓ Comment rationale cleanup — v2.1
- ✓ OT2 command parsing with safety measures — v2.2
- ✓ READ telemetry with CSV format — v2.2
- ✓ BT2/ET2 disabled channel documentation — v2.2
- ✓ UNITS temperature scale parsing — v2.2
- ✓ ARCHITECTURE.md v2.2 command flows documented — v2.3
- ✓ PROTOCOL.md complete Artisan specification — v2.3
- ✓ CODE_QUALITY_ISSUES.md corrected (24 unsafe blocks) — v2.3
- ✓ hardware.md v2.2 specifications verified — v2.3
- ✓ Documentation cross-references validated — v2.3
- ✓ PROT-01: READ response terminates with exactly one CRLF — v2.5
- ✓ PROT-02: READ response is a 4-value CSV with one-decimal precision — v2.5
- ✓ ROR-01: delta_bt updates last_bt so ROR becomes non-zero after the second BT sample — v2.5
- ✓ ARCH-01: A centralized terminator policy appends CRLF at a single output boundary — v2.5
- ✓ TEST-01: Tests cover READ terminator and ROR update behavior — v2.5

### Active (Next Milestone TBD)

- [ ] ROR-02: ROR resets cleanly on START/STOP and formatter reset
- [ ] TEST-02: Dual output routing tests validate terminator behavior across USB CDC and UART
- [ ] TEST-03: Protocol fuzz tests for malformed command framing

### Out of Scope

- Hardware testing (actual ESP32 + roaster) — requires physical hardware
- PID control implementation
- Roast profile automation
- WiFi/Web UI

## Context

Brownfield ESP32-C3 Rust embedded project using embassy-rs framework.

**v1.0 shipped:** Core Artisan protocol implementation with test infrastructure.

**v1.1 cleanup:** Removed unused modules and consolidated abstractions.

**v1.2 polish:** Hardened commands and formatted outputs.

**v1.3 verification:** USB CDC dual-channel implementation.

**v1.5 complete:** Full Artisan serial protocol with READ, OT1, IO3, UP, DOWN, START, STOP commands.

**v1.7 complete:** Non-blocking logging infrastructure with defmt + bbqueue + UART drain task.

**v2.0 complete:** Code quality audit with clippy/geiger configuration and 31-issue inventory.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Dual clippy config (Cargo.toml + clippy.toml) | Portability + project-specific thresholds | ✓ Configured |
| allow-unwrap-in-tests=true | Tests can use unwrap for test logic | ✓ Configured |
| Grep-based unsafe analysis | cargo-geiger embedded feature complexity | ✓ Documented 22 blocks |
| cargo unsafe-check alias | Avoid cargo-geiger shadowing | ✓ Working |

## Constraints

- **Protocol**: ARTISAN+ standard serial protocol
- **Baud rate**: 115200 (typical for Artisan)
- **Pins**: UART_TX=20, UART_RX=21
- **Commands**: READ, START, STOP, OT1 (0-100), IO3 (0-100), UP, DOWN
- **USB**: Native USB CDC (USB Serial JTAG)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| UART for Artisan communication | Standard approach for ESP32 artisan integration | ✓ Verified |
| USB CDC as primary channel | Native USB, no external adapter needed | ✓ Implemented |
| Multiplexer with timeout | Graceful channel switching | ✓ Implemented |
| First command wins priority | Simple, predictable behavior | ✓ Implemented |
| USB + UART dual support | Maximum flexibility for users | ✓ Implemented |
| UP/DOWN clamping | No error at boundaries, just clamp | ✓ Implemented |
| Unused READ channels = -1 | Per Artisan spec | ✓ Implemented |
| OT2 decimal rounding | Round to nearest integer (50.5 → 51) | ✓ Implemented v2.2 |
| OT2 heater stop on out-of-range | Safety measure for invalid fan values | ✓ Implemented v2.2 |
| READ one-decimal format | Consistent with Artisan spec (75.0) | ✓ Implemented v2.2 |
| UNITS parse only, no conversion | Temperatures stay Celsius internally | ✓ Implemented v2.2 |
| Centralized CRLF termination at output boundary | Prevent double terminators across USB CDC/UART | ✓ Implemented v2.5 |
| Reset formatter on START/STOP transitions | Avoid stale ROR state across sessions | ✓ Implemented v2.5 |

---

*Last updated: 2026-02-17 — v2.5 milestone complete*
