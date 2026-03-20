 # Phase 100: Error Taxonomy Completion - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Close the remaining RUST-03 gaps by converting the RoasterError/Max31856Error variants so the code compiles, push AppError diagnostics into telemetry/guards/TRACE, and stabilize the safe-shutdown flow that hinges on embassy_time timers—nothing beyond those outcomes is in scope.

</domain>

<decisions>
## Implementation Decisions

### Error variant shape strategy
- Convert RoasterError and Max31856Error entirely to struct variants unless a unit variant can be safely represented by one fixed message that never needs extra context.
- Let each variant carry as much payload as makes sense for its use case: some stay minimal, others include diagnostic context such as module or parameter names, depending on how much information Claude deems helpful.
- Keep module-local conversion helpers (peripheral drivers, helpers, etc.) that transport legacy unit variants into the new struct-backed AppError so wiring stays localized.
- AppError retryability/severity metadata is left to Claude to flagify wherever telemetry/guards/TRACE needs it.

### AppError diagnostics coverage
- Claude decides which channels (telemetry, guards, TRACE) attach the richer diagnostics, how they format and emit them, and what fallbacks apply when diagnostics cannot attach directly.

### Safe-shutdown behavior
- Claude chooses how LED blink/heartbeat behavior, pending-task handling, and success/failure signals behave while `enter_safe_shutdown()` waits on embassy_time, observing the existing guard/telemetry conventions.

### Error boundary contracts & trait wiring
- Claude picks the right boundary definitions, From/Into wiring layers, and metadata-preservation strategy so AppError remains the canonical diagnostic carrier across the entire signal path.

### Claude's Discretion
- Formatting, verbosity, instrumentation fallbacks, LED cues, and retry/cancel helpers are all Claude’s discretion for planning and execution.

</decisions>

<specifics>
## Specific Ideas
- No explicit references were provided; follow existing telemetry/guard/TRACE patterns while keeping diagnostics focused on AppError sources and Display output.
</specifics>

<deferred>
## Deferred Ideas
- None — discussion stayed within the phase boundary defined above.
</deferred>

---

*Phase: 100-error-taxonomy-completion*
*Context gathered: 2026-03-20*
