# Phase 49: Safety Static Fixes - Context

**Gathered:** 2026-02-18
**Status:** Ready for planning

<domain>
## Phase Boundary

Replace all unsafe static/mutable patterns with StaticCell to fix memory safety issues in embedded Rust code. Four specific refactoring targets:
- SAFE-01: Replace make_static in main.rs
- SAFE-02: Fix mutable static in driver.rs get_usb_cdc_driver()
- SAFE-03: Fix mutable static in driver.rs get_uart_driver()
- SAFE-04: Replace ServiceContainer::get_instance() unsafe static mut

</domain>

<decisions>
## Implementation Decisions

### Safety justification approach
- Inline documentation at each usage site explaining why the pattern is safe
- No separate SAFETY.md needed - comments within code are sufficient

### Migration strategy
- Incremental approach: replace one unsafe pattern at a time
- Run tests between each replacement to catch issues early
- Order: SAFE-01 → SAFE-02 → SAFE-03 → SAFE-04

### Verification approach
- Build must succeed after each change
- Existing test suite must pass
- No additional unit tests required for this refactoring

### Claude's Discretion
- Exact StaticCell implementation details (which import, exact API usage)
- Whether to use `make_static!` macro or manual StaticCell::init()
- Specific error handling if any initialization can fail

</decisions>

<specifics>
## Specific Ideas

No specific requirements — standard StaticCell pattern from the `StaticCell` crate is expected.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 49-safety-static-fixes*
*Context gathered: 2026-02-18*
