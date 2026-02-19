# LibreRoaster

## What This Is

ESP32-C3 firmware for coffee roaster control with ARTISAN+ serial protocol compatibility. Allows Artisan coffee roasting software to read temperature data and control heater/fan output via UART or USB CDC.

## Core Value

Artisan can read temperatures and control heater/fan during a roast session via serial connection.

## Current Milestone: vNext

**Goal:** TBD - run /gsd-new-milestone to define next milestone

## Last Shipped: v3.0 Critical Safety Fixes (2026-02-19)

v3.0 fixed critical safety issues: Use-After-Free bug, unsafe statics (replaced with StaticCell), OT2 parser test, README documentation, async MAX31856 temperature reading, and separate LEDC timers for SSR/Fan.

## Next Milestone

TBD (run /gsd-new-milestone to define)

## Current State

v3.0 shipped: StaticCell patterns eliminate unsafe statics, async temperature reading, separate LEDC timers, fan telemetry fixed. Ready for next milestone.

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
- ✓ SSR-01: Saturating SSR duty conversion 0-100 → LEDC 0-255 — v2.6
- ✓ SSR-02: SSR cycle guard (≥1s) enforcement — v2.6
- ✓ SSR-03: LEDC drift monitoring (±2 ticks) with retry — v2.6
- ✓ FAN-01: FanController writes LEDC duty directly — v2.6
- ✓ FAN-02: Fan/SSR LEDC writes serialized via LedcBus — v2.6
- ✓ IO-01: Async UART with embassy traits and event queues — v2.6
- ✓ IO-02: USB CDC back-pressure handling — v2.6
- ✓ IO-03: CommandQueue FIFO with reject-on-full — v2.6
- ✓ TEST-02: Transport flood tests — v2.6
- ✓ SSR-01: Saturating SSR duty conversion 0-100 → LEDC 0-255 — v2.6
- ✓ SSR-02: SSR cycle guard (≥1s) enforcement — v2.6
- ✓ SSR-03: LEDC drift monitoring (±2 ticks) with retry — v2.6
- ✓ FAN-01: FanController writes LEDC duty directly — v2.6
- ✓ FAN-02: Fan/SSR LEDC writes serialized via LedcBus — v2.6
- ✓ IO-01: Async UART with embassy traits and event queues — v2.6
- ✓ IO-02: USB CDC back-pressure handling — v2.6
- ✓ SAFE-01: Replace make_static Use-After-Free in main.rs with StaticCell — v3.0
- ✓ SAFE-02: Replace mutable static in USB CDC driver with StaticCell — v3.0
- ✓ SAFE-03: Replace mutable static in UART driver with StaticCell — v3.0
- ✓ SAFE-04: Replace ServiceContainer::get_instance() static mut with StaticCell — v3.0
- ✓ TEST-01: Fix test_parse_ot2_partial_command - returns InvalidValue — v3.0
- ✓ DOCS-01: Update README.md to 4-value format (ET,BT,HEATER,FAN) — v3.0
- ✓ PERF-01: Async MAX31856 temperature reading with embassy-time Timer — v3.0
- ✓ PERF-02: Separate SSR and Fan LEDC timers — v3.0

### Active

- [ ] (Run /gsd-new-milestone to define next requirements)

### Out of Scope

- Hardware testing (actual ESP32 + roaster) — requires physical hardware
- PID control implementation
- Roast profile automation
- WiFi/Web UI
- Telemetry channel for SSR/fan duty versus Artisan commands — deferred until hardware reliability proves stable
- Dynamic PWM frequency reconfiguration across board variants — future milestone

## Context

Brownfield ESP32-C3 Rust embedded project using embassy-rs framework.

**v1.0 shipped:** Core Artisan protocol implementation with test infrastructure.

**v1.1 cleanup:** Removed unused modules and consolidated abstractions.

**v1.2 polish:** Hardened commands and formatted outputs.

**v1.3 verification:** USB CDC dual-channel implementation.

**v1.5 complete:** Full Artisan serial protocol with READ, OT1, IO3, UP, DOWN, START, STOP commands.

**v1.7 complete:** Non-blocking logging infrastructure with defmt + bbqueue + UART drain task.

**v2.0 complete:** Code quality audit with clippy/geiger configuration and 31-issue inventory.

**v2.6 focus:** Fix SSR duty math (no double division), FanController LEDC updates, and asynchronous UART/USB CDC transports so hardware output is deterministically driven.

**v3.0 focus:** Critical safety fixes - Use-After-Free, unsafe statics, test failures, documentation fixes, blocking I/O fixes.

**v3.0 shipped:** All 8 requirements complete with StaticCell patterns, async temperature, separate LEDC timers.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Dual clippy config (Cargo.toml + clippy.toml) | Portability + project-specific thresholds | ✓ Configured |
| allow-unwrap-in-tests=true | Tests can use unwrap for test logic | ✓ Configured |
| Grep-based unsafe analysis | cargo-geiger embedded feature complexity | ✓ Documented 22 blocks |
| cargo unsafe-check alias | Avoid cargo-geiger shadowing | ✓ Working |
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
| Saturating SSR duty conversion | Fix double-division, clamp to LEDC 0-255 | ✓ Implemented v2.6 |
| Shared LedcBus with serialization | SSR and Fan share timer via atomic guard | ✓ Implemented v2.6 |
| Embassy async UART/USB transports | Non-blocking with back-pressure | ✓ Implemented v2.6 |

## Constraints

- **Protocol**: ARTISAN+ standard serial protocol
- **Baud rate**: 115200 (typical for Artisan)
- **Pins**: UART_TX=20, UART_RX=21
- **Commands**: READ, START, STOP, OT1 (0-100), IO3 (0-100), UP, DOWN
- **USB**: Native USB CDC (USB Serial JTAG)
- **LEDC**: 25 kHz, 8-bit timers shared between SSR and fan with serialized access

---

*Last updated: 2026-02-19 — v3.0 milestone complete*
