# Project Research Summary

**Project:** LibreRoaster
**Domain:** ESP32-C3 Artisan/Artisan+ UART firmware (roaster control)
**Researched:** 2026-02-04
**Confidence:** MEDIUM

## Executive Summary

LibreRoaster is an ESP32-C3 firmware that must present a fully compliant Artisan/Artisan+ UART protocol so Artisan can read roaster telemetry and drive heater/fan outputs. Experts ship this class of firmware by centralizing protocol knowledge (command registry + formatter), enforcing strict value and formatting rules, and proving behavior through host-side mocks before hardware testing. The recommended path is to extend the existing Embassy-based tasks with a data-driven command manifest, a single formatter with golden/property tests, and a strict mock UART harness for end-to-end command→response coverage.

The stack should stay minimal on-device, keeping new crates as dev-only: proptest for parser/formatter fuzzing, insta for formatter golden outputs, mock-embedded-io for UART simulation, and rstest for command matrices. Key risks are partial command coverage, formatter drift (precision/CRLF), permissive mocks that hide timing/backpressure issues, and undefined behavior on malformed inputs; mitigate with a registry-backed coverage matrix, centralized formatter helpers, strict mock UART modes, and explicit error schemas. With these in place, the roadmap can sequence coverage, formatting, and integration hardening without inflating firmware size.

## Key Findings

### Recommended Stack

Host-side test stack only (firmware size unchanged): proptest 1.9.0 for property fuzzing, insta 1.46.3 for golden formatter outputs, mock-embedded-io 0.1.0 plus embedded-io-cursor 0.1.0 for UART/fixture simulation, and rstest 0.26.1 for table-driven command matrices. Coverage via cargo-llvm-cov 0.8.2 on host. All align with embedded-io 0.7.1 and Rust 1.88 toolchain.

**Core technologies:**
- proptest — property-based parser/formatter fuzzing — shrinks failing cases and exercises malformed commands without firmware bloat.
- insta — snapshot approval testing — locks formatter precision/CRLF/columns to prevent regressions.
- mock-embedded-io — UART mock implementing embedded-io/async — enables end-to-end command loops with Embassy tasks.

### Expected Features

Must cover full Artisan/Artisan+ command matrix with strict bounds and deterministic formatting; start/stop state integrity and command→response integration tests are table stakes. Differentiators include defensive parser diagnostics, fuzz/soak coverage, timing-tolerance tests, formatter golden files, and optional per-channel safety caps. Defer feature-flagged protocol extensions, telemetry enrichments, and pluggable validation policies to v2+.

**Must have (table stakes):**
- Complete Artisan/Artisan+ command coverage with explicit errors for unknown/bad payloads.
- Strict bounds and error responses (0–100% outputs) with deterministic CSV formatting and cadence.
- Command→response integration tests over mock UART plus start/stop state correctness.

**Should have (competitive):**
- Defensive parser diagnostics with verbosity controls.
- Fuzz/soak and timing-tolerance tests on parser/formatter via mock UART.
- Formatter golden files for regression detection; configurable per-channel safety caps.

**Defer (v2+):**
- Feature-flagged protocol extensions, telemetry/log enrichments, pluggable validation policies.

### Architecture Approach

Use Embassy async tasks for UART read/write, a shared command registry feeding parser and dispatcher, centralized formatter for Artisan/Artisan+ responses, and a mock UART-backed integration harness. Keep parsing/formatting pure and stateless; isolate hardware concerns in `uart/`, protocol logic in `input/` and `output/`, control/state in `control/`, and reusable mocks/fixtures in `test_support/` with round-trip tests under `tests/`.

**Major components:**
1. Command registry/dispatcher — maps commands to handlers and enforces mode/validation.
2. Formatter — canonicalizes CSV/Artisan+ outputs with golden/property tests.
3. Mock UART integration harness — drives reader/formatter/writer tasks to assert timing, backpressure, and responses.

### Critical Pitfalls

1. **Partial command matrix coverage** — prevent with a spec-derived manifest + table-driven coverage tests for all commands and error paths.
2. **Formatter drift (precision/CRLF/columns)** — avoid by centralizing formatter helpers and locking snapshots across normal and edge values (NaN/inf, large numbers).
3. **Mock UART too permissive** — implement strict mode with buffer limits, per-byte timing, backpressure, and timeout assertions.
4. **State leakage between handlers/formatter** — keep formatter stateless per request; add harness reset hooks and randomized command order tests.
5. **Undefined behavior on invalid inputs** — define error schema, bound payload sizes, and test malformed frames/partial lines to ensure no panics or stale reuse.

## Implications for Roadmap

