# Roadmap: LibreRoaster

## Overview

v5.0 is the active quality-hardening milestone focused on audit-grade cleanup, Rust/SOLID refactoring discipline, and real-hardware validation without changing protocol semantics.

## Milestones

- ✅ **v4.1 Documentation Update** — Phases 62-69 (shipped 2026-02-23) — *[details](milestones/v4.1-ROADMAP.md)*
- ✅ **v4.2 Anti-windup integral** — Phases 70-72 (shipped 2026-02-24)
- ✅ **v4.3 Code Cleanup** — Phases 73-74 (shipped 2026-02-24)
- ✅ **v4.4 SSR Refactoring & Test Stubs** — Phases 75-76 (shipped 2026-02-25) — *[details](milestones/v4.4-ROADMAP.md)*
- 🚧 **v4.5 Alta Prioridad** — Phases 77-80 (in progress)
- 🚧 **v5.0 Auditoria integral de calidad Rust** — Phases 81-88 (in progress)

## Phases

### 🚧 v5.0 Auditoria integral de calidad Rust (In Progress)

**Milestone Goal:** Auditar y refactorizar el firmware de forma segura para eliminar codigo muerto, reforzar practicas Rust/SOLID y validar que el control real desde Artisan Scope sigue siendo viable.

### Phase 81: Quality Baseline and Ratcheting Policy
**Goal**: Users can run a reproducible quality gate baseline with module-criticality ratchets that tighten quality without blocking lower-risk work.
**Depends on**: Phase 80
**Requirements**: QG-01, QG-02
**Status**: ✓ Complete (2026-03-07)
**Verification:** ✅ `passed` — score 7/7 must-haves verified; report: .planning/phases/81-quality-baseline-and-ratcheting-policy/81-VERIFICATION.md
**Success Criteria**:
1. User can run one documented baseline command sequence and get deterministic pass/fail results for format, curated lint, and tests.
2. User can see an explicit quality policy that applies stricter gates to safety/control/protocol modules before lower-risk modules.
3. User can fail a gate intentionally and observe actionable output that identifies the failing module and policy rule.
4. User can rerun gates after fixes and confirm policy ratchet behavior remains reproducible.
**Plans**: 3 plans

Plans:
- [x] 81-01-PLAN.md — Define versioned quality policy artifacts, tier mapping, and ratchet governance ✓ COMPLETE
- [x] 81-02-PLAN.md — Implement deterministic baseline orchestrator with policy-aware compact reporting ✓ COMPLETE
- [x] 81-03-PLAN.md — Add intentional-failure selfcheck fixtures and fail/rerun reproducibility drill ✓ COMPLETE

### Phase 82: Dead Code and Dependency Cleanup
**Goal**: Users can remove dead code and unused dependencies in controlled batches with evidence-backed safety and no behavior regressions.
**Depends on**: Phase 81
**Requirements**: DC-01, DC-02, DC-03
**Status**: ✓ Complete (2026-03-07)
**Verification:** ✅ `passed` — score 6/6 must-haves verified; report: .planning/phases/82-dead-code-and-dependency-cleanup/82-dead-code-and-dependency-cleanup-VERIFICATION.md
**Success Criteria**:
1. User can review a dead-code inventory by module with risk classification and evidence for why each candidate is safe or unsafe to remove.
2. User can execute small-batch dead-code removals and verify no regressions via tests and baseline behavior checks.
3. User can run dependency-audit workflow (`machete`/`udeps`) and see unused dependencies plus documented allowlisted exceptions.
4. User can trace each removal batch to evidence artifacts that justify deletion decisions.
**Plans**: 3 plans

Plans:
- [x] 82-01 — Establish dead-code inventory instrumentation and risk guidance ✓ COMPLETE
- [x] 82-02 — Create removal batch workflow with verification evidence ✓ COMPLETE
- [x] 82-03 — Automate dependency audits and allowlisted exceptions ✓ COMPLETE

