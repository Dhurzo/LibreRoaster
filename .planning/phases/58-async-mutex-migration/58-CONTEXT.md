# Phase 58: Async Mutex Migration - Context

**Gathered:** 2026-02-19
**Status:** Ready for planning

<domain>
## Phase Boundary

Replace unsafe `take/replace` pattern with `embassy_sync::Mutex` to eliminate race condition in async sensor reading. Success criteria: build compiles, no take/replace, `with_roaster()` is async, all callers updated, no race condition, sync access available for ISR.

</domain>

<decisions>
## Implementation Decisions

### API compatibility
- **Dual API:** Keep old sync `with_roaster()` for ISR contexts, new async `with_roaster_async()` for task context
- **Naming:** New async method named `with_roaster_async` to clearly distinguish from sync version
- **Deprecation:** Add `#[deprecated]` attribute to old sync API to guide users to migrate
- **Abstraction:** Hide `embassy_sync::Mutex` behind abstraction — do not expose in public API

### Migration approach
- **Scope:** All at once — replace all take/replace in single change
- **Feature flag:** None needed — direct migration
- **Callers:** All callers updated in same PR — no deprecated API left behind
- **Transition:** Direct migration from take/replace to lock pattern, no intermediate state

### Claude's Discretion
- Testing strategy (unit vs integration tests for race verification)
- Lock contention behavior (panic, timeout, or error)
- Exact error types and messages

</decisions>

<specifics>
## Specific Ideas

No specific references or examples — standard embedded patterns apply.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 58-async-mutex-migration*
*Context gathered: 2026-02-19*
