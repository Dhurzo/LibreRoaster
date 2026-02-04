# Pitfalls Research

**Domain:** Artisan command coverage, formatter hardening, mock UART integration tests for LibreRoaster v1.2
**Researched:** 2026-02-04
**Confidence:** MEDIUM (based on protocol expectations + prior firmware experience; validate against current Artisan/Artisan+ spec)

## Critical Pitfalls

### Pitfall 1: Partial command matrix coverage

**What goes wrong:** Some Artisan/Artisan+ commands (or sub-modes like PID tuning, background profile sync) remain unimplemented or untested, causing inconsistent behavior or silent failures.

**Why it happens:** Coverage lists are built from memory; optional or rarely used commands (e.g., STOP/RESET variants, calibration, diagnostics) are skipped; command/response pairs not cross-checked against spec revisions.

**How to avoid:** Derive a command manifest directly from the current protocol spec; encode it as data (e.g., table-driven tests) to assert implemented + tested status; add golden tests for every command/response shape including error paths.

**Warning signs:** TODOs in test matrix; commands only mentioned in docs but absent in tests; failing snapshot diffs for newly added commands; unverified negative-path responses.

**Phase to address:** Command coverage expansion (Phase: Artisan/Artisan+ coverage).

---

### Pitfall 2: Formatter drift from Artisan expectations

**What goes wrong:** Responses use wrong delimiters, decimal precision, sign handling, or line endings (CRLF vs LF), leading Artisan to mis-parse or ignore data.

**Why it happens:** Formatting is hand-built per command; float formatting relies on platform defaults; inconsistent rounding between commands; lack of explicit CRLF normalization and trailing whitespace control.

**How to avoid:** Centralize formatter with explicit format spec (precision, sign, padding, separators, CRLF); add golden snapshots for all commands including negative/NaN/inf/large values; run formatter through spec-derived fixtures; enforce formatter via a single helper used by all commands.

**Warning signs:** Snapshot diffs with whitespace-only changes; platform-specific float representations in tests; Artisan UI showing stale or missing fields; ad-hoc string interpolation in command handlers.

**Phase to address:** Formatter hardening (Phase: Spec-compliant formatter).

---

### Pitfall 3: Mock UART not matching real timing/backpressure

**What goes wrong:** Integration tests pass with a permissive mock, but hardware drops bytes, interleaves responses, or blocks when buffers fill, causing field failures.

**Why it happens:** Mock UART lacks buffering limits, lacks per-byte timing, or processes synchronously; timeouts not exercised; backpressure and half-duplex ordering not modeled.

**How to avoid:** Implement a mock UART with configurable buffer sizes, per-byte delay, and backpressure; include tests for timeouts, interleaving, and partial reads/writes; keep a “strict” mock mode for CI; verify CRLF-delimited framing under load.

**Warning signs:** Tests rely on `sleep` instead of timeouts; mocks write entire frames at once; no assertions on buffer fullness or dropped bytes; integration tests never simulate interleaved commands.

**Phase to address:** Integration tests with mock UART (Phase: Command → response verification).

---

### Pitfall 4: State leakage across formatter/command handlers

**What goes wrong:** Shared mutable state (cached status, sequence counters, global formatter buffers) leaks between commands, making tests order-dependent and producing inconsistent responses.

**Why it happens:** Reusing mutable singletons or static buffers; not resetting state between tests; command handlers mutating shared structs that are assumed read-only.

**How to avoid:** Keep formatter stateless or scoped per request; clone status before mutation; add test harness reset hooks; run commands in randomized order in tests to detect coupling.

**Warning signs:** Tests pass when run individually but fail in suite; flakiness related to command ordering; use of `static mut`/globals or shared `String` buffers.

**Phase to address:** Formatter hardening & integration tests.

---

### Pitfall 5: Undefined behavior on invalid/edge inputs

**What goes wrong:** Invalid commands, malformed frames, or out-of-range values cause panics, hangs, or ambiguous responses; Artisan retries indefinitely or shows stale data.

**Why it happens:** Edge cases not codified (NaN/inf temps, negative rates, oversized payloads); error codes not standardized; parser assumes well-formed input; lack of timeout/abort paths.

**How to avoid:** Define error schema per command (codes/messages); add explicit handling for malformed frames and out-of-range values; cap payload sizes; add tests for NaN/inf/overflow and partial frames; enforce timeouts with clear error responses.

