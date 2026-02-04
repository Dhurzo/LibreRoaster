# Phase 4: Code Removal - Context

**Gathered:** 2026-02-04
**Status:** Ready for planning

<domain>
## Phase Boundary

Delete 7 unused modules/files identified in analysis:
- Output modules: `serial.rs`, `uart.rs`, `manager.rs`, `scheduler.rs`
- Control modules: `command_handler.rs`, `pid.rs`, `abstractions_tests.rs`

Clean up module exports in `output/mod.rs` and `control/mod.rs`.

This is straightforward deletion — no implementation choices to make about behavior.

</domain>

<decisions>
## Implementation Decisions

### Deletion approach
- Delete files entirely (not comment out or move)
- Deleted files remain in git history for reference

### Verification approach
- Run `cargo check` after each deletion batch
- Fix any compilation errors immediately
- Proceed output modules → control modules → exports

### Claude's Discretion
- Order of deletion (output first, then control)
- Which exports to remove from mod.rs files
- How to handle any edge cases during deletion

</decisions>

<specifics>
## Specific Ideas

No specific requirements — standard cleanup approach. Files confirmed unused via:
- Import analysis
- `cargo clippy` warnings
- Cross-reference with main.rs and application modules

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 04-code-removal*
*Context gathered: 2026-02-04*
