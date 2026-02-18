# Phase 52: Performance Fixes - Context

**Gathered:** 2026-02-18
**Status:** Ready for planning

<domain>
## Phase Boundary

Replace blocking MAX31856 temperature read with async delay, and separate SSR and Fan LEDC timers (Timer0 vs Timer1). This fixes blocking I/O issues and PWM frequency conflicts.

</domain>

<decisions>
## Implementation Decisions

### MAX31856 async API
- Async token pattern (caller passes async context)
- Simple Result<f32, Error> return
- One-shot read (no cancellation support)
- In-place temperature reading

### LEDC timer assignment
- Timer0 for SSR (~1Hz zero-crossing)
- Timer1 for Fan (25kHz)
- Separate LEDC bus instances per timer
- Type-safe timer configuration at compile time

### Read timeout handling
- Retry N times then fail (not retry forever)
- 2 retries before giving up (3 total attempts)
- Fixed duration between retries (not exponential backoff)
- Reuse existing error type for failures

### Claude's Discretion
- Exact async token/builder pattern implementation
- Timer hardware constraint validation
- Specific timeout duration values
- Error type details (reuse vs new variant)

</decisions>

<specifics>
## Specific Ideas

- SSR PWM at ~1Hz is appropriate for zero-crossing control
- Fan PWM at 25kHz is standard for audible-free operation
- MAX31856 async read should integrate cleanly with existing Artisan temperature reading

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 52-performance-fixes*
*Context gathered: 2026-02-18*
