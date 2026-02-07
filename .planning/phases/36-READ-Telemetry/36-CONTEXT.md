# Phase 36: READ Telemetry - Context

**Gathered:** 2026-02-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Wire ArtisanFormatter response to READ command. When Artisan sends READ, call `format_read_response()` and return CSV telemetry. Only implements READ command wiring — other commands are separate phases.

</domain>

<decisions>
## Implementation Decisions

### Response Values
- FAN and HEATER percentages sent with one decimal place (e.g., "75.0")
- Follows existing format pattern in ArtisanFormatter

### Comment Wording
- Place comment on variable storage location
- Comment text: "store value for future et2 and bt2 support"
- Document that BT2/ET2 channels return -1 per Artisan spec

### Error Handling
- If `format_read_response()` returns error or produces malformed output:
  - Stop heater immediately
  - Panic with appropriate error message

### Claude's Discretion
- Exact CSV format details
- Variable naming for storage
- Error message wording

</decisions>

<specifics>
## Specific Ideas

- "store value for future et2 and bt2 support" — comment on variable
- One decimal format: 75.0, 100.0, etc.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 36-READ-Telemetry*
*Context gathered: 2026-02-07*
