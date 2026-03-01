# Feature Research: v4.5 Refactoring Tasks

**Domain:** Embedded Firmware Refactoring (ESP32-C3 Coffee Roaster)
**Researched:** 2026-02-28
**Project:** LibreRoaster v4.5 - SSR deduplication continuation from v4.4
**Confidence:** HIGH (codebase verified)

---

## Executive Summary

This document analyzes the four refactoring tasks specified for v4.5. Unlike typical feature work, these are code quality improvements that:

- Eliminate duplication in SSR control
- Improve test infrastructure accessibility
- Remove heap allocations from hot paths
- Extend existing handler pattern to Artisan commands

Each task has clear technical outcomes and bounded scope.

---

## Table Stakes (Must Achieve)

Essential outcomes for v4.5 to be considered successful. These are refactoring prerequisites.

| Refactoring | Expected Outcome | Complexity | Dependencies |
|-------------|------------------|------------|--------------|
| **Task 1: Extract detect_heat_source() to SsrControlBase** | Eliminates ~30 lines of duplicate code. Both `SsrControl` (lines 213-246) and `SsrControlSimple` (lines 329-362) have identical implementations. Moving to `SsrControlBase` removes duplication. | LOW | None - pure extraction |
| **Task 2: Migrate test stubs to crate::common::*** | Stubs (`StubHeater`, `StubFan`, `StubThermometer`) move from `tests/common/mod.rs` to `src/common/mod.rs`. Makes stubs accessible to library code for mocking in integration tests. | LOW | None - structural change |
| **Task 3: Replace Vec<f32> with heapless::Deque<f32, 5>** | `ArtisanFormatter.bt_history` uses fixed-size deque. Current code at artisan.rs:51-55 manually manages 5-element sliding window with Vec (calls `remove(0)` which is O(n)). Deque does this in O(1). | LOW | None - drop-in replacement |
| **Task 4: Refactor process_artisan_command() to handler pattern** | Large match statement (roaster_refactored.rs:533-657, ~125 lines) extracted to handler structs. Uses existing `RoasterCommandHandler` trait pattern from `handlers.rs`. | MEDIUM | Requires extending handlers.rs pattern |

---

## Differentiators (Value-Adding Improvements)

Benefits beyond basic refactoring - nice-to-have outcomes.

| Refactoring | Value Proposition | Complexity | Notes |
|-------------|-------------------|------------|-------|
| **detect_heat_source extraction** | Enables unified periodic health check across SSR implementations. Future: consistent timing logic. | LOW | Adds `HeatSourceDetector` trait to base |
| **crate::common migration** | Enables doctests and example binaries to use stubs. Better documentation through runnable examples. | LOW | Requires `pub(crate)` visibility |
| **heapless::Deque** | Zero-allocation, no_std compatible. Paves way for `#![no_std]` builds. Cargo.toml already has heapless v0.9.2. | LOW | Matches 5-element window in current code |
| **Handler pattern for ArtisanCommand** | Individual handlers testable in isolation. Enables future middleware (logging, rate limiting). | MEDIUM | Follows existing RoasterCommandHandler pattern |

---

## Anti-Features (What NOT To Do)

Common mistakes during these refactorings - avoid these approaches.

| Anti-Pattern | Why Problematic | Correct Approach |
|--------------|-----------------|------------------|
| **Adding new functionality during refactoring** | Scope creep. v4.5 should only refactor existing code. | Complete extraction first, new features in separate milestones |
| **Changing public API signatures** | Breaks existing callers. v4.5 must be drop-in replacement. | Preserve trait bounds and method signatures |
| **Replacing Vec<f32> with Deque<f32, N> where N != 5** | Current code explicitly manages 5-element window. Different capacity changes semantics. | Use `Deque<f32, 5>` to match current sliding window |
| **Creating new handler trait for ArtisanCommand** | Already has `RoasterCommandHandler` in handlers.rs. Duplication. | Extend existing trait with ArtisanCommand variants |
| **Moving stubs without pub visibility** | Must be `pub(crate)` to be usable across modules. | Ensure `pub(crate)` visibility in src/common/mod.rs |

---

## Refactoring Dependencies

```
[Task 3: heapless::Deque]
    └── No dependencies - pure replacement

[Task 1: detect_heat_source extraction]
    └── No dependencies - pure extraction

[Task 2: crate::common migration]
    └── No dependencies - structural change

[Task 4: handler pattern refactor]
        └── optional──> [Task 1: detect_heat_source] (if handler needs heat source detection)
        └── optional──> [Task 2: crate::common] (if new tests need stubs)
```

### Dependency Notes

- **Tasks 1-3 are independent** - can be completed in any order
- **Task 4 (handler pattern)** has optional dependencies on Tasks 1 and 2
- **No blocking dependencies** - all four can proceed in parallel

---

## Current vs Expected State

