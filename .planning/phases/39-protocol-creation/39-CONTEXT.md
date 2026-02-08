# Phase 39: PROTOCOL.md Creation - Context

**Gathered:** 2026-02-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Create internalDoc/PROTOCOL.md with complete Artisan protocol specification. This document will serve as the authoritative reference for integration partners and developers communicating with LibreRoaster. Must document all supported commands, response formats, error handling, and behavior specifics including OT2 rounding/clamping and UNITS parse-only behavior.

</domain>

<decisions>
## Implementation Decisions

### Document structure/navigation
- **Commands organized by workflow** (Setup → Control → Monitoring)
  - Logical flow matches typical integration sequence
  - Easier for developers to understand the conversation pattern
- **No quick-start section needed** — straight to reference documentation
  - Assume readers are already familiar with serial communication
  - Keep document dense and scannable

### Detail depth/examples
- **Standard technical reference level**: Syntax + description + parameters + examples
  - Each command: syntax definition, purpose description, parameter table, 1-2 realistic examples
- **Realistic values in examples** (not placeholders)
  - OT2 75.5, READ 185.3,201.4,45,80
  - Shows actual expected data format
- **OT2 rounding/clamping: Brief mention only**
  - Include in command description
  - No step-by-step algorithm section (already documented in ARCHITECTURE.md)

### Visual presentation
- **Tables for command parameters**: Parameter | Type | Range | Description
  - Structured, scannable, easy to compare across commands
- **Include ASCII sequence diagram for OT2 flow**
  - Complex parsing/handling flow benefits from visualization
  - Reference existing diagram from ARCHITECTURE.md
- **Quick-reference command table at end**
  - Appendix-style summary for quick lookups
  - All commands in compact format

### Claude's Discretion
- READ response format detail placement (lean toward dedicated section)
- Error response documentation approach (lean toward central section with per-command references)
- UNITS "no conversion" caveat presentation (lean toward clear implementation note)
- Code block example style (lean toward clean first, then annotated breakdown)
- ASCII diagram style and complexity
- Table formatting specifics

</decisions>

<specifics>
## Specific Ideas

- Structure follows typical hardware integration pattern: Connection → Configuration → Control → Monitoring
- Parameter tables should include range validation info (0-100 for OT2, C/F for UNITS)
- Examples should show both successful commands and realistic response values
- Reference existing ARCHITECTURE.md for internal flow details where appropriate
- Include note about 4-value READ format (not 7-value) to prevent confusion with older documentation

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 39-protocol-creation*
*Context gathered: 2026-02-07*
