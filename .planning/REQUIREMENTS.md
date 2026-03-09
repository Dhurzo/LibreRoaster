# Requirements: LibreRoaster v5.0

**Defined:** 2026-03-07
**Core Value:** Artisan can read temperatures and control heater/fan during a roast session via serial connection.

## v5.0 Requirements

### Quality Gates

- [x] **QG-01**: User can rely on a reproducible quality baseline (`cargo fmt --check`, curated `clippy`, and test gates) with explicit pass/fail policy.
- [x] **QG-02**: User can enforce a ratcheting quality policy by module criticality (safety/control/protocol first) without blocking lower-risk modules initially.

### Dead Code

- [x] **DC-01**: User can review a module-by-module dead code inventory with risk level and evidence of use/non-use before removals.
- [x] **DC-02**: User can remove dead code in small batches and verify no functional regressions through tests and behavior checks.
- [x] **DC-03**: User can identify and clean unused dependencies through a controlled `machete`/`udeps` workflow with an explicit allowlist for intentional exceptions.

### Rust Best Practices

- [x] **RUST-01**: User can apply mechanical Rust modernization (idioms/clippy/cargo-fix curated pass) with no observable semantic behavior change.
- [x] **RUST-02**: User can audit active `unsafe` attributes/surfaces and maintain updated justification/status for each remaining case.

### SOLID (Pragmatic)

- [x] **SOLID-01**: User can improve separation of responsibilities at high-value seams (handlers/hardware/control boundaries) while preserving safety ordering and loop behavior.
- [x] **SOLID-02**: User can run fault-injection scenarios for watchdog/guard/comms paths and verify expected safe behavior.

### Hardware Real Validation

- [x] **HW-01**: User can define numeric acceptance thresholds for real control behavior (command-to-actuator latency, response envelope, safety counters).
- [x] **HW-02**: User can validate on real hardware that Artisan Scope controls a real roaster with this firmware within the defined thresholds.

## Future Requirements (Deferred)

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
| QG-01 | Phase 81, 87 | Complete |
| QG-02 | Phase 81, 88 | Complete |
| DC-01 | Phase 82 | Complete |
| DC-02 | Phase 82 | Complete |
| DC-03 | Phase 82 | Complete |
| RUST-01 | Phase 83, 87 | Complete |
| RUST-02 | Phase 83 | Complete |
| SOLID-01 | Phase 84, 88 | Complete/Pending |
| SOLID-02 | Phase 84, 86 | Complete |
| HW-01 | Phase 85, 86 | Complete |
| HW-02 | Phase 85 | Complete |

**Coverage:**
- v5.0 requirements: 11 total
- Mapped to phases: 11
- Unmapped: 0

---
*Requirements defined: 2026-03-07*
*Last updated: 2026-03-07 after milestone v5.0 roadmap mapping*
