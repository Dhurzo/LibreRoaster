# Phase 33: Module-by-Module Cleanup - Context

**Gathered:** 2026-02-06
**Status:** Ready for planning

<domain>
## Phase Boundary

Clean all src/ and tests/ Rust files by removing noise comments and preserving rationale explanations. Based on Phase 32 inventory (~615 comments across 52 files).

**Scope:**
- Clean all src/ files (44 files)
- Clean all tests/ files (8 files)
- Same Phase 33 for both, tests after src/

**Out of Scope:**
- Any new functionality or refactoring
- Comment style reformatting (only content cleanup)

</domain>

<decisions>
## Implementation Decisions

### Processing Order

**Order: Application core → Hardware drivers → Protocol modules → Control logic**

1. Application core first (app_builder, service_container, tasks)
2. Hardware drivers (uart, usb_cdc, max31856, fan, pid, ssr, board)
3. Protocol modules (input: parser, multiplexer, init_state; output: artisan)
4. Control logic (handlers, abstractions, traits, roaster_refactored)
5. Entry points (main.rs, lib.rs) — no preference on order

### Plan Structure

**Single plan, single task for all cleanup**

- One comprehensive plan: 33-01-PLAN.md
- Single atomic task that cleans all files (src/ + tests/)
- No commits during cleanup
- One commit at the end of Phase 33

**Rollback strategy:** If cleanup breaks something, revert to pre-cleanup state (single revert)

### Verification

**Verify once at end of Phase 33**

**Verification steps (in order):**
1. `cargo build` — verify compilation passes
2. `cargo test` — verify all tests pass
3. Diff review — automated check that rationale comments still exist
4. Commit changes

**Success criteria:**
- Build passes ✓
- Tests pass ✓
- No rationale comments accidentally removed ✓

**Diff review approach:** Automated check that searches for key rationale comment patterns (protocol references, workarounds, historical context) still present in cleaned files.

### Test Files

**Include test file cleanup in Phase 33**

- Clean tests/ after src/ is complete
- Apply same classification rules (remove noise, keep rationale)
- Test doc comments: Remove (same rule as src/ doc comments)

</decisions>

<specifics>
## Specific Ideas

- Protocol reference comments in input/parser.rs, input/multiplexer.rs, input/init_state.rs — KEEP
- Hardware workaround comments in hardware/ modules — KEEP
- Historical context for disabled code — KEEP
- All API doc comments (///) — REMOVE
- TODO/FIXME comments — REMOVE
- Noise comments (obvious statements, code duplication) — REMOVE

</specifics>

<deferred>
## Deferred Ideas

- Comment cleanup for README.md — v2.2
- Comment cleanup for internalDoc/*.md — v2.2
- Comment cleanup for Cargo.toml — v2.2

</deferred>

---

*Phase: 33-module-cleanup*
*Context gathered: 2026-02-06*
