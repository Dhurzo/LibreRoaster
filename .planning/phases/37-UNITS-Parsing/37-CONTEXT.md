# Phase 37: UNITS Parsing - Context

**Gathered:** 2026-02-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Implement UNITS command parsing for temperature scale preference. Command parses UNITS,C or UNITS,F during init, stores preference in separate struct, no temperature conversion applied. Only implements parsing — other commands are separate phases.

</domain>

<decisions>
## Implementation Decisions

### Storage Location
- Separate struct for temperature scale preference
- Claude's discretion on exact struct name and implementation

### Response Behavior
- Silent execution — no ACK response to Artisan

### Operational Scope
- UNITS command during initialization only (Phase 17 pattern)
- Default to Celsius (C) if not specified

### ERR Conditions
- Claude's discretion on what triggers ERR (invalid mode, malformed input, etc.)

### Claude's Discretion
- Struct name and location for temperature scale storage
- ERR condition specifics
- Parser integration details

</decisions>

<specifics>
## Specific Ideas

- Default to Celsius (C) during init
- Silent execution — no response on success
- No temperature conversion applied

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 37-UNITS-Parsing*
*Context gathered: 2026-02-07*
