# Phase 22: Async Transport & Metadata - Context

**Gathered:** 2026-02-05
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase delivers the async transport layer for logging. It focuses on creating a background task that drains the `bbqueue` buffer created in Phase 21 and outputs logs to hardware (UART0 or USB-Serial-JTAG). Additionally, it implements channel prefixing (e.g., `[USB]`) to distinguish between different communication channels.

This phase does NOT include the actual Artisan traffic sniffing (Phase 23), which is about instrumenting the communication tasks themselves.

</domain>

<decisions>
## Implementation Decisions

### Build System Integration
- **Availability**: The logging infrastructure will be always active and included in the codebase (no optional feature gating for the core logic).
- **Profile Support**: Logging remains active in both Debug and Release profiles to support field diagnostics.
- **Default Level**: The global log level defaults to `Info`.
- **Failure Policy**: If the logging infrastructure (e.g., global buffer initialization) fails to initialize during startup, the system must trigger a `panic` to ensure safety and transparency.

### Claude's Discretion
- **Transport Selection**: Choice of UART0 vs USB-Serial-JTAG for log output.
- **Channel Prefixing Format**: Exact format of the prefix (e.g., `[USB] `, `[UART] `).
- **Drain Task Priority**: Priority level of the async drain task relative to other tasks.

</decisions>

<specifics>
## Specific Ideas

- Follow the standard "deferred logging" pattern established in Phase 21.
- The drain task should be low-priority to not interfere with critical control loops.
- Logs should be tagged with their channel to distinguish USB CDC from UART traffic.

</specifics>

<deferred>
## Deferred Ideas

- **Async Transport (Phase 22)**: Current phase.
- **Channel Prefixing (Phase 22)**: Current phase.
- **Artisan Sniffing (Phase 23)**: Instrumenting the communication tasks to log raw ASCII traffic.
- **Smart Filtering**: Runtime suppression of `READ` commands to save buffer space.

</deferred>

---

*Phase: 22-async-transport*
*Context gathered: 2026-02-05*
