# Phase 24: Non-Blocking Logging Foundation - Context

**Gathered:** 2026-02-05
**Status:** Ready for planning
**Gap Closure:** Closes LOG-06 (Critical)

<domain>
## Phase Boundary

This phase closes the critical gap identified in the v1.7 audit: LOG-06 requires non-blocking logging using `defmt` and `bbqueue`, but neither library is implemented.

The Phase 21 summary claimed defmt + bbqueue were integrated, but the implementation uses `log::info!` with `alloc::format!` which is blocking.

This phase delivers the actual non-blocking logging foundation.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
- **Buffer Size**: Optimal bbqueue size (balance memory vs. log capacity)
- **Overflow Behavior**: Drop oldest vs. newest when buffer full
- **defmt Backend**: RTT (default) or UART if available

</decisions>

<specifics>
## Specific Ideas

- Add `defmt` and `bbqueue` dependencies to `Cargo.toml`
- Replace `log_channel!` macro to use defmt deferred formatting
- Create global bbqueue buffer with producer/consumer
- Initialize infrastructure in `main.rs` at startup
- Verify non-blocking behavior (<1μs per log call)

</specifics>

<deferred>
## Deferred Ideas

- **UART Transport (Phase 25)**: Background task that drains bbqueue to UART0
- **Hardware Verification**: Testing with actual ESP32-C3 hardware

</deferred>

---

*Phase: 24-defmt-bbqueue-foundation*
*Gap closure for: LOG-06 (Critical)*
