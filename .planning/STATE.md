# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-07)

**Core value:** Artisan can read temperatures and control heater/fan during a roast session via serial connection.
**Current focus:** Phase 84 SOLID seam hardening - ports-and-policies implementation.

## Current Position

Phase: 84 of 85 (SOLID Seam Hardening and Fault Injection)
Plan: 3 of 3
Status: Phase complete
Last activity: 2026-03-08 — Completed 84-03-PLAN.md (fault-injection harness)

Progress: [██████████] 100% (120/119 plans complete)

## Performance Metrics

- **Velocity:**
- Total plans completed: 20 (phase 84 started)
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
| 83 | 3/3 | 3 | Phase complete |
| 84 | 1/3 | 3 | In progress |

**Recent Trend:**
- Architecture cleanup work is stabilizing command handling and shared test infrastructure.
- v5.0 shifts execution focus from feature delivery to audit-grade quality hardening and hardware evidence.
- Dead-code instrumentation/removal gating plus dependency audits now have executable workflows backed by audit logs.
- Phase 84 introduces ports-and-policies pattern to centralize hardware authority.

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
- Handler policy evaluation returns outcomes without hardware writes; RoasterControl is single writer.

### Pending Todos

- Phase 85: Hardware Acceptance Thresholds and Real Roaster Validation

### Blockers/Concerns

- Real hardware access and measurement setup are required to execute Phase 85 (HW-02).

## Session Continuity

Last session: 2026-03-08T13:11:17Z
Stopped at: Completed 84-03-PLAN.md (fault-injection harness and STATUS evidence matrix)
Resume file: None
