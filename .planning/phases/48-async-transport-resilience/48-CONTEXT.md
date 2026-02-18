# Phase 48: Async Transport Resilience - Context

**Gathered:** 2026-02-17
**Status:** Ready for planning

<domain>
## Phase Boundary

Move UART and USB CDC channels onto embassy async paths so transport congestion never blocks the executor even when fans/SSRs demand attention. UART used for logging, USB for Artisan commands.

</domain>

<decisions>
## Implementation Decisions

### Transport architecture
- USB CDC receives Artisan commands (primary)
- UART dedicated to logging output
- Separate channels for each transport
- Parse-then-dispatch: Commands parsed and validated in transport layer before reaching control logic

### Command queuing
- FIFO queue for command ordering
- Queue sized large enough to handle burst commands
- When queue full: reject new commands (return nothing to Artisan, no response)
- Error handling: You decide

### Claude's Discretion
- Transport task management (independent vs shared executor)
- Exact queue size tuning
- Back-pressure strategy details
- UART logging format and verbosity

</decisions>

<specifics>
## Specific Ideas

- USB for commands, UART for logging (not bidirectional)
- Queue should be big enough that rejection is rare edge case
- No response sent when commands rejected (Artisan times out)

</specifics>

<deferred>
## Deferred Ideas

- None — discussion stayed within phase scope

</deferred>

---

*Phase: 48-async-transport-resilience*
*Context gathered: 2026-02-17*
