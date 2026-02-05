# Phase 21: Logging Foundation - Context

**Gathered:** 2026-02-05
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase delivers the core non-blocking logging infrastructure for LibreRoaster. It focuses on integrating `defmt` as the primary logging framework and setting up `bbqueue` as a global, lock-free circular buffer. The goal is to ensure that log calls from any task are mere memory writes, preventing any blocking of the Embassy executor or the 100ms PID loop. This phase does *not* include the actual transmission of logs to hardware (Phase 22) or the sniffing of Artisan traffic (Phase 23).

</domain>

<decisions>
## Implementation Decisions

### Build System Integration
- **Availability**: The logging infrastructure will be always active and included in the codebase (no optional feature gating for the core logic).
- **Profile Support**: Logging remains active in both Debug and Release profiles to support field diagnostics.
- **Default Level**: The global log level defaults to `Info`.
- **Failure Policy**: If the logging infrastructure (e.g., global buffer initialization) fails to initialize during startup, the system must trigger a `panic` to ensure safety and transparency.

### Claude's Discretion
- **Buffer Overflow Strategy**: Choice of dropping oldest vs. newest logs when the `bbqueue` is full (balancing data loss vs. memory safety).
- **Macro Implementation**: Whether to use `defmt` macros directly or provide a lightweight internal wrapper.
- **Buffer Size**: Determining the optimal static allocation for the `bbqueue` based on current memory constraints.

</decisions>

<specifics>
## Specific Ideas

- The implementation should follow the standard "deferred logging" pattern common in high-performance embedded Rust.
- Verification of non-blocking behavior should be a priority in the testing plan for this phase.

</specifics>

<deferred>
## Deferred Ideas

- **Async Transport (Phase 22)**: The background task that drains the `bbqueue` to UART or USB-Serial-JTAG.
- **Channel Prefixing (Phase 22)**: Adding metadata like `[USB]` or `[UART]` to log messages.
- **Artisan Sniffing (Phase 23)**: Instrumenting the communication tasks to log raw ASCII traffic.
- **Smart Filtering**: Runtime suppression of `READ` commands to save buffer space.

</deferred>

---

*Phase: 21-logging-foundation*
*Context gathered: 2026-02-05*
