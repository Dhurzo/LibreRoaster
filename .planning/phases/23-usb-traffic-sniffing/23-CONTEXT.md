# Phase 23: USB Traffic Sniffing - Context

**Gathered:** 2026-02-05
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase implements the actual instrumentation of Artisan communication channels. Building on Phase 21 (defmt + bbqueue) and Phase 22 (async transport + channel prefixes), this phase adds logging calls to the USB CDC reader and writer tasks to capture all Artisan traffic.

This is the final phase of the v1.7 milestone that completes the logging infrastructure.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
- **Log Call Placement**: Where exactly in usb_reader_task and usb_writer_task to add log calls
- **Log Detail Level**: Full ASCII dump vs. parsed command summary
- **Performance Impact**: Ensure logging doesn't slow down Artisan communication

</decisions>

<specifics>
## Specific Ideas

- Use the `log_channel!` macro from Phase 22
- Log format: `[USB] RX: READ` or `[USB] TX: 185.2,192.3,-1.0,-1.0,24.5,45,75`
- Both incoming commands and outgoing responses should be logged

</specifics>

<deferred>
## Deferred Ideas

- Smart Filtering (suppress READ polls) - Future phase
- Web UI for log viewing - Future phase
- Log persistence to storage - Future phase

</deferred>

---

*Phase: 23-usb-traffic-sniffing*
*Context gathered: 2026-02-05*