### Task 1: detect_heat_source()

| Aspect | Current (v4.4) | Expected (v4.5) |
|--------|---------------|-----------------|
| Implementation count | 2 (SsrControl, SsrControlSimple) | 1 (SsrControlBase) |
| Lines duplicated | ~30 lines | 0 lines |
| Trait usage | HeatSourceDetector trait exists | Base struct implements directly |

### Task 2: Test stubs location

| Aspect | Current (v4.4) | Expected (v4.5) |
|--------|---------------|-----------------|
| Module path | `tests/common/mod.rs` | `src/common/mod.rs` |
| Visibility | test-only | `pub(crate)` |
| Usable from library | No | Yes |

### Task 3: bt_history type

| Aspect | Current (v4.4) | Expected (v4.5) |
|--------|---------------|-----------------|
| Type | `Vec<f32>` | `heapless::Deque<f32, 5>` |
| Allocation | Heap (dynamic) | Stack (fixed) |
| Remove(0) complexity | O(n) | O(1) amortized |

### Task 4: process_artisan_command()

| Aspect | Current (v4.4) | Expected (v4.5) |
|--------|---------------|-----------------|
| Implementation | 125-line match in RoasterControl | Handler structs delegating |
| Testability | Requires full RoasterControl | Handlers testable in isolation |
| Extensibility | Modify large function | Add new handler |

---

## MVP Definition (v4.5 Scope)

### Must Complete

- [ ] **Task 1:** `detect_heat_source()` extracted to SsrControlBase - no duplicate implementations
- [ ] **Task 2:** Test stubs migrated to `crate::common::*` with `pub(crate)` visibility
- [ ] **Task 3:** No `Vec<f32>` in artisan.rs, uses `heapless::Deque<f32, 5>`
- [ ] **Task 4:** `process_artisan_command()` delegates to handler structs

### Completion Criteria

Each refactoring is complete when:

1. **Task 1:** Both `SsrControl` and `SsrControlSimple` call base implementation - verify no duplicate `detect_heat_source` methods
2. **Task 2:** Stubs accessible via `crate::common::{StubHeater, StubFan, StubThermometer}`
3. **Task 3:** Binary size unchanged (fixed capacity equals same memory usage)
4. **Task 4:** Match statement removed from RoasterControl, handlers registered

### Testing Requirements

- [ ] All existing tests pass after refactoring
- [ ] No new compiler warnings
- [ ] Binary size change < 1KB (embedded constraint)
- [ ] No runtime behavioral changes (same outputs for same inputs)

---

## Prioritization Matrix

| Refactoring | Code Quality Impact | Risk | Priority |
|-------------|---------------------|------|----------|
| Task 3: Vec→Deque | HIGH - eliminates heap in hot path | LOW | P1 |
| Task 1: detect_heat_source | MEDIUM - removes duplication | LOW | P1 |
| Task 2: crate::common | MEDIUM - enables better testing | LOW | P2 |
| Task 4: Handler pattern | MEDIUM - enables extensibility | MEDIUM | P2 |

**Priority Rationale:**

- P1 tasks have clear, bounded scope and low risk - should be completed first
- P2 tasks require more architectural decisions (handler trait design for ArtisanCommand)
- All tasks are independent enough to parallelize

---

## Feature Interactions

### With Existing v4.4 Features v4.4 Feature

| | Interaction with v4.5 Tasks |
|--------------|------------------------------|
| SsrControlBase extracted (v4.4) | Task 1 extends this pattern further |
| Test stubs in tests/common (v4.4) | Task 2 moves these to library |
| ArtisanFormatter with READ response (v4.4) | Task 3 improves its implementation |
| Handler pattern in handlers.rs (v4.4) | Task 4 extends this pattern |

### Testing Strategy

- **Task 1:** Existing unit tests in `src/hardware/ssr.rs` should pass without modification
- **Task 2:** Update test imports to use new module path
- **Task 3:** Unit tests in artisan.rs verify same ROR calculations
- **Task 4:** Existing integration tests verify command processing unchanged

---

## Sources

- **Codebase verified:**
  - `src/hardware/ssr.rs` - lines 87-156 (SsrControlBase), 213-246, 329-362 (detect_heat_source)
  - `tests/common/mod.rs` - lines 1-317 (test stubs)
  - `src/output/artisan.rs` - lines 25, 51-55 (Vec<f32> usage)
  - `src/control/roaster_refactored.rs` - lines 533-657 (process_artisan_command)
  - `src/control/handlers.rs` - lines 1-471 (existing handler pattern)
- **Cargo.toml:** heapless v0.9.2 already in dependencies
- **Template:** /home/juan/.config/opencode/get-shit-done/templates/research-project/FEATURES.md

---

*Feature research for: LibreRoaster v4.5 refactoring tasks*
*Researched: 2026-02-28*
