# Phase 75: SSR Refactoring - Context

**Gathered:** 2026-02-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Extract common state into SsrControlBase and define SsrControlTrait to eliminate code duplication between SsrControl and SsrControlSimple. Existing public API must remain fully backward compatible. All tests must pass after refactoring.

</domain>

<decisions>
## Implementation Decisions

### Trait granularity
- Multiple smaller traits organized by capability (not one monolithic trait)
- Traits grouped by capability: heat detection, periodic checks, status getters
- Which specific capabilities become traits: **Claude's discretion** — based on code analysis and usage patterns

### Default implementations
- Rich default implementations where there is shared logic
- SsrControlBase holds both state AND methods that operate on that state
- When to use defaults vs required methods: **Claude's discretion** — based on duplication analysis

### API compatibility
- **Full backward compatibility** — public API of SsrControl and SsrControlSimple must remain exactly the same
- SsrControlBase and traits live in the same module as SsrControl (not a new submodule)
- SsrControlBase and traits are fully public (not pub(crate))

### Migration strategy
- Refactor in one pass (extract base + add trait together)
- Run tests at the end of refactoring
- If tests fail mid-refactor: stop and fix before continuing

### Claude's Discretion
- Which specific capabilities become traits (detection, status, etc.)
- Exact trait method signatures and default implementations
- How much logic goes in defaults vs requires overrides
- Specific module organization details

</decisions>

<specifics>
## Specific Ideas

- "Multiple smaller traits (e.g., HeatSourceDetector, PeriodicCheck)" — from discussion
- SsrControlBase should have state + methods that use it
- Same module as existing SsrControl (not new submodule)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 75-ssr-refactoring*
*Context gathered: 2026-02-24*
