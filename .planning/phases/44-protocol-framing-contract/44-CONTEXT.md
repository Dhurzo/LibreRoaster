# Phase 44: Protocol Framing Contract - Context

**Gathered:** 2026-02-17
**Status:** Ready for planning

<domain>
## Phase Boundary

READ responses use exact CSV framing with a single CRLF terminator, consistently across USB CDC and UART output paths.

</domain>

<decisions>
## Implementation Decisions

### Response strictness
- Auto-correct to spec when a response would violate framing; do not drop or error.
- Strict CRLF-only line endings.
- No spaces after commas (exact `a,b,c,d`).
- Never include extra lines or prefixes/suffixes; a single CSV line only.

### Missing/invalid values
- Use `0.0` as the placeholder when a sensor value is unavailable.
- If one value is invalid, still send the other fields with the placeholder for that field.
- Clamp NaN/inf to `0.0`.
- During idle/non-roast state, return live readings when available.

### Claude's Discretion
- None specified.

</decisions>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 44-protocol-framing-contract*
*Context gathered: 2026-02-17*
