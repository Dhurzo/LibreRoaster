# LibreRoaster

## What This Is

ESP32-C3 firmware for coffee roaster control with ARTISAN+ serial protocol compatibility. Allows Artisan coffee roasting software to read temperature data and control heater/fan output via UART or USB CDC.

## Core Value

Artisan can read temperatures and control heater/fan during a roast session via serial connection.

## Current Milestone: v0.1 — First Working Version

> ✅ **v0.1 released 2026-04-30** and **v5.4 completed 2026-04-22** (architecture decomposition, ServiceContainer DI, clippy cleanup). Since then the V2-series / Bug B1-B36 / p1-p12 hardening arc (2026-07-22 → 08-03) shipped. For the current project state read `CONTEXT.md` (repo root).

**Goal:** Firmware compiles, flashes onto ESP32-C3, and boots without panics. USB CDC responds to Artisan READ with TC4-format temperatures. Control loop runs continuously (100 ms timer; real tick ≈ 310-330 ms with the MAX31856 conversion wait).

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
- ✓ All 5 embassy tasks spawn successfully (UART reader, USB reader, dual output, control loop, regression).
- ✓ Control loop ticks at ~310-330ms (100 ms timer + 210 ms MAX31856 conversion wait) with stable SensorRead, ControlUpdate, LedcWrite, WatchdogFeed timing.
- ✓ x86_64 host builds and tests pass.

### Active

- Connect thermocouples and verify real readings (hardware-validation milestone — needs a physical roaster)
- End-to-end roast with real Artisan + heater/fan hardware (not yet validated on real hardware)

### Out of Scope

- Net-new product features beyond core stability — focus is reliable baseline
- Graphical UI or display support
- Changing Artisan protocol behavior — all responses must remain byte-identical
- Refactoring test infrastructure beyond the broken test fix

## Current State

- ✅ v0.1 released (2026-04-30); v5.4 architecture decomposition completed (2026-04-22)
- ✅ RoasterControl decomposed into SensorController / ActuatorController (heater + fan together) / SafetyController / CommandDispatcher
- ✅ ServiceContainer uses constructor injection (single async-mutex slot; channels + multiplexer are module-level statics)
- ✅ All 631 host tests pass (2026-08-04, `--features test`); clippy clean on both targets
- ✅ V2-series / Bug B1-B36 / p1-p12 hardening arc shipped 2026-07-22 → 08-03
- ⏳ Real-hardware validation (thermocouples, heater, fan, real Artisan) still pending

## Context

- LibreRoaster is a brownfield embedded firmware — changes must be backward-compatible
- Embassy async framework uses cooperative multitasking — Send bounds are critical
- The heapless hot path conversion (v5.1) changed OutputFormatter trait — decomposition must preserve this
- Clippy config denies: unwrap_used, expect_used, panic in production code

## Constraints

- **Scope**: Architectural refactoring only — no behavioral changes
- **Delivery**: Code changes + passing tests + clean clippy on both targets
- **Quality bar**: `cargo build --release --target riscv32imc-unknown-none-elf --features embedded` must be warning-free
- **Test bar**: All host tests must pass after each phase (`cargo test --target x86_64-unknown-linux-gnu --features test --lib --tests --no-fail-fast`; 631 as of 2026-08-04)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Decompose RoasterControl before ServiceContainer DI | DI depends on controller interfaces existing first | ✅ Done (2026-04-22) |
| Fix clippy + test first (independent of architecture) | Quick wins that don't conflict with decomposition | ✅ Done (2026-04-22) |
| Use trait objects for controller interfaces | Enables DI without changing Embassy task signatures | ✅ Done (2026-04-22) |
| Preserve Artisan protocol byte-for-byte | Roaster must remain 100% Artisan+ compatible | ✅ Done — responses verified identical |

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
*Last updated: 2026-08-04 — status refreshed for the v0.1/v5.4/V2-arc reality*
