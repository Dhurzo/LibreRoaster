# LibreRoaster

## What This Is

ESP32-C3 firmware for coffee roaster control with ARTISAN+ serial protocol compatibility. Allows Artisan coffee roasting software to read temperature data and control heater/fan output via UART or USB CDC.

## Core Value

Artisan can read temperatures and control heater/fan during a roast session via serial connection.

## Current Milestone: v2.1 Comment Rationale Cleanup

**Goal:** Remove comments that don't add relevant information. Keep only rationale comments (explaining WHY), not WHAT comments.

**Scope:** All Rust source files in src/

**Target:**
- Remove obvious/verbose comments that duplicate code
- Keep design rationale and non-obvious explanations
- Reference documentation (internalDoc/*.md) for extended context

## Current State

v2.0 Code Quality Audit — Complete. Technical debt inventory finished with 31 issues identified (1 High, 7 Medium, 23 Low).

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
- ✓ Comment rationale cleanup — v2.1 (in progress)

### Active

(None — v2.0 complete, next milestone TBD)

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

---

*Last updated: 2026-02-06 — v2.1 Comment Rationale Cleanup started*
