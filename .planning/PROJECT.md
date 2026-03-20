# LibreRoaster

## What This Is

ESP32-C3 firmware for coffee roaster control with ARTISAN+ serial protocol compatibility. Allows Artisan coffee roasting software to read temperature data and control heater/fan output via UART or USB CDC.

## Core Value

Artisan can read temperatures and control heater/fan during a roast session via serial connection.

## Current Milestone: v5.2 Architecture Hardening & Validation

**Goal:** Harden diagnostics, fix the embedded build flow, and instrument the traceability + HIL automation so the control loop carries reliable metadata end-to-end.

**Status:** ✅ SHIPPED (2026-03-20)

v5.2 delivered: flashable `.bin` artifacts, unified error taxonomy from hardware through AppError, end-to-end TRACE instrumentation, manifest-aware HIL validation, and diagnostics automation that packages safe-shutdown artifacts for auditors.

## Current State

- The embedded build now runs with `cargo build --release --target riscv32imc-unknown-none-elf --features embedded` and produces `libreroaster.bin` after `espflash save-image`.
- `RoasterError`/`AppError` conversions plus mock hardware tests exercise every control boundary and keep guard telemetry annotated with `error_category`/`error_source`.
- TRACE instrumentation and parser cover the queue → actuator → telemetry → guard flow, while HIL manifests + analysis provide golden outputs and the safe-shutdown replay report ensures DIAG-01 coverage.
- Diagnostics scripts (`scripts/replay_safe_shutdown.py`) regenerate the traceability matrix (192-byte buffer) and produce `replay-report.json` for audits.

## Next Milestone Goals

1. `/gsd-new-milestone` to plan the safe-shutdown artifact replay automation (Phase 104) and the next diagnostics/traceability chapter.
2. Keep trace instrumentation, docs, and automation artifacts aligned while the next phase fills in the remaining automation checks.

<details>
<summary>Previous project context</summary>

Historical milestone write-ups (v5.1, v4.5, etc.) remain available in `.planning/MILESTONES.md` and the per-milestone archives.

</details>

---
*Last updated: 2026-03-20 after v5.2 milestone completion*
