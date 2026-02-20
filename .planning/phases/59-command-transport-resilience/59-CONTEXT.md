# Phase 59: Command Transport Resilience - Context

**Gathered:** 2026-02-20
**Status:** Ready for planning

<domain>
## Phase boundary

Make Artisan command transport resilient when the USB CDC and UART0 channels both fire commands at the same time. The focus is on the shared command queue, queue_processor_task, and the CommandMultiplexer so that bursts from either interface do not drop, reorder, or stall commands even under sustained back-pressure.

## Lean scope

- Keep the existing queue_processor_task architecture (no rewrite) but add instrumentation and failure detection.
- Simulate concurrent load from both channels via a host-side integration harness instead of running on-device hardware.
- Surface any backlog detection in the README so operators know how to reproduce and interpret the new test.
</domain>

<decisions>
## Implementation decisions

- **Instrumentation:** Add queue depth counters and backlog logging inside `queue_processor_task` and expose them to the new integration test through shared state or hooks.
- **Testing:** The concurrency test runs on `x86_64-unknown-linux-gnu` using the host `ServiceContainer` (same harness used in previous plans) and exercises both the USB CDC and UART drivers simultaneously rather than requiring physical hardware.
- **Documentation:** Update `README.md` to explain the concurrency test, what it asserts, and how to interpret the logged backlog metrics.

### Claude's discretion
- How to record queue depth/backlog (metrics struct, logging, etc.)
- Which logging channel (console vs. `info!`) is best for capturing instrumentation without extra dependencies
- How to phrase the README guidance so operators know when to rerun the test
</decisions>

<specifics>
## Ideas to reuse

- `tests/mock_uart_integration.rs` already shows how to drive ServiceContainer in host tests; reuse some of its initialization scaffolding.
- `queue_processor_task` already consumes commands from an `embassy_sync::mutex::Waker`; add a lightweight counter around the `select!` or `while let` loop.
</specifics>

<deferred>
## Deferred ideas

- Hardware-in-the-loop verification on ESP32 boards (still requires actual hardware).
</deferred>

---

*Phase: 59-command-transport-resilience*
*Context gathered: 2026-02-20*
