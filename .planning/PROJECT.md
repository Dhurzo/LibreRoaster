# LibreRoaster

## What This Is

ESP32-C3 firmware for coffee roaster control with ARTISAN+ serial protocol compatibility. Allows Artisan coffee roasting software to read temperature data and control heater/fan output via UART or USB CDC.

## Core Value

Artisan can read temperatures and control heater/fan during a roast session via serial connection.

## Current Milestone: v5.4 Architecture Decomposition & Quality Fixes

**Goal:** Resolve 4 deferred architectural and quality issues identified during the v5.1 code quality review: SRP violation in RoasterControl, DIP violation in ServiceContainer, 24 pre-existing clippy issues, and 1 broken test.

**Target outcomes:**
- RoasterControl decomposed into focused, single-responsibility controllers
- ServiceContainer singleton replaced with constructor dependency injection
- Zero clippy warnings on both ESP32 and host targets
- All 244 tests passing (including fixed ssr_scheduler test)
- Artisan protocol 100% compatible — no behavioral changes

## Requirements

### Validated

- ✓ Artisan can read roaster telemetry over the serial protocol during a roast session.
- ✓ Artisan can control heater and fan outputs through the firmware command path.
- ✓ The project ships embedded diagnostics, traceability, and HIL-oriented tooling that support validation and audits.
- ✓ v5.1 code quality review completed — 12 improvements committed (heapless hot path, SAFETY docs, error propagation fixes)

### Active

- [ ] Decompose RoasterControl into focused controllers (SRP fix)
- [ ] Replace ServiceContainer singleton with constructor dependency injection (DIP fix)
- [ ] Fix all 24 pre-existing clippy warnings on ESP32 target
- [ ] Fix broken ssr_scheduler test (guard_rejects_commands_while_busy)

### Out of Scope

- Net-new product features — this is purely architectural cleanup
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