### Phase 83: Rust Modernization and Unsafe Surface Audit
**Goal**: Users can modernize Rust code mechanically while preserving behavior and maintaining clear justification for remaining unsafe surfaces.
**Depends on**: Phase 82
**Requirements**: RUST-01, RUST-02
**Status**: ✓ Complete (2026-03-07)
**Verification:** ✅ `passed` — report: .planning/phases/83-rust-modernization-and-unsafe-surface-audit/83-VERIFICATION.md
**Success Criteria**:
1. User can run curated modernization passes (idioms/clippy/cargo-fix scope) and observe no protocol or control-flow behavior changes.
2. User can compare before/after behavior checks and confirm outputs remain semantically equivalent for existing command flows.
3. User can inspect an up-to-date unsafe surface register that lists active unsafe attributes/blocks with current justification and status.
4. User can identify which unsafe cases were removed, retained, or narrowed after modernization.
**Plans**: 3 plans

Plans:
- [x] 83-01 — Build modernization automation + doc ✓ COMPLETE
- [x] 83-02 — Capture unsafe register + summary ✓ COMPLETE
- [x] 83-03 — Regression verification guide + runner ✓ COMPLETE

### Phase 84: SOLID Seam Hardening and Fault Injection
**Goal**: Users can validate cleaner responsibility boundaries across handlers/hardware/control seams while preserving safety ordering under normal and faulted conditions.
**Depends on**: Phase 83
**Requirements**: SOLID-01, SOLID-02
**Status**: ✓ Complete (2026-03-08)
**Verification:** ✅ `passed` — score 3/3 must-haves verified; report: .planning/phases/84-solid-seam-hardening-and-fault-injection/84-VERIFICATION.md
**Success Criteria**:
1. User can observe that handler, hardware, and control responsibilities are separated at agreed high-value seams without breaking command behavior.
2. User can run watchdog/guard/comms fault-injection scenarios and observe expected safe-system responses.
3. User can verify safety/manual authority ordering remains intact after seam adjustments.
4. User can confirm control-loop behavior remains stable under fault and recovery scenarios.
**Plans**: 3 plans

Plans:
- [x] 84-01 — Harden handler/hardware policy boundary via ports-and-policies traits ✓ COMPLETE
- [x] 84-02 — Add stage instrumentation reporter for the 100 ms loop ✓ COMPLETE
- [x] 84-03 — Lock down the fault-injection harness and STATUS evidence matrix ✓ COMPLETE

### Phase 85: Hardware Acceptance Thresholds and Real Roaster Validation
**Goal**: Prove on real hardware that Artisan Scope controls a real roaster within explicit acceptance thresholds.
**Depends on**: Phase 84
**Requirements**: HW-01, HW-02
**Status**: ✓ Complete (2026-03-08)
**Verification:** ✅ `passed` — score 6/6 must-haves verified; report: .planning/phases/85-hardware-acceptance-thresholds-and-real-roaster-validation/85-VERIFICATION.md
**Success Criteria**:
1. User can review and apply numeric acceptance thresholds for command-to-actuator latency, response envelope, and safety counters.
2. User can execute a real-hardware validation run where Artisan Scope commands drive roaster outputs through this firmware.
3. User can compare measured run data against thresholds and determine pass/fail unambiguously.
4. User can package validation evidence (logs/transcripts/measurements) suitable for milestone signoff.
**Plans**: 3 plans

Plans:
- [x] 85-01-PLAN.md — Threshold Definition and Firmware Instrumentation ✓ COMPLETE
- [x] 85-02-PLAN.md — HIL Validation Runner Implementation ✓ COMPLETE
- [x] 85-03-PLAN.md — Real Hardware Validation Execution ✓ COMPLETE

### Phase 86: Fix Integration Regression (P84 <-> P85)
**Goal**: Restore broken regression tests and fault-injection scenarios after the 18-column STATUS expansion.
**Depends on**: Phase 85
**Requirements**: SOLID-02, HW-01
**Status**: pending
**Success Criteria**:
1. `tests/fault_injection_scenarios.rs` and `tests/regression_status.rs` handle 18-column STATUS output.
2. `SystemStatus` struct initialization in tests matches current implementation.
3. Assertions for CSV column counts match 18-field telemetry layout.
4. Fix verified using `scripts/quality-baseline.sh` orchestrator.

