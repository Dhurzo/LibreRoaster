# Feature Research

**Domain:** Artisan protocol edge-case fixes (embedded firmware)
**Researched:** 2026-02-17
**Confidence:** MEDIUM

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = product feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| READ response terminates with single CRLF | Artisan expects a single line terminator per response; double CRLF creates blank lines and parsing issues | LOW | Ensure formatter outputs bare CSV; append one CRLF in transport layer only (USB CDC, UART, dual output). |
| READ response stays 4-value CSV with one decimal | Current protocol spec uses 4-value CSV for READ and one-decimal precision | LOW | Keep `ET,BT,HEATER,FAN` with one decimal, no extra fields or embedded terminators. |
| ROR becomes non-zero after second sample | ROR should reflect BT change over time, not stay at 0.00 once BT changes | MEDIUM | Update `last_bt` / history during formatting so delta_bt and ROR use fresh state; maintain `ROR` with two decimals in stream format. |
| ROR resets when formatter resets | Start/stop cycles should not carry stale history | LOW | Reset `last_bt` and history on `reset()` and initial format call. |

### Differentiators (Competitive Advantage)

Features that set the product apart. Not required, but valuable.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Centralized line-termination policy | Prevents regressions across USB CDC, UART, and dual output paths | MEDIUM | Single function or boundary that appends CRLF; formatter and channel payloads stay terminator-free. |
| Protocol-focused tests for CRLF + ROR | Faster verification of edge cases | MEDIUM | Add tests to assert exactly one CRLF at output boundary and non-zero ROR after BT changes. |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Embed CRLF in formatter output | “Make formatter output complete line” | Causes double CRLF when transport also appends terminator | Keep formatter pure; append CRLF in transport only. |
| Restore legacy 7-value READ (time,ET,BT,ROR,Gas,...) | Some docs show legacy format | Breaks current spec and tests; incompatible with current parsing expectations | Keep 4-value CSV; document legacy as unsupported. |
| Add ROR/time fields to READ response | “More telemetry in READ” | Changes protocol contract and can break Artisan expectations | Keep ROR in continuous stream only; READ remains 4 values. |

## Feature Dependencies

```
READ response single-CRLF
    └──requires──> Output channel + transport writers (USB CDC, UART, dual output)

ROR non-zero after second sample
    └──requires──> BT sampling + formatter state (last_bt/history)
                       └──requires──> control loop cadence (1s or fixed interval)

Protocol tests for CRLF + ROR
    └──requires──> Host-friendly test harness + output channel visibility
```

### Dependency Notes

- **READ response single-CRLF requires output writers:** Terminator is appended in transport tasks, so the formatter must emit raw CSV only.
- **ROR non-zero after second sample requires BT sampling:** Delta and history must update per format call to reflect temperature change.
- **Protocol tests require harness visibility:** Tests should observe channel output or writer output to assert terminator behavior.

## MVP Definition

### Launch With (v1)

Minimum viable product — what's needed to validate the concept.

- [ ] Single CRLF terminator for READ responses — fixes double-CRLF edge case
- [ ] ROR updates correctly after BT changes — non-zero ROR once enough samples exist
- [ ] Tests covering CRLF + ROR behaviors — prevents regression

### Add After Validation (v1.x)

Features to add once core is working.

- [ ] Centralized terminator utility — reduce code duplication and drift
- [ ] Additional tests for dual output routing — verify channel-specific termination behavior

### Future Consideration (v2+)

Features to defer until product-market fit is established.

- [ ] Protocol fuzz tests for malformed command framing — useful, but out of scope for edge-case fixes

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Single CRLF terminator for READ | HIGH | LOW | P1 |
| ROR updates after BT changes | HIGH | MEDIUM | P1 |
| Tests for CRLF + ROR | HIGH | MEDIUM | P1 |
| Centralized terminator utility | MEDIUM | MEDIUM | P2 |
| Dual output routing tests | MEDIUM | MEDIUM | P2 |

**Priority key:**
- P1: Must have for launch
- P2: Should have, add when possible
- P3: Nice to have, future consideration

## Competitor Feature Analysis

| Feature | Competitor A | Competitor B | Our Approach |
|---------|--------------|--------------|--------------|
| READ terminator handling | N/A (protocol spec only) | N/A | Single CRLF at transport boundary |
| ROR formatting | N/A (protocol spec only) | N/A | Two-decimal ROR in stream output only |

## Sources

- `internalDoc/PROTOCOL.md` (READ format, precision, protocol notes)
- `src/output/artisan.rs` (ROR formatting, delta_bt/history usage, READ formatting)
- `src/application/tasks.rs` (READ response wiring to output channel)
- `src/hardware/usb_cdc/tasks.rs` (CRLF appended on USB CDC write)
- `src/hardware/uart/tasks.rs` (CRLF appended on UART write)

---
*Feature research for: Artisan protocol edge-case fixes*
*Researched: 2026-02-17*
