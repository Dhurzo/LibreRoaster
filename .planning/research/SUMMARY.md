# Project Research Summary

**Project:** LibreRoaster v5.0 Auditoria integral de calidad Rust
**Domain:** Embedded Rust firmware quality hardening (ESP32-C3 coffee roaster control)
**Researched:** 2026-03-07
**Confidence:** HIGH

## Executive Summary

LibreRoaster v5.0 is a quality-hardening milestone for a brownfield embedded firmware product, not a feature-expansion release. The research converges on a conservative, evidence-first strategy used by mature embedded Rust teams: freeze runtime behavior contracts first, then run staged dead-code and best-practice cleanup behind strict gates, and only then perform targeted SOLID seam extraction in the highest-risk modules.

The recommended approach is to preserve the existing deterministic runtime architecture (`control_loop_task`, handler authority, guarded actuation) and add a host-side quality layer for audit inventory, automated gates, and hardware evidence packaging. Tooling should be pinned and split by purpose: stable for daily lint/test/governance workflows, nightly only for `cargo-udeps` dead-dependency audits. This keeps hardening velocity high without destabilizing firmware builds.

The key risks are false-positive dead code deletions, authority fragmentation during refactors, and host-only confidence that misses real hardware behavior. Mitigation is explicit in all research tracks: candidate classification with evidence per deletion, single-writer invariants at safety/manual boundaries, loop-budget/timing gates in release builds, and mandatory Artisan Scope hardware validation artifacts before milestone signoff.

## Key Findings

### Recommended Stack

The stack recommendation is intentionally incremental: keep firmware runtime tech unchanged, add audit-grade quality tooling around it, and pin versions to avoid drift while refactoring.

**Core technologies:**
- Rust toolchain pin `1.88.0` (+ nightly tools-only) — deterministic ESP32-C3 builds with `esp-hal 1.0.0` compatibility and controlled nightly usage for `udeps`.
- `cargo-nextest` `0.9.129` + `cargo-llvm-cov` `0.8.4` — reliable regression orchestration with measurable coverage fail-under gates before deletion.
- `cargo-udeps` `0.1.60` + `cargo-machete` `0.9.1` — layered unused dependency detection (fast stable preflight + deeper nightly verification).
- `cargo-modules` `0.25.0` + `cargo-deny` `0.19.0` + `cargo-geiger` `0.13.0` — architecture hygiene, supply-chain policy, and unsafe-surface trend control.
- `serialport` `4.8.1` + `csv` `1.4.0` (dev) — reproducible command/telemetry evidence for HW-01 hardware validation.

### Expected Features

v5.0 success criteria are operational hardening capabilities, not new protocol semantics. Research aligns on a four-part MVP sequence with clear dependencies.

**Must have (table stakes):**
- Quality gate baseline with ratcheting lint/format/fail policy by module criticality.
- Codebase audit inventory plus controlled dead-code elimination workflow (static signals + runtime evidence).
- Rust best-practices modernization pass (mechanical first, semantic second).
- Hardware realism validation path (Artisan-driven HIL smoke + auditable artifacts).

**Should have (competitive):**
- Command-to-actuator traceability matrix (`command -> queue -> actuator -> telemetry -> guard`).
- Artifact-backed HIL scenarios with expected envelopes and release evidence retention.
- Fault-injection hardening scenarios for watchdog/guard/comms anomalies.

**Defer (v2+):**
- Full hardware lab orchestration platform.
- Global strict clippy saturation in one pass.
- Any protocol-semantic redesign during quality hardening.

### Architecture Approach

The architecture recommendation is to keep runtime paths stable and add an out-of-band quality layer. Runtime remains centered on queue/multiplexer ingestion, `control_loop_task` cadence, authoritative handler chain, guarded actuation, and telemetry output. New v5.0 components live mostly in `.planning/quality/` and `tests/hardware/`, where audit inventories, dependency maps, gate outputs, and hardware evidence templates can evolve without injecting overhead into the 100 ms loop.

**Major components:**
1. `ServiceContainer` + transport tasks — preserve queue/channel boundaries and ingestion semantics.
2. `control_loop_task` + `RoasterControl`/handlers — retain deterministic orchestration and single authority for manual/safety state.
3. v5.0 quality layer (`.planning/quality/`, `tests/hardware/`) — run gates, collect artifacts, and support milestone signoff decisions.

### Critical Pitfalls

1. **False dead-code deletions with linker/runtime side-effects** — classify candidates, ban blind removals of `#[used]`/unsafe linker attributes, require symbol/test/transcript evidence per deletion.
2. **Removing Artisan branches needed in real sessions** — freeze canonical transcripts and gate deletions on replay with response-shape assertions.
3. **SOLID refactor splitting safety/manual authority** — document invariants and enforce single-writer ownership for critical fields with contract tests.
4. **Hot-path regressions from over-abstraction** — enforce release loop-budget gates, keep tick path allocation-free, and avoid heavy diagnostics in control cadence.
5. **Skipping HIL readiness and relying on host-only confidence** — require command-to-actuator correlation artifacts and at least one fault-injection run before real hardware signoff.

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Baseline Freeze and Gate Foundation
**Rationale:** Every later change depends on stable contracts and measurable guardrails.
**Delivers:** Frozen command/safety/telemetry invariants, pinned toolchain, initial gate pipeline (`clippy -> machete -> nextest -> deny`).
**Addresses:** Quality gate baseline, codebase audit setup.
**Avoids:** Protocol drift, observability loss, rollback ambiguity.