### Phase 87: Wire Modernization to Quality Policy (P81 <-> P83)
**Goal**: Enforce quality-baseline policy within automated modernization scripts to ensure no policy bypass.
**Depends on**: Phase 86
**Requirements**: QG-01, RUST-01
**Status**: pending
**Success Criteria**:
1. `run-modernization.sh` calls `scripts/quality-baseline.sh` for all audit checks.
2. `run-regression-checks.sh` calls `scripts/quality-baseline.sh`.
3. Policy ratchets (Tier 1/2) are enforced during automated cleanup runs.

### Phase 88: Architecture Alignment and UNITS Refactor
**Goal**: Align system instrumentation with safety tiers and refactor the UNITS command into the ManualCommandPolicy trait.
**Depends on**: Phase 87
**Requirements**: SOLID-01, QG-02
**Status**: pending
**Success Criteria**:
1. `src/application/stage_instrumentation.rs` promoted to Tier 1 in quality policy.
2. `UNITS` command refactored to implement `ManualCommandPolicy` trait.
3. Legacy match branch for `UNITS` removed from `RoasterControl::process_command`.
4. System remains architecturally consistent at the policy/handler boundary.

<details>
<summary>🚧 v4.5 Alta Prioridad (In Progress)</summary>

### Phase 77: Memory Optimization
**Goal**: Eliminate heap allocation from ArtisanFormatter's BT history tracking using heapless::Deque
**Requirements**: PERF-03
**Plans**: 1 plan

- [x] 77-01 — Replace Vec<f32> with Deque<f32, 5>, replace format! with heapless::String ✓ COMPLETE

**Success Criteria**:
1. `ArtisanFormatter.bt_history` uses `heapless::Deque<f32, 5>` instead of `Vec<f32>`
2. `MutableArtisanFormatter.bt_history` uses `heapless::Deque<f32, 5>` instead of `Vec<f32>`
3. No `alloc::format!` calls in hot path; uses `core::write!` with `heapless::String`
4. All existing tests pass without modification

### Phase 78: SSR Deduplication
**Goal**: Extract detect_heat_source()Base, eliminating duplicate to SsrControl code
**Requirements**: SSR-06
**Plans**: 1 plan

- [x] 78-01 — Extract detect_heat_source to SsrControlBase ✓ COMPLETE

### Phase 79: Test Infrastructure
**Goal**: Migrate test stubs to library-accessible location
**Requirements**: TEST-06
**Plans**: 5 plans

- [x] 79-01 — Migrate stubs to src/common/mod.rs with pub(crate) visibility ✓ COMPLETE
- [x] 79-02 — Close integration-stub imports and host test target gaps (gap closure) ✓ COMPLETE
- [x] 79-03 — Publish core/alloc stub helpers through `libreroaster::common` so the shim re-export compiles (gap closure) ✓ COMPLETE
- [x] 79-04 — Host mock UART suites match emulator buffers and complete on x86_64 (gap closure) ✓ COMPLETE
- [x] 79-05 — Guard roaster_sync critical section so concurrent sensor reads no longer panic (gap closure) ✓ COMPLETE

**Success Criteria**:
1. `StubHeater`, `StubFan`, `StubThermometer` exist in `src/common/mod.rs` with `pub(crate)` visibility
2. Tests can import stubs via `crate::common::{StubHeater, StubFan, StubThermometer}`
3. Both unit tests (`#[cfg(test)]`) and integration tests (`tests/*.rs`) use the same stubs
4. All tests pass after migration

**Verification:** ✅ `passed` — host mock UART suites and `concurrent_sensor_reads_verify_async_mutex` now pass on x86_64 after draining buffers, capping heapless growth, and serializing `roaster_sync` via `TestCriticalSection`.

### Phase 80: Handler Pattern
**Goal**: Refactor process_artisan_command() to delegate to ArtisanCommandHandler
**Requirements**: REF-01
**Success Criteria**:
1. `process_artisan_command()` delegates to handler structs (no direct match in RoasterControl)
2. Large ~125-line match statement removed from roaster_refactored.rs
3. Existing Artisan command behavior unchanged (READ, START, STOP, OT1, IO3, UP, DOWN all work)
4. All integration tests pass
**Plans:** 4 plans

- [x] 80-01 — Route manual Artisan commands through the handler chain and keep the handler authoritative for manual outputs
- [ ] 80-02 — To be defined
- [ ] 80-03 — To be defined
- [ ] 80-04 — To be defined

