# Pitfalls Research

**Domain:** Embedded Artisan-like serial protocol fixes (READ terminators, delta_bt/ROR state)
**Researched:** 2026-02-17
**Confidence:** MEDIUM

## Critical Pitfalls

### Pitfall 1: Terminator mismatch (CR/LF, missing EOL)

**What goes wrong:**
READ responses end with the wrong terminator (LF vs CRLF) or omit the terminator on some frames, causing Artisan to hang, skip frames, or mis-parse fields.

**Why it happens:**
The firmware formats strings in multiple places, or a buffer path trims or appends terminators inconsistently.

**How to avoid:**
Define one canonical formatter for READ responses, including terminator, and unit-test the exact bytes. Add a golden-file test for CRLF/LF correctness.

**Warning signs:**
Host shows intermittent “No data” or “timeout,” but serial logs show data; packet captures reveal lines without the expected EOL.

**Phase to address:**
Phase 1: Protocol contract + tests.

---

### Pitfall 2: Terminator split or truncation across writes

**What goes wrong:**
The terminator is split across multiple writes or dropped due to buffer pressure, causing the host parser to stall until the next frame.

**Why it happens:**
Serial writes are chunked or non-blocking; ring buffer overflow drops tail bytes under load.

**How to avoid:**
Write READ frames atomically when possible, or ensure write loop drains the entire buffer. Add tests simulating buffer limits.

**Warning signs:**
The last two bytes of frames are missing or delayed; decoding succeeds only every N frames.

**Phase to address:**
Phase 2: Firmware implementation + serial IO tests.

---

### Pitfall 3: delta_bt/ROR computed from stale or out-of-order samples

**What goes wrong:**
delta_bt/ROR spikes or lags because the computation uses old BT values or the order of updates is reversed.

**Why it happens:**
BT update and ROR update happen in separate tasks/ISR paths; timestamping is inconsistent.

**How to avoid:**
Centralize sampling and delta computation in a single time-step function that updates BT then ROR in a defined order. Store last-sample time and value together.

**Warning signs:**
ROR is non-zero during steady temperature; spikes at every other sample; ROR changes even when BT stays constant.

**Phase to address:**
Phase 2: Firmware implementation with unit tests for sample sequences.

---

### Pitfall 4: State not reset across START/STOP or roast transitions

**What goes wrong:**
delta_bt/ROR carry over from the previous roast, causing a big initial spike or incorrect negatives after reset.

**Why it happens:**
State variables persist across protocol state changes and are not reset when a new roast begins.

**How to avoid:**
Define explicit state transitions and reset delta/ROR accumulator on START, STOP, or ROAST_END. Add tests for transition sequences.

**Warning signs:**
First frame after START shows a large ROR; restarting the host without power-cycle produces invalid deltas.

**Phase to address:**
Phase 1: Protocol contract + tests; Phase 2: Implementation.

---

### Pitfall 5: Unit/scale mismatch for delta_bt/ROR

**What goes wrong:**
ROR is scaled incorrectly (per-second vs per-minute), or integer division truncates values, producing flat or extreme ROR.

**Why it happens:**
Mismatch between sampling interval and calculation formula, or inconsistent units across modules.

**How to avoid:**
Document the units (e.g., degrees/minute). Use fixed-point or float consistently. Add tests with known input/output pairs.

**Warning signs:**
ROR values are near zero for typical roasts or 60x larger than expected; changes when sampling interval changes.

**Phase to address:**
Phase 1: Protocol contract + tests.

---

### Pitfall 6: Concurrency races between sampling and formatting

**What goes wrong:**
READ response shows mixed values from different timestamps, or delta/ROR is computed mid-update.

**Why it happens:**
Sampling updates BT and delta in ISR or separate task while formatter reads without locks.

**How to avoid:**
Use a snapshot struct or critical section when copying values for serialization. Prefer single-writer, multi-reader patterns.

**Warning signs:**
Occasional impossible combinations (e.g., ROR sign flips with unchanged BT) or non-deterministic test failures.

**Phase to address:**
Phase 2: Firmware implementation + concurrency tests.

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Hardcode terminator in multiple modules | Quick patch | Inconsistent behavior, hard to audit | Never |
| Compute delta/ROR in the formatter | Fewer files changed | Race conditions and hidden side effects | Never |
| Skip sampling timestamp | Simpler math | Incorrect ROR when interval drifts | Only in throwaway spikes |

## Integration Gotchas

Common mistakes when connecting to external services.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Artisan client | Assuming LF is acceptable everywhere | Confirm and test exact terminator (CRLF vs LF) expected by Artisan profile |
| Host sampling loop | Changing sample interval without updating ROR formula | Tie ROR computation to actual delta time, not constant |
| Serial bridge/USB | Ignoring line ending translation | Disable or account for any host-side line-ending conversion |

## Performance Traps

Patterns that work at small scale but fail as usage grows.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Excessive float formatting per frame | Jittery sampling, missed frames | Use fixed-point or preformatted buffers | At higher sample rates (>=10 Hz) |
| Serial buffer overrun under load | Missing terminators or truncated frames | Throttle output or increase buffer; measure worst-case latency | When Wi-Fi or logging adds latency |
| Heavy logging in ISR | Timing drift, ROR instability | Keep ISR minimal; offload formatting to main loop | On small MCUs like ESP32-C3 |

## Security Mistakes

Domain-specific security issues beyond general web security.

| Mistake | Risk | Prevention |
|---------|------|------------|
| Trusting inbound serial commands without bounds | Memory overwrite or unexpected state transitions | Validate input length and command set before parsing |
| Echoing raw inbound data into READ output | Protocol injection or desync | Sanitize or separate inbound/outbound buffers |

## UX Pitfalls

Common user experience mistakes in this domain.

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| ROR jitter caused by noisy sampling | Roasting decisions feel unreliable | Apply minimal smoothing with explicit window size |
| ROR not reset on new roast | Confusing first readings | Reset state on START/STOP transitions |
| Delayed READ responses | Artisan graphs lag | Measure end-to-end latency and cap response time |

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **READ terminator:** Often missing the last CR/LF on buffer pressure — verify with a byte-level capture.
- [ ] **ROR unit:** Often mis-scaled after changing sample interval — verify with known input sequence.
- [ ] **State reset:** Often missing on START/STOP — verify first frame after transition.
- [ ] **Concurrency:** Often missing snapshot/lock — verify with stress test and random delays.

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Terminator mismatch | LOW | Align terminator in formatter, update tests, reflash firmware |
| ROR spikes from stale state | MEDIUM | Add state reset and sample order fixes; recalibrate with known sequence |
| Concurrency mismatch | MEDIUM | Add snapshot copy and re-run stress tests |

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Terminator mismatch | Phase 1: Protocol contract + tests | Byte-level fixture tests for READ frames |
| Terminator split/truncation | Phase 2: Serial IO implementation | Stress tests with buffer limits |
| Stale/out-of-order delta/ROR | Phase 2: Sampling logic | Unit tests on sample sequences |
| Missing state reset | Phase 1-2: Protocol transitions | Integration test: START/STOP sequence |
| Unit/scale mismatch | Phase 1: Definitions | Golden expected outputs with known deltas |
| Concurrency race | Phase 2: Concurrency | Randomized timing tests |

## Sources

- Prior embedded serial protocol experience (unpublished)
- Common issues observed in Artisan-like device integrations (anecdotal)

---
*Pitfalls research for: Embedded Artisan protocol edge-case fixes*
*Researched: 2026-02-17*
