# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-07)

**Core value:** Artisan can read temperatures and control heater/fan during a roast session via serial connection.
**Current focus:** Phase 82 verification and handover for dead-code + dependency cleanup.

## Current Position

Phase: 82 of 85 (Dead Code and Dependency Cleanup)
Plan: 3 of 3
Status: Complete (verified)
Last activity: 2026-03-07 — Phase 82 verified; 6/6 must-haves passed.

Progress: [██████████] 100% (119/119 plans complete)

## Performance Metrics

- **Velocity:**
- Total plans completed: 16 (phase 82 complete)
- Average duration: mixed; recent phases include both quick gap-closures and deeper refactors
- Total execution time: cumulative multi-milestone history (see roadmap progress table)

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 77 | 1/1 | 1 | n/a |
| 78 | 1/1 | 1 | n/a |
| 79 | 5/5 | 5 | n/a |
| 80 | 1/4 | 4 | in progress |
| 81 | 3/3 | 3 | Phase complete |
| 82 | 3/3 | 3 | Phase complete |

**Recent Trend:**
- Architecture cleanup work is stabilizing command handling and shared test infrastructure.
- v5.0 shifts execution focus from feature delivery to audit-grade quality hardening and hardware evidence.
- Dead-code instrumentation/removal gating plus dependency audits now have executable workflows backed by audit logs.

## Accumulated Context

### Decisions

- v5.0 is structured as five dependency-ordered phases: quality baseline, dead-code cleanup, Rust modernization, SOLID seam hardening, and real-hardware validation.
- Requirement coverage is strict: each v5.0 requirement maps to exactly one phase (11/11, no duplicates).
- Hardware signoff is gated by explicit numeric thresholds before real-roaster validation execution.
- Tiered enforcement: T1 blocks (safety/control/protocol), T2/T3 informational for gradual ratcheting
- Policy-first quality: Define contract (QG-POLICY v1.0.0) before automation
- Ratchet updates require both version bump (semver) and human-readable changelog entry
- Clippy-driven dead-code inventory now records Git/toolchain metadata plus a stable pointer for downstream automation.
- Risk guidance now ties each high/medium/low bucket to specific inventory spans and required evidence before removal.
- Run `cargo machete` with `--with-metadata --skip-target-dir` and gate on exit codes so the audit prints logs even when unused crates are reported.
- Run `cargo +nightly udeps --quiet` after cleaning artifacts so duplicate-lang-item errors do not block the nightly audit.
- Allowlist the crates that udeps flags today (`embassy-usb`, `embedded-hal-bus`, `embedded-io`, `libm`, `static_cell`) so every audit run can justify their retention.

### Pending Todos

- Complete remaining phase 80 plans (v4.5) while preserving Artisan command compatibility.
- Kick off Phase 83: Rust Modernization and Unsafe Surface Audit.

### Blockers/Concerns

- Real hardware access and measurement setup are required to execute Phase 85 (HW-02).

## Session Continuity

Last session: 2026-03-07T13:11:00Z
Stopped at: Phase 82 verified (6/6 must-haves) — ready for Phase 83.
Resume file: None