**Warning signs:** Panic traces in test logs; missing tests for invalid inputs; responses reuse last good value instead of signaling error; parser uses unchecked `unwrap` on user data.

**Phase to address:** Spec-compliant formatter & integration error-path tests.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Hardcode command strings per handler | Faster to implement missing commands | Drift vs. spec; duplicated fixes across handlers | Never; use centralized formatter/command map |
| Use relaxed mock UART (no backpressure/timeout) | Easier green tests | False confidence; hardware regressions | Only for unit tests; not for integration/CI |
| Rely on default float formatting | Less code | Locale/precision differences; spec drift | Never; specify precision/sign consistently |
| Reuse mutable global buffer for responses | Lower allocations | Order-dependent bugs; data races in async contexts | Only if guarded and reset per test (discouraged) |
| Skip negative/invalid input cases | Faster coverage | Hangs/panics in field; undefined UI behavior | Never; add explicit error-path fixtures |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Artisan/Artisan+ protocol | Assuming LF line endings or variable field order | Enforce CRLF and stable field ordering per spec; snapshot against official fixtures |
| UART hardware | Ignoring buffer limits and per-byte timing | Simulate backpressure and partial frames; assert on timeouts and retries |
| Formatter + parser | Formatting and parsing diverge (rounding, separators) | Use shared constants/helpers; golden tests for round-trip and error cases |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Logging every byte/frame in integration tests | Slow tests; timing-sensitive flakes | Gate verbose logging behind flag; default off in CI | Under load tests or slower CI runners |
| Blocking writes in handler while waiting for parser | Deadlocks or stalled responses | Use non-blocking IO with timeouts; separate read/write tasks | When command bursts arrive back-to-back |
| Large allocations per response | Heap churn; timing drift | Reuse small buffers per request scope; avoid global reuse | High-frequency telemetry commands |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Leaving debug/undocumented commands reachable in release builds | Unintended device control | Guard with feature flags; strip in release | 
| Panicking on malformed frames | DoS/reset loops | Graceful error responses; no `unwrap` on external input |
| Echoing raw user input in error messages | Info leakage | Sanitize and bound error output |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Ambiguous error responses (no code/message) | Artisan shows stale or cryptic status | Standardize error codes/messages per command |
| Missing heartbeat/keep-alive handling | UI appears frozen | Send explicit keep-alive or clear timeouts on inactivity |

## "Looks Done But Isn't" Checklist

- [ ] Command manifest matches spec (including rare commands and error responses) and is encoded in tests.
- [ ] Formatter snapshots cover CRLF, precision, sign, NaN/inf, large numbers, and trailing whitespace.
- [ ] Mock UART tests include timeouts, backpressure, partial frames, and interleaving.
- [ ] Negative/invalid inputs return explicit error codes without panics or reuse of stale data.
- [ ] Tests run in randomized order without flaking (no state leakage).

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Partial command matrix coverage | MEDIUM | Build manifest from spec; add missing handlers + tests; backfill snapshots |
| Formatter drift | MEDIUM | Introduce centralized formatter; regenerate snapshots; add CRLF/precision guards |
| Mock UART too permissive | MEDIUM | Replace mock with strict mode; add timeout/backpressure fixtures; rerun integration tests |
| State leakage between commands | HIGH | Refactor to stateless handlers; add reset hooks; rerun randomized test order |
| Undefined behavior on invalid input | HIGH | Define error schema; guard parser; add error-path tests; audit unwraps |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Partial command matrix coverage | Artisan/Artisan+ coverage | Table-driven test manifest; coverage report vs. spec |
| Formatter drift | Spec-compliant formatter | Golden snapshots across commands; CRLF/precision lint |
| Mock UART too permissive | Command → response verification | Strict mock with timeouts/backpressure; interleaving tests |
| State leakage | Formatter hardening & integration | Randomized command order tests; fixture resets |
| Undefined behavior on invalid input | Formatter & integration error-path tests | Error-code fixtures; fuzz/negative input suite |

## Sources

- Artisan/Artisan+ protocol docs (verify version used in LibreRoaster)
- Prior UART integration post-mortems (buffer/backpressure issues)
- Firmware formatter/telemetry conventions (CRLF and precision norms)

---
*Pitfalls research for: Artisan coverage + formatter hardening + mock UART integration*
*Researched: 2026-02-04*
