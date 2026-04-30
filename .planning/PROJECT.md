# LibreRoaster

## What This Is

ESP32-C3 firmware for coffee roaster control with ARTISAN+ serial protocol compatibility. Allows Artisan coffee roasting software to read temperature data and control heater/fan output via UART or USB CDC.

## Core Value

Artisan can read temperatures and control heater/fan during a roast session via serial connection.

## Current Milestone: v0.1 — First Working Version

**Goal:** Firmware compiles, flashes onto ESP32-C3, and boots without panics. USB CDC responds to Artisan READ with TC4-format temperatures. Control loop runs continuously at ~160ms per cycle.

**Target outcomes:**
- Firmware boots on ESP32-C3 hardware with no panics or watchdog resets
- All hardware inits: dual MAX31856 thermocouples, SSR heater, variable-speed fan, RTC WDT
- USB CDC (ttyACM0) responds to READ command with real temperatures
- Artisan protocol 100% compatible — READ returns `AMB,ET,BT,0.0,0.0`
- UART0 (GPIO20/21) driver properly initialized for future communication
- Host tests pass for command multiplexer and Artisan protocol

## Requirements

### Validated

- ✓ Artisan can read roaster telemetry over USB CDC serial protocol.
- ✓ Firmware boots stably without panics or watchdog resets.
- ✓ USB CDC uses async (non-blocking) I/O — executor not blocked.
- ✓ All 7 embassy tasks spawn successfully (UART, USB, queue processors, dual output, control loop, regression).
- ✓ Control loop cycles at ~160ms with stable SensorRead, ControlUpdate, LedcWrite, WatchdogFeed timing.
- ✓ x86_64 host builds and tests pass.

### Active

- [ ] Connect thermocouple and verify temperature readings > 0.0°C
- [ ] Test Artisan WRITE/OT1/OT2 commands for heater and fan control
- [ ] Verify RTC WDT recovery under fault conditions

### Out of Scope

- Net-new product features beyond core stability — focus is reliable baseline
- Graphical UI or display support
- Changing Artisan protocol behavior — all responses must remain byte-identical
- Refactoring test infrastructure beyond the broken test fix

## Current State

- v5.1 code quality review committed (0a43b46) with 12 improvements across 17 files
- ESP32 build compiles clean (zero warnings with our changes)
- 243/244 host tests pass (1 pre-existing failure in ssr_scheduler)
- 24 pre-existing clippy issues identified across 7 files
- RoasterControl has 28+ methods spanning 6 responsibilities (SRP violation)
- ServiceContainer uses static_cell singleton with 6+ call sites (DIP violation)

## Context

- LibreRoaster is a brownfield embedded firmware — changes must be backward-compatible
- Embassy async framework uses cooperative multitasking — Send bounds are critical
- The heapless hot path conversion (v5.1) changed OutputFormatter trait — decomposition must preserve this
- Clippy config denies: unwrap_used, expect_used, panic in production code

## Constraints

- **Scope**: Architectural refactoring only — no behavioral changes
- **Delivery**: Code changes + passing tests + clean clippy on both targets
- **Quality bar**: `cargo build --release --target riscv32imc-unknown-none-elf --features embedded` must be warning-free
- **Test bar**: All 244 tests must pass after each phase

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Decompose RoasterControl before ServiceContainer DI | DI depends on controller interfaces existing first | — Pending |
| Fix clippy + test first (independent of architecture) | Quick wins that don't conflict with decomposition | — Pending |
| Use trait objects for controller interfaces | Enables DI without changing Embassy task signatures | — Pending |
| Preserve Artisan protocol byte-for-byte | Roaster must remain 100% Artisan+ compatible | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

<details>
<summary>Previous project context</summary>

### v5.3 Deep Bug Analysis & Defect Report
**Goal:** Audit the whole repository to identify likely bugs, rank their criticity, and produce an implementation-ready defect report for a follow-up milestone.

### v5.2 Architecture Hardening & Validation (Shipped 2026-03-20)
Phases 95-103. Flashable embedded build, unified error taxonomy, TRACE instrumentation, manifest-aware HIL validation, safe-shutdown replay.

Historical milestone write-ups available in `.planning/MILESTONES.md`.

</details>

---
*Last updated: 2026-04-22 after v5.4 milestone start*
