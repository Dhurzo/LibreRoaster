# Project Research Summary

**Project:** LibreRoaster
**Domain:** ESP32-C3 Rust firmware Artisan protocol edge-case fixes
**Researched:** 2026-02-17
**Confidence:** MEDIUM

## Executive Summary

LibreRoaster is an embedded ESP32-C3 firmware project that must interoperate with Artisan via a strict ASCII serial protocol. Experts treat this as a protocol-compatibility problem: preserve the current stack, enforce exact line termination and CSV shape, and validate behavior with host-side tests so regressions are caught quickly.

The recommended approach is to keep the existing Rust 1.88 + esp-hal + embassy-time baseline, focus changes in the Artisan formatter and output boundary, and add tests that assert byte-accurate CRLF handling plus correct delta_bt/ROR state updates. The work should be phased as protocol contract + tests first, then firmware implementation details that ensure atomic writes and consistent state updates.

Key risks are terminator inconsistencies, truncated line endings under buffer pressure, and stale/out-of-order delta_bt updates. Mitigate these by having a single terminator policy, snapshot-then-format semantics, and deterministic sample sequencing with explicit reset behavior across START/STOP.

## Key Findings

### Recommended Stack

No stack changes are required for this milestone. The current Rust 1.88 toolchain, esp-hal 1.0.0, and embassy-time 0.5.0 are already validated and suitable for protocol edge-case fixes. Host-side tests under `--features test` are the preferred verification path for CRLF and ROR logic.

**Core technologies:**
- Rust toolchain 1.88: build/test harness — already pinned and validated
- esp-hal 1.0.0: ESP32-C3 HAL — stable baseline for I/O paths
- embassy-time 0.5.0: timing utilities — used for ROR/delta timing logic

### Expected Features

The MVP is narrowly scoped to Artisan protocol correctness: a single CRLF terminator for READ, a 4-value CSV with one-decimal precision, and ROR that becomes non-zero after the second BT sample with resets on formatter reset. Differentiators focus on reducing regressions via centralized terminator policy and tests.

**Must have (table stakes):**
- Single CRLF terminator for READ responses — exact Artisan framing
- READ stays 4-value CSV with one-decimal precision — protocol contract
- ROR updates after BT changes with proper reset — correct derived metrics
- Tests covering CRLF + ROR behaviors — regression prevention

**Should have (competitive):**
- Centralized line-termination policy — prevents USB/UART drift
- Protocol-focused tests for CRLF + ROR — faster verification

**Defer (v2+):**
- Protocol fuzz tests for malformed command framing — useful but out of scope

### Architecture Approach

The architecture emphasizes a clear pipeline: parser and multiplexer feed an Artisan command handler, which updates a status snapshot used by a stateful formatter to produce READ responses. Terminator policy should be owned by a single formatter boundary, and derived metrics (delta_bt/ROR) should update in the formatter based on a consistent cadence or snapshot.

**Major components:**
1. Serial input + parser — turn ASCII into Artisan commands
2. RoasterControl + command handler — create status snapshot per READ
3. ArtisanFormatter + serial output — format CSV and enforce terminator

### Critical Pitfalls

1. **Terminator mismatch or missing EOL** — enforce a single terminator policy and unit-test exact bytes.
2. **Terminator split/truncation across writes** — write frames atomically or drain buffers fully; add stress tests.
3. **Stale/out-of-order delta_bt/ROR** — update in a single ordered step with timestamps.
4. **State not reset on START/STOP** — reset history on transitions and test the first frame.
5. **Unit/scale mismatch for ROR** — document units and validate with known sequences.

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Protocol Contract + Tests
**Rationale:** Lock down the exact protocol behavior first to prevent regressions.
**Delivers:** Byte-accurate CRLF termination tests, ROR/delta behavior tests, documented terminator ownership.
**Addresses:** Single CRLF terminator, ROR non-zero after BT changes, tests covering edge cases.
**Avoids:** Terminator mismatch, unit/scale errors, missing reset behavior.

### Phase 2: Formatter + Output Implementation
**Rationale:** Implement deterministic formatter state updates and safe output paths once the contract is defined.
**Delivers:** Updated formatter state handling, atomic/consistent output writes, START/STOP reset logic.
**Uses:** Rust 1.88 + esp-hal + embassy-time (no stack changes).
**Implements:** Snapshot-then-format, derived-metric state in formatter, single terminator policy.

### Phase 3: Integration Hardening
**Rationale:** Validate end-to-end behavior across USB CDC/UART and dual output paths.
**Delivers:** Integration tests for dual output routing and buffer pressure scenarios.

### Phase Ordering Rationale

- The protocol contract must be defined and tested before implementation changes to avoid partial fixes.
- Formatter changes depend on a consistent snapshot and terminator policy, which the tests enforce.
- Integration hardening follows once core behavior is correct to ensure I/O paths do not reintroduce regressions.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 2:** Serial write behavior under buffer pressure and task scheduling specifics

Phases with standard patterns (skip research-phase):
- **Phase 1:** Protocol tests and CRLF contract are well-defined and documented
- **Phase 3:** Standard integration testing patterns apply once I/O paths are known

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Official docs and existing project pinning; no changes needed |
| Features | MEDIUM | Based on internal docs and code references; verify against current implementation |
| Architecture | MEDIUM | Based on project context; limited codebase validation |
| Pitfalls | MEDIUM | Domain experience and common issues; needs validation in project context |

**Overall confidence:** MEDIUM

### Gaps to Address

- Confirm current formatter/output ownership of terminator in code — adjust if multiple paths append CRLF.
- Validate delta_bt/ROR update cadence against actual sampling interval — ensure correct units.
- Verify USB CDC/UART write buffering behavior for terminator truncation risk.

## Sources

### Primary (HIGH confidence)
- https://docs.rs/esp-hal/latest/esp_hal/ — HAL version and usage
- https://docs.rs/embassy-time/latest/embassy_time/ — timing utilities
- https://docs.rs/embedded-hal/latest/embedded_hal/ — trait compatibility

### Secondary (MEDIUM confidence)
- `internalDoc/PROTOCOL.md` — READ format and precision
- `src/output/artisan.rs` — formatter and ROR formatting
- `src/application/tasks.rs` — READ response wiring
- `src/hardware/usb_cdc/tasks.rs` — CRLF behavior on USB CDC
- `src/hardware/uart/tasks.rs` — CRLF behavior on UART

### Tertiary (LOW confidence)
- Prior embedded serial protocol experience — pitfalls and mitigations

---
*Research completed: 2026-02-17*
*Ready for roadmap: yes*
