# Phase 45: ROR State + Regression Tests - Context

**Gathered:** 2026-02-17
**Status:** Ready for planning

<domain>
## Phase Boundary

Ensure ROR updates correctly during roast sessions and add regression tests that validate READ terminator framing and ROR update behavior. No new protocol features are added beyond this.

</domain>

<decisions>
## Implementation Decisions

### ROR update timing
- ROR becomes non-zero on the first BT change after the second sample.
- ROR tracking resets on roast session end/stop.
- ROR tracking starts at the roast session start event.
- Unchanged BT samples are treated as no change; ROR stays 0 until BT changes.

### Claude's Discretion
None specified.

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

*Phase: 45-ror-state-regression-tests*
*Context gathered: 2026-02-17*
