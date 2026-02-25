# Phase 61: USB Instrumentation Wiring - Context

**Gathered:** 2026-02-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Wire the exported `process_usb_command_data_test` into an instrumentation consumer and ensure the wiring is exercised during the planned instrumentation run so the formerly unused hook is validated without introducing any new capabilities.

</domain>

<decisions>
## Implementation Decisions

### Hook execution context
- Invoke `process_usb_command_data_test` from a dedicated host-side instrumentation task/harness rather than embedding it inside the USB queue processor, keeping the hook isolated from production command traffic.

### Trigger conditions
- The wired hook only runs during the documented instrumentation/integration run governing this milestone; it does not fire on every instrumentation trigger unless the documented run is explicitly re-executed for verification.

### Validation coverage
- Validation stops at simply executing the instrumentation path that calls the hook; no extra assertions or metric inspections are required beyond observing a successful run.

### Documentation framing
- Document the wiring in `INSTRUMENTATION_README.MD` (internal docs), focusing on the hook’s location and purpose without rehashing success criteria or spelling out rerun steps so future readers quickly understand why the hook exists.

### Claude's Discretion
- Whether the instrumentation task shares the same `ServiceContainer`/USB reader context as production or runs in an isolated container, how the hook call is structured (direct vs helper), and any extra gating/handshake logic are left to the implementer.
- Whether an explicit instrumentation mode flag/readiness handshake is required before running the hook is also left to Claude’s discretion.

</decisions>

<specifics>
## Specific Ideas

- Highlight the dedicated instrumentation task in `INSTRUMENTATION_README.MD` so people browsing the instrumentation docs see where and why the hook is wired without chasing the Rust source tree.
- Keep the queue processor untouched so the wiring remains isolated in the instrumentation task that runs during the documented integration test.

</specifics>

<deferred>
## Deferred Ideas

- None — discussion stayed within phase scope

</deferred>

---

*Phase: 61-usb-instrumentation-wiring*
*Context gathered: 2026-02-20*