### Phase 2: Dead Code Audit and Low-Risk Elimination
**Rationale:** Remove maintenance drag early, but only where evidence is strongest.
**Delivers:** Dead-code inventory, dependency map, small-batch removals with coverage and transcript checks.
**Uses:** `dead_code` policy, `cargo-machete`, `cargo +nightly udeps`, `cargo-llvm-cov`.
**Avoids:** Linker-side-effect deletions, feature-gate drift, hidden protocol regressions.

### Phase 3: Rust Modernization (Mechanical First)
**Rationale:** Improve maintainability before structural seam work, while minimizing behavior change risk.
**Delivers:** Curated clippy improvements, lint ratchet by module, explicit error conversion policy, reviewed unsafe-attribute updates.
**Implements:** Out-of-band quality gates with no added runtime loop cost.
**Avoids:** Error semantic collapse, noisy global lint churn, unsafe modernization sweep risks.

### Phase 4: Pragmatic SOLID Seam Extraction in Hot Paths
**Rationale:** After cleanup/modernization, seams can be introduced with clearer dependency boundaries.
**Delivers:** Incremental ports-and-policies extraction around command routing/handlers, preserved behavior and safety ordering.
**Addresses:** Targeted SOLID alignment and testability gains.
**Avoids:** Big-bang rewrites, authority fragmentation, 100 ms loop jitter regressions.

### Phase 5: HIL Preflight and Hardware Evidence Pack
**Rationale:** Host confidence must be converted into real-hardware proof before closeout.
**Delivers:** Artisan Scope checklist runs, command/actuator/telemetry correlation artifacts, nominal + injected-fault evidence bundle.
**Uses:** `serialport`/`csv` harness, `tests/hardware` evidence templates.
**Avoids:** Late hardware surprises and non-auditable release claims.

### Phase Ordering Rationale

- The order follows hard dependencies from FEATURES + PITFALLS: gates and invariants first, deletions second, modernization third, structural refactor fourth, hardware proof last.
- Architecture guidance supports seam-first changes only after inventory and low-risk cleanup reduce coupling noise.
- This sequence minimizes compounded risk by isolating one class of change per phase and keeping each slice reversible.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 2:** Target/feature matrix for dead-code decisions, especially linker/attribute-sensitive modules.
- **Phase 4:** Refactor boundary design in `RoasterControl`/handlers to guarantee single-writer invariants and loop-budget compliance.
- **Phase 5:** HIL scenario thresholds, actuator observability method, and fault-injection acceptance criteria.

Phases with standard patterns (skip research-phase):
- **Phase 1:** Toolchain pinning and baseline quality-gate setup are well-documented and high-confidence.
- **Phase 3:** Mechanical modernization workflow (curated clippy/cargo fix discipline) is established if safety modules are handled conservatively.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Version/tool recommendations are specific and mostly backed by official docs/repos plus project constraints. |
| Features | MEDIUM-HIGH | Priorities are clear and dependency-mapped, but final scope pressure (P1 vs P2) remains planning-dependent. |
| Architecture | HIGH | Strongly grounded in existing LibreRoaster runtime boundaries and explicit non-invasive patterns. |
| Pitfalls | MEDIUM-HIGH | Comprehensive and practical, with solid official references; some prevention tactics still need project-specific thresholds. |

**Overall confidence:** HIGH

### Gaps to Address

- **Quantified loop-budget threshold:** define concrete per-tick SLO and fail criteria for release builds before Phase 4 merges.
- **Target/feature gate matrix completeness:** formalize required host + riscv32 jobs per feature profile to avoid false dead-code confidence.
- **HIL acceptance envelopes:** set numeric pass/fail tolerances for command latency, actuator response, and safety counters.
- **Evidence retention policy:** decide where gate/HIL artifacts live and how long they are retained for audit traceability.

## Sources

### Primary (HIGH confidence)
- `.planning/research/STACK.md` — pinned stack and toolchain/gate recommendations.
- `.planning/research/FEATURES.md` — table stakes, differentiators, anti-features, and dependency model.
- `.planning/research/ARCHITECTURE.md` — runtime boundaries, incremental patterns, and build order.
- `.planning/research/PITFALLS.md` — phase-specific failure modes and prevention controls.
- Rust official docs (rustc lints, cargo fix/profiles, Rust reference/edition guide) — lint, unsafe attribute, and build behavior constraints.
- Embedded ecosystem docs (`esp-hal`, Embassy executor, embedded-hal) — runtime and boundary constraints for ESP32-C3/async firmware.

### Secondary (MEDIUM confidence)
- Tooling project docs (`cargo-udeps`, `cargo-nextest`, `cargo-llvm-cov`, `cargo-deny`, `cargo-modules`, `cargo-geiger`) — operational behavior and integration guidance.
- `probe-rs` and community-maintained Embassy FAQ — practical target validation and release/LTO gotchas.

### Tertiary (LOW confidence)
- None identified as decision-critical for this synthesis.

---
*Research completed: 2026-03-07*
*Ready for roadmap: yes*