**Verification:** ✅ `passed` — Handler Pattern manual command refactor verified (score 3/3 must-haves) with `cargo check --lib` and `cargo test --test fan_serialization --test mock_uart_integration`; report: .planning/phases/80-handler-pattern/80-VERIFICATION.md

</details>

<details>
<summary>✅ v4.1 Documentation Update (Shipped 2026-02-23)</summary>

- Phases 62-69 archived — documentation cleanup, watchdog/LEDC/regression instrumentation, automation hook docs
- README/FLASH_GUIDE now match current pinouts, targets, and automation hooks; instrumentation guide points at STATUS/REG telemetry
- WatchdogFeeder, LEDC guard, and regression runner telemetry now log `SAFETY WATCHDOG`, `SAFETY LEDC-GUARD`, and `SAFETY OT-REGRESSION` while `run_overtemp_regression` stays private
- Full phase details live in `.planning/milestones/v4.1-ROADMAP.md`

</details>

<details>
<summary>✅ v4.4 SSR Refactoring & Test Stubs (Shipped 2026-02-25)</summary>

#### Phase 75: SSR Refactoring
**Goal**: Extract common state into SsrControlBase and define SsrControlTrait to eliminate code duplication between SsrControl and SsrControlSimple.
**Requirements**: SSR-01, SSR-02, SSR-03, SSR-04, SSR-05
**Plans**: 2 plans

- [x] 75-01 — Extract SsrControlBase and traits, refactor both SSR types
- [x] 75-02 — Add missing trait implementations (gap closure)

#### Phase 76: Test Infrastructure
**Goal**: Create shared test stubs module to eliminate ~5x duplication in test helpers.
**Requirements**: TEST-01, TEST-02, TEST-03, TEST-04, TEST-05
**Plans**: 1 plan

- [x] 76-01 — Create tests/common/mod.rs with StubHeater, StubFan, StubThermometer

**Key outcomes:**
- Created SsrControlBase struct with ~90 lines of duplicated code eliminated
- Both SSR types now implement HeatSourceDetector, PeriodicCheck, StatusGetters traits
- Created tests/common/mod.rs with StubHeater, StubFan, StubThermometer using RefCell
- Added reset_channels() and collect_output() helper functions
- Fixed Cargo.toml to enable host target compilation
- Completed 2 phases (75-76) and 3 plans total

</details>

## Progress

| Phase | Milestone | Plans | Status | Completed |
|-------|-----------|-------|--------|-----------|
| 70    | v4.2      | 2/2   | Complete | 2026-02-24 |
| 71    | v4.2      | 3/3   | Complete | 2026-02-24 |
| 72    | v4.2      | 3/3   | Complete | 2026-02-24 |
| 73    | v4.3      | 1/1   | Complete | 2026-02-24 |
| 74    | v4.3      | 1/1   | Complete | 2026-02-24 |
| 75    | v4.4      | 2/2   | Complete | 2026-02-24 |
| 76    | v4.4      | 1/1   | Complete | 2026-02-25 |
| 77    | v4.5      | 1/1   | Complete | 2026-02-28 |
| 78    | v4.5      | 1/1   | Complete    | 2026-02-28 |
| 79    | v4.5      | 5/5   | ✓ Complete | 2026-02-28 |
| 80    | v4.5      | 1/4   | In progress | — |
| 81    | v5.0      | 3/3   | ✓ Complete | 2026-03-07 |
| 82    | v5.0      | 3/3   | ✓ Complete | 2026-03-07 |
| 83    | v5.0      | 3/3   | ✓ Complete | 2026-03-07 |
| 84    | v5.0      | 3/3   | ✓ Complete | 2026-03-08 |
| 85    | v5.0      | 3/3   | ✓ Complete | 2026-03-08 |
| 86    | v5.0      | 0/1   | pending    | — |
| 87    | v5.0      | 0/1   | pending    | — |
| 88    | v5.0      | 0/1   | pending    | — |

---

## Coverage

- **v5.0 requirements:** 11 total
- **Mapped to phases:** 11 (100%)
- **Unmapped:** 0

---

*Ready for planning: `/gsd-plan-phase 81`*
