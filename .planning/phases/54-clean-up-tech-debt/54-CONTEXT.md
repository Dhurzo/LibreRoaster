# Phase 54: Clean Up Tech Debt - Context

**Gathered:** 2026-02-18
**Status:** Ready for planning

<domain>
## Phase Boundary

Remove dead code and fix compilation issues. Specifically: remove unused fan_timer/ssr_timer fields in ledc_bus.rs, fix 12+ compilation warnings, and fix integration tests compilation with std feature.

</domain>

<decisions>
## Implementation Decisions

### Warning Handling
- Fix all compiler warnings and clippy lints
- Fix new warnings as they appear
- One-time cleanup (don't set deny level for future)
- Clean build is the goal

### Integration Tests
- Fix broken tests to compile with std feature
- Tests should work on host target (x86_64) only
- Use mock implementations for no_std compatibility
- Integration tests must meet same warning standards as main code (clean compilation)

### Dead Code Scope
- Remove all dead code across codebase, not just the two specified fields
- Keep potentially useful code (code with no current callers but might be needed later)
- Use `#[allow(dead_code)]` attributes for kept-but-potentially-unused code
- Document all kept code explaining why it's retained

</decisions>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches for code cleanup patterns.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 54-clean-up-tech-debt*
*Context gathered: 2026-02-18*
