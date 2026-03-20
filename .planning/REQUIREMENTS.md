# Requirements: LibreRoaster v5.2

**Defined:** 2026-03-20
**Updated:** 2026-03-20 (v5.2 planning)
**Core Value:** Artisan can read temperatures and control heater/fan during a roast session via serial connection.

## v5.2 Requirements

### Critical Blocker Resolution

- **BUILD-01**: Fix main.rs compilation issues blocking binary production with --features embedded.

### Rust Best Practices

- **RUST-03**: User can normalize cross-module error taxonomy and boundary contracts for all major subsystems.

### SOLID (Pragmatic)

- **SOLID-03**: User can use an end-to-end traceability matrix (`command -> queue -> actuator -> telemetry -> guard`) for regression triage.

### Hardware Real Validation

- **HW-03**: User can run artifact-backed HIL scenarios with golden outputs and retention policy for release audits.

### Diagnostics Coverage

- **DIAG-01**: Diagnostics coverage for InitError flows.

## v5.1 Requirements (Complete)

### Rust Best Practices

- **RUST-03**: User can normalize cross-module error taxonomy and boundary contracts for all major subsystems.

### SOLID (Pragmatic)

- **SOLID-03**: User can use an end-to-end traceability matrix (`command -> queue -> actuator -> telemetry -> guard`) for regression triage.

### Hardware Real Validation

- **HW-03**: User can run artifact-backed HIL scenarios with golden outputs and retention policy for release audits.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Big-bang architecture rewrite for strict SOLID purity | High regression risk in safety-critical runtime paths |
| Enabling all strict clippy groups globally in one pass | Excessive churn/noise for brownfield hardening milestone |
| Protocol semantic redesign during v5.0 | Would mix behavior change with hardening and blur regressions |
| Full hardware lab orchestration platform | Too large for this milestone; deferred after pragmatic HIL path |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| DIAG-01 | Phase 102 | Complete |
| BUILD-01 | Phase 95 | Complete |
| RUST-03 | Phase 96, 100 | Complete |
| SOLID-03 | Phase 97, Phase 101 | Complete |
| HW-03 | Phase 98 | Complete |
| DOCS-01 | Phase 89 | Complete |
| DOCS-02 | Phase 90 | Complete |
| DOCS-03 | Phase 91 | Complete |

**Coverage:**
- v5.2 requirements: 4 total
- v5.1 requirements: 3 total
- Mapped to phases: 7
- Unmapped: 0

---
*Requirements defined: 2026-03-10*
*Last updated: 2026-03-20 (v5.2 planning)*
