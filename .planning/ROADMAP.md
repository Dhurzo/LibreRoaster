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
**Status**: In progress (1/3 plans complete, 2026-03-20)
**Plans:** 3 plans

Plans:
- [x] 98-01-PLAN.md — Define the HIL scenario contract, manifest, and retention policy
- [ ] 98-02-PLAN.md — Make validation_runner.py and analysis.py manifest-aware so runs generate packaged evidence
- [ ] 98-03-PLAN.md — Document the workflow, playbook, and README guidance for artifact-backed HIL validation

---

## Coverage

*Coverage table will be updated after next milestone is planned.*

---

*Ready for planning: `/gsd-new-milestone`*