### Phase 1: Command Manifest & Coverage Matrix
**Rationale:** Locks protocol completeness early and aligns parser/dispatcher with spec before formatter changes. Reduces risk of hidden missing commands.
**Delivers:** Spec-derived command registry, coverage matrix, and unit tests for all command parses/handlers (success + explicit errors). Implements P1 feature “complete command coverage”.
**Avoids:** Partial command matrix coverage; undefined behavior on unknown commands.
**Research flag:** Validate latest Artisan/Artisan+ spec details (rare commands, mode gating) via `/gsd-research-phase` if spec ambiguity remains.

### Phase 2: Spec-Compliant Formatter & Error Schema
**Rationale:** Formatter drift is a top risk; stabilizing output before integration avoids rework. Relies on Phase 1 commands enumerated.
**Delivers:** Central formatter helpers, golden snapshots (insta), property tests (proptest) for precision/CRLF/column counts, standardized error codes/messages. Covers P1 deterministic formatter + strict bounds; sets up P2 formatter golden files.
**Avoids:** Formatter drift, state leakage, undefined input behavior.
**Research flag:** If official Artisan formatting nuances (line endings, decimal places) are unclear, run targeted research/tests with the PC app.

### Phase 3: Strict Mock UART Integration Loop
**Rationale:** Confirms timing/backpressure and end-to-end correctness using real tasks; depends on Phase 1/2 protocol correctness.
**Delivers:** Mock UART with buffer/backpressure/timing controls, integration tests for command sequences (success/error/start/stop), cadence verification, coverage reporting via cargo-llvm-cov. Implements P1 command→response integration; enables P2 timing-tolerance and fuzz/soak paths.
**Avoids:** Permissive mock masking UART issues; latent panics on malformed frames.
**Research flag:** Optional `/gsd-research-phase` on UART timing/backpressure norms for Artisan over USB serial if discrepancies arise.

### Phase 4: Resilience & Safety Enhancements
**Rationale:** Builds atop stable integration loop to add diagnostics and optional caps without destabilizing baseline.
**Delivers:** Defensive parser diagnostics, timing-tolerance suites, fuzz/soak coverage, configurable per-channel safety caps (feature-gated), and snapshot maintenance workflow.
**Avoids:** Regression from late-added safety/diagnostics; logging-induced performance traps.
**Research flag:** Likely skip unless extending protocol; patterns are standard once baseline is verified.

### Phase Ordering Rationale

- Establish correctness (coverage) → stability (formatter) → realism (mock timing/backpressure) → robustness (fuzz/diagnostics) aligns with architecture dependencies.
- Command registry first keeps parser/dispatcher in sync; formatter next avoids double work in integration tests; strict mock ensures CI confidence before optional enhancements.
- Pitfall coverage maps: Phase 1 (coverage gaps), Phase 2 (formatter drift/state leakage/invalid inputs), Phase 3 (mock permissiveness), Phase 4 (resilience/diagnostics gaps).

### Research Flags

Phases needing deeper research:
- **Phase 1:** Confirm complete Artisan/Artisan+ command list and mode gating from latest spec/PC app behavior.
- **Phase 2:** Verify canonical formatting (precision, CRLF) against Artisan client expectations.
- **Phase 3:** Characterize typical USB-serial jitter/backpressure for Artisan to tune mock defaults.

Phases with standard patterns (research likely unnecessary):
- **Phase 4:** Diagnostics/fuzz/safety caps use established Rust test/mocking patterns once baseline is stable.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Versions and crates pulled from crates.io with explicit compatibility to embedded-io 0.7.1 and Rust 1.88; dev-only usage keeps firmware unaffected. |
| Features | MEDIUM | Derived from domain expectations and existing project goals; needs validation against current Artisan/Artisan+ spec nuances. |
| Architecture | MEDIUM | Based on current repo structure and common Embassy patterns; limited external validation. |
| Pitfalls | MEDIUM | Grounded in prior firmware experience; depends on confirming actual Artisan spec and USB-serial behavior. |

**Overall confidence:** MEDIUM

### Gaps to Address

- Confirm full Artisan/Artisan+ command manifest (including rare/diagnostic commands) against latest official docs/PC behavior.
- Validate canonical formatter rules (decimal precision, CRLF, column ordering) with the Artisan client to finalize snapshots.
- Measure realistic UART timing/backpressure under USB serial to set strict mock defaults and CI tolerances.
- Hardware validation remains out of scope; plan a follow-up once host-side integration is green.

## Sources

### Primary (HIGH confidence)
- crates.io package pages: proptest 1.9.0, insta 1.46.3, mock-embedded-io 0.1.0, embedded-io-cursor 0.1.0, rstest 0.26.1, cargo-llvm-cov 0.8.2 (version verification and compatibility notes).

### Secondary (MEDIUM confidence)
- Existing LibreRoaster codebase and prior milestone notes on Artisan formatter/parsing behavior; Artisan/Artisan+ protocol expectations inferred from project context (needs direct spec confirmation).

### Tertiary (LOW confidence)
- None cited beyond internal inference; future validation with the Artisan PC app recommended.

---
*Research completed: 2026-02-04*
*Ready for roadmap: yes*
