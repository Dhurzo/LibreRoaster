```markdown
# Feature Research

**Domain:** Artisan / Artisan+ protocol polish for roast controllers
**Researched:** 2026-02-04
**Confidence:** MEDIUM

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = product feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Complete command coverage (Artisan & Artisan+) | Artisan clients assume all documented serial commands work (READ, OT1, IO3, START/STOP, SET-mode variants) | MEDIUM | Extend current OT1/IO3 coverage to all setpoint/readback commands; reject unknown opcodes cleanly |
| Strict value bounds & error responses | Operator safety and Artisan UI rely on 0–100% limits with predictable errors | LOW | Return explicit error/NAK instead of clamping; include malformed payload errors |
| Deterministic formatter output | Artisan parses decimals and CSV with fixed precision/newlines | LOW | Enforce stable decimal places, separators, newline termination; cover edge cases (NaN/inf, empty payload) |
| Command→response integration tests over mock UART | Prevent regressions in framing/latency-sensitive flows | MEDIUM | Use existing mock UART driver and infra; cover success + error paths for each command |
| Start/stop control state | Roast cycle start/stop toggles sampling and outputs | MEDIUM | Maintain state machine; ensure STOP halts outputs and streaming |
| ROR/temperature reporting cadence | Artisan expects consistent sampling cadence | MEDIUM | Use existing ROR calc; ensure formatter emits cadence and zeros when data missing |
### Differentiators (Competitive Advantage)

Features that set the product apart. Not required, but valuable.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Defensive parser diagnostics | Faster operator debugging when serial wiring or payloads are bad | MEDIUM | Return structured error codes/messages; log offending bytes; toggle verbosity for tests |
| Fuzz/soak coverage for parser & formatter | Hardens against noise and malformed frames | MEDIUM | Randomized invalid inputs through mock UART; asserts no panics and correct errors |
| Timing tolerance tests (latency/jitter) | Confidence under USB/serial jitter typical of Artisan setups | MEDIUM | Simulate delayed reads/writes; assert state consistency and no dropped frames |
| Conformance golden files for formatter | Prevents drift in decimal precision or CSV layout | LOW | Snapshot expected outputs across commands, including edge values |
| Configurable safety caps (per-command) | Allows installations to cap heater/fan independently | MEDIUM | Optional max bounds per channel; keep protocol-compliant errors when exceeded |
### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Auto-clamping invalid percentages | "Make it forgiving" | Hides errors; can overdrive hardware silently | Reject with clear error/NAK and keep last safe value |
| Silent protocol extensions (undocumented opcodes) | Custom shop integrations | Breaks Artisan compatibility; untestable by clients | Gate extensions behind explicit feature flags and separate namespace |
| Implicit unit switching or scaling | "Support Fahrenheit/percent in one" | Confuses Artisan expectations; corrupts data logs | Keep protocol units fixed; convert at client/UI layer |
| State mutation on invalid commands | "Try to be helpful" | Leads to stuck outputs or unsafe states | Make error paths side-effect free |
| Long-running blocking handlers on serial thread | "Do more work per command" | Increases jitter; Artisan timeouts | Offload heavy work; keep UART path non-blocking |

## Feature Dependencies

```
Complete command coverage
    └──requires──> Existing OT1/IO3 parsing & validation
                        └──requires──> Boundary checks already in place

Deterministic formatter output
    └──requires──> ArtisanFormatter & MutableArtisanFormatter edge-case handling

Command→response integration tests
    └──requires──> Mock UART driver + integration test harness
                        └──requires──> Example API usage fixture

Timing tolerance tests ──enhances──> Command→response integration tests

Configurable safety caps ──conflicts──> Auto-clamping invalid percentages
```

### Dependency Notes

- **Complete command coverage requires existing OT1/IO3 parsing & validation:** Extend current parser tables and reuse boundary validators to avoid duplication.
- **Deterministic formatter output requires ArtisanFormatter edge-case handling:** Formatter must reject or canonicalize NaN/inf before emitting strings.
- **Command→response integration tests require mock UART driver:** Reuse existing mock to simulate serial timing and capture full frames.
- **Timing tolerance tests enhance command→response integration tests:** Layer jitter/delay scenarios atop existing end-to-end flows.
- **Configurable safety caps conflict with auto-clamping:** Caps should emit explicit errors instead of silently clamping.

## MVP Definition

### Launch With (v1)

- [ ] Complete command coverage for Artisan/Artisan+ (including READ/START/STOP and OT1/IO3 setpoints)
- [ ] Strict bounds with explicit error responses for all commands
- [ ] Deterministic formatter outputs (precision, delimiters, newline termination) including edge-case handling
- [ ] Command→response integration tests over mock UART for all commands (success and error paths)
- [ ] Start/stop state machine correctness with sampling/streaming toggles

### Add After Validation (v1.x)

- [ ] Conformance golden files for formatter regression detection
- [ ] Defensive parser diagnostics with verbosity controls
- [ ] Fuzz/soak coverage for parser and formatter via mock UART
- [ ] Timing tolerance tests under induced jitter/delay
- [ ] Configurable per-channel safety caps above global bounds

### Future Consideration (v2+)

- [ ] Feature-flagged protocol extensions beyond Artisan spec
- [ ] Telemetry/log streaming enrichments (diagnostics channels) separate from core protocol
- [ ] Pluggable validation policies per installation profile

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Complete command coverage (Artisan/Artisan+) | HIGH | MEDIUM | P1 |
| Strict bounds + explicit errors | HIGH | LOW | P1 |
| Deterministic formatter output | HIGH | LOW | P1 |
| Command→response integration tests (mock UART) | HIGH | MEDIUM | P1 |
| Start/stop state machine correctness | HIGH | MEDIUM | P1 |
| Conformance golden files | MEDIUM | LOW | P2 |
| Parser diagnostics verbosity | MEDIUM | MEDIUM | P2 |
| Fuzz/soak coverage | MEDIUM | MEDIUM | P2 |
| Timing tolerance tests | MEDIUM | MEDIUM | P2 |
| Configurable per-channel safety caps | MEDIUM | MEDIUM | P2 |
| Feature-flagged extensions | LOW | MEDIUM | P3 |
| Telemetry/log enrichments | LOW | MEDIUM | P3 |
| Pluggable validation policies | LOW | HIGH | P3 |

**Priority key:**
- P1: Must have for launch
- P2: Should have, add when possible
- P3: Nice to have, future consideration

## Competitor Feature Analysis

| Feature | Competitor A (TC4/Artisan baseline) | Competitor B (custom controllers) | Our Approach |
|---------|-------------------------------------|-----------------------------------|--------------|
| Core command support | Implements documented Artisan serial commands; limited validation | Varies; often partial coverage | Full coverage with strict validation and explicit errors |
| Formatting | Fixed decimal precision; occasional drift in custom forks | Inconsistent; some clamp silently | Golden-file enforced, deterministic formatting |
| Error handling | Minimal NAK messages | Often silent failures | Structured errors; side-effect-free on failure |
| Testing | Sparse unit tests | Rare integration tests | Mock-UART end-to-end tests incl. jitter scenarios |

## Sources

- Artisan / Artisan+ serial protocol expectations (general domain knowledge; verify against official docs during implementation)
- Existing LibreRoaster features: OT1/IO3 parsing, ArtisanFormatter/MutableArtisanFormatter, ROR calculation, integration test infra, mock UART driver, example API usage
```
