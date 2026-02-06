# Phase 25: UART Drain Task - Context

**Gathered:** 2026-02-05
**Status:** Ready for planning
**Gap Closure:** Closes LOG-03, integration gap (Critical)

<domain>
## Phase Boundary

This phase closes the critical gap identified in the v1.7 audit: The `drain_task.rs` file referenced in Phase 22 summary does not exist. Logs call `log_channel!` but there's no background task consuming them.

This phase creates the async drain task that reads from bbqueue and outputs to UART0.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
- **Task Priority**: Low priority to avoid blocking control loops
- **Drain Interval**: How often to check for new logs (10ms default)
- **UART Configuration**: 115200 baud, GPIO20/GPIO21 (from 22-RESEARCH.md)

</decisions>

<specifics>
## Specific Ideas

- Create `src/logging/drain_task.rs` with Embassy async task
- Use UART0 at 115200 baud (GPIO20 TX, GPIO21 RX)
- Task runs at lowest priority, yields to other tasks
- Reads from bbqueue consumer, writes formatted logs to UART
- Spawn task in `main.rs` alongside other tasks

</specifics>

<deferred>
## Deferred Ideas

- **Hardware Verification**: Testing UART0 output with terminal
- **Performance Tuning**: Optimize drain interval based on load testing

</deferred>

---

*Phase: 25-uart-drain-task*
*Gap closure for: LOG-03, Integration Gap*
