# Roadmap: LibreRoaster

## Overview

## Milestones

- 🟡 **v5.2 Architecture Hardening & Validation** — Phases 95-98 (planning 2026-03-20) — *[details](milestones/v5.2-ROADMAP.md)*
- ✅ **v5.1 Actualización de Documentación** — Phases 89-94 (shipped 2026-03-12) — *[details](milestones/v5.1-ROADMAP.md)*
- ✅ **v5.0 Auditoria integral de calidad Rust** — Phases 81-88 (shipped 2026-03-09) — *[details](milestones/v5.0-ROADMAP.md)*
- ✅ **v4.5 Alta Prioridad** — Phases 77-80 (shipped 2026-03-09)
- ✅ **v4.4 SSR Refactoring & Test Stubs** — Phases 75-76 (shipped 2026-02-25) — *[details](milestones/v4.4-ROADMAP.md)*
- ✅ **v4.3 Code Cleanup** — Phases 73-74 (shipped 2026-02-24)
- ✅ **v4.2 Anti-windup integral** — Phases 70-72 (shipped 2026-02-24)
- ✅ **v4.1 Documentation Update** — Phases 62-69 (shipped 2026-02-23) — *[details](milestones/v4.1-ROADMAP.md)*

## Phases

### Phase 96: Error Architecture Implementation
**Goal**: Design and implement cross-module error taxonomy
**Requirements**: RUST-03
**Dependencies**: Phase 95
**Status**: Planned (2026-03-20)
**Plans:** 5 plans

Plans:
- [ ] 96-01-PLAN.md — Add error source chaining to error enums
- [ ] 96-02-PLAN.md — Implement embedded-hal Error traits for hardware errors
- [ ] 96-03-PLAN.md — Eliminate panic-prone initialization paths
- [ ] 96-04-PLAN.md — Create error boundary contracts with From trait implementations
- [ ] 96-05-PLAN.md — Expand error testing infrastructure with mocks and comprehensive tests

---

### Phase 97: Traceability Matrix Tooling
**Goal**: Build the command → queue → actuator → telemetry → guard traceability matrix for regression triage
**Requirements**: SOLID-03
**Dependencies**: Phase 96
**Status**: Verified (2026-03-20)
**Plans:** 3 plans (complete)
**Verification:** 97-VERIFICATION.md ✓

Plans:
- [x] 97-01-PLAN.md — Instrument TraceId propagation and queue/guard logging
- [x] 97-02-PLAN.md — Add host parser script plus sample trace log
- [x] 97-03-PLAN.md — Document the TRACE stream and triage steps

---

### Phase 98: HIL Validation Infrastructure
**Goal**: Establish artifact-backed HIL scenarios with golden outputs
**Requirements**: HW-03
**Dependencies**: Phase 96, Phase 97
**Status**: Verified (2026-03-20)
**Plans:** 3 plans (complete)
**Verification:** 98-hil-validation-infrastructure-VERIFICATION.md ✓

Plans:
- [x] 98-01-PLAN.md — Define the HIL scenario contract, manifest, and retention policy
- [x] 98-02-PLAN.md — Make validation_runner.py and analysis.py manifest-aware so runs generate packaged evidence
- [x] 98-03-PLAN.md — Document the workflow, playbook, and README guidance for artifact-backed HIL validation

---

### Phase 99: Embedded Build Verification
**Goal**: Produce an audited VERIFICATION.md and documentation that prove the embedded build flow claimed in Phase 95 is repeatable for BUILD-01.
**Requirements**: BUILD-01
**Dependencies**: Phase 95, Phase 98
**Status**: Verified (2026-03-20)
**Plans:** 2 plans (complete)

Plans:
- [x] 99-01-PLAN.md — Re-run `cargo build --release --target riscv32imc-unknown-none-elf --features embedded`, capture output/artifacts, and publish a `95-build-verification.md` audit artifact.
- [x] 99-02-PLAN.md — Update README.md and DEVELOPMENT.md with the verified embedded-target build command so the documented workflow matches the audited artifact.

---

### Phase 100: Error Taxonomy Completion
**Goal**: Close the remaining RUST-03 gaps by fixing the compile blockers, wiring AppError diagnostics into telemetry/guards/TRACE, and stabilizing the safe-shutdown flow.
**Requirements**: RUST-03
**Dependencies**: Phase 96
**Status**: Complete (2026-03-20)
**Plans:** 3 plans (complete)
**Verification:** 100-VERIFICATION.md ✓

Plans:
- [x] 100-01-PLAN.md — Convert `RoasterError`/`Max31856Error` to struct variants and clean up unit-variant usage so that codebase compiles without `AppError` dead paths.
- [x] 100-02-PLAN.md — Import and re-export `AppError.source()`/`Display` in telemetry/guard/TRACE modules so that richer diagnostics reach live instrumentation.
- [x] 100-03-PLAN.md — Fix `enter_safe_shutdown()` so it can await embassy_time timers without blocking the LED blink path, then document the recovery flow for verification.

---

### Phase 101: Traceability Matrix Alignment
**Goal**: Align the TRACE regression-triage tooling with the runtime event names so SOLID-03 can consume live logs and the TRACE flow is restorable.
**Requirements**: SOLID-03
**Dependencies**: Phase 97
**Status**: Verified (2026-03-20)
**Plans:** 2 plans (complete)
**Verification:** 101-VERIFICATION.md ✓

Plans:
- [x] 101-01-PLAN.md — Update `scripts/traceability_matrix.py` to filter for the actual TRACE events (`queue_enqueue`, `actuation`, `telemetry`, `guard`) and add regression tests that replay sample logs.
- [x] 101-02-PLAN.md — Document the TRACE matrix generation flow after the parser fix so the regression triage path can be rerun from live captures.

---

### Phase 102: Safe-Shutdown Diagnostics
**Goal**: Trace startup failures end-to-end by emitting guard TRACE events with AppError metadata, publishing a failure log, and documenting how to replay it through the regression parser.
**Requirements**: DIAG-01 (Diagnostics coverage for InitError flows)
**Dependencies**: Phase 101
**Status**: Verified (2026-03-20)
**Plans:** 1 plan (complete)

Plans:
- [x] 102-01-PLAN.md — Emit guard TRACE events when `init_hardware()` fails, add a safe-shutdown sample log, and extend the parser/tests/docs so auditors can rerun the flow.

---

### Phase 103: Safe-Shutdown Replay Artifact
**Goal**: Package safe-shutdown guard TRACE failures into reproducible artifacts (log, trace matrix, metadata) and document how auditors can rerun them without hardware.
**Requirements**: DIAG-01 (Diagnostics coverage for InitError flows)
**Dependencies**: Phase 102
**Status**: Planned (2026-03-20)
**Plans:** 1 plan

Plans:
- [ ] 103-01-PLAN.md — Package the safe-shutdown log into a replay zip, cover the packaging script with regression tests, and document the workflow so auditors can reproduce the guard failure path.

---

## Coverage

*Coverage table will be updated after next milestone is planned.*

---

*Ready for planning: `/gsd-new-milestone`*
