# Roadmap: LibreRoaster v1.1 Cleanup

**Created:** 2026-02-04
**Phase Count:** 4
**Coverage:** 20/20 requirements mapped ✓

## Overview

This roadmap focuses on cleaning up the codebase by removing unused code, consolidating duplicates, and fixing warnings. The approach is vertical slices: remove unused modules first, then consolidate traits, fix warnings, and verify.

## Phase Dependencies

```
Phase 4 (Code Removal)
    ↓
Phase 5 (Trait Consolidation)
    ↓
Phase 6 (Warning Fixes)
    ↓
Phase 7 (Final Cleanup)
```

Each phase builds on the previous: warning fixes depend on trait consolidation, etc.

---

## Phase 4: Code Removal

**Goal:** Delete all unused modules and clean up module exports

**Requirements:** CLEAN-01, CLEAN-02, CLEAN-03, CLEAN-04, CLEAN-05, CLEAN-06, CLEAN-07, CLEAN-08, CLEAN-09

**Plans:**
- [ ] 04-01-PLAN.md — Delete unused output modules
- [ ] 04-02-PLAN.md — Delete unused control modules
- [ ] 04-03-PLAN.md — Clean up module exports

### Success Criteria

1. **Output modules deleted:** `serial.rs`, `uart.rs`, `manager.rs`, `scheduler.rs` removed
2. **Control modules deleted:** `command_handler.rs`, `pid.rs`, `abstractions_tests.rs` removed
3. **Module exports cleaned:** Unused exports removed from `output/mod.rs` and `control/mod.rs`
4. **Build compiles:** Project builds without errors after deletions

### Verification Method

Run `cargo check` and verify no errors from deleted modules.

---

## Phase 5: Trait Consolidation

**Goal:** Remove duplicate OutputManager trait and simplify abstractions

**Requirements:** CLEAN-10, CLEAN-11

**Plans:**
- [ ] 05-01-PLAN.md — Remove OutputManager wrapper trait

### Success Criteria

1. **Wrapper removed:** OutputManager trait wrapper removed from `control/abstractions.rs`
2. **No orphan impl:** OutputManager impl block deleted
3. **Build succeeds:** Code compiles without the wrapper

### Verification Method

Run `cargo check` and verify abstractions.rs compiles correctly.

---

## Phase 6: Warning Fixes

**Goal:** Fix all unused import warnings and dead code

**Requirements:** CLEAN-12, CLEAN-13, CLEAN-14, CLEAN-15, CLEAN-16, CLEAN-17

**Plans:**
- [ ] 06-01-PLAN.md — Fix unused imports in abstractions.rs
- [ ] 06-02-PLAN.md — Remove unused fields from handlers

### Success Criteria

1. **No unused imports:** All `unused` warnings resolved
2. **No unused fields:** `output_manager` field removed from handlers
3. **Clean build:** `cargo clippy` shows no warnings

### Verification Method

Run `cargo clippy` and verify no warnings.

---

## Phase 7: Final Cleanup

**Goal:** Clean up lib.rs exports and verify

**Requirements:** CLEAN-18, CLEAN-19, CLEAN-20

**Plans:**
- [ ] 07-01-PLAN.md — Clean up lib.rs exports
- [ ] 07-02-PLAN.md — Final build verification

### Success Criteria

1. **lib.rs cleaned:** Only necessary modules exported
2. **All tests pass:** `cargo test` succeeds
3. **Clean output:** `cargo clippy` shows zero warnings

### Verification Method

Run `cargo test` and `cargo clippy` to verify.

---

## Coverage Summary

| Phase | Goal | Requirements | Success Criteria |
|-------|------|--------------|-----------------|
| 4 - Code Removal | Delete unused modules | CLEAN-01 to CLEAN-09 | 4 |
| 5 - Trait Consolidation | Simplify abstractions | CLEAN-10 to CLEAN-11 | 2 |
| 6 - Warning Fixes | Fix warnings | CLEAN-12 to CLEAN-17 | 6 |
| 7 - Final Cleanup | Verify clean build | CLEAN-18 to CLEAN-20 | 3 |

**Total:** 20 requirements → 4 phases → 15 success criteria

---

*Last updated: 2026-02-04*
