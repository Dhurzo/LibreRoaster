# Phase 35: OT2 Command - Context

**Gathered:** 2026-02-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Implement fan speed control command `OT2,{n}` where n is 0-100. Command parses fan value from Artisan serial input and sets fan hardware accordingly. Follows existing OT1/IO3 parser pattern. Only implements OT2 command — other commands (READ, UNITS) are separate phases.

</domain>

<decisions>
## Implementation Decisions

### Command Format
- Command format: `OT2,{n}` exactly as OT1 pattern
- No variations — consistent with existing Artisan protocol

### Value Handling
- Decimals: Round to nearest integer (50.5 → 51)
- Out of range (>100 or <0): Clamp to 0-100 silently, stop heater
- Negative numbers: Clamp to 0 (same as OT1 behavior)
- Boundary values (0, 100): Accept and apply directly

### Response Behavior
- Silent execution — no acknowledgment response on success
- Only ERR response on parse failure

### Claude's Discretion
- Exact clamping implementation approach
- Parser integration details
- Hardware mapping details

</decisions>

<specifics>
## Specific Ideas

- "Same as OT1" — follow existing OT1 implementation pattern
- Out of range → stop heater (as stated in task description)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 35-OT2-Command*
*Context gathered: 2026-02-07*
