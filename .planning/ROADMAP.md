# Roadmap: LibreRoaster

## Milestones

- ✅ **v2.0 Code Quality Audit** — Phases 31 (shipped 2026-02-05)
- 🚧 **v2.1 Comment Rationale Cleanup** — Phases 32-34 (in progress)

---

## Progress

| Milestone | Phases | Status | Completion Date |
|-----------|--------|--------|-----------------|
| v2.0 Code Quality Audit | 31 | ✅ Complete | 2026-02-05 |
| v2.1 Comment Rationale Cleanup | 32-34 | ○ In Progress | — |

---

## v2.1 Comment Rationale Cleanup

**Goal:** Remove noise comments, keep rationale (WHY) explanations.

### Phase 32: Comment Inventory & Classification

**Goal:** Audit src/ directory, categorize all comments, identify cleanup targets.

**Requirements:** COMM-01

**Status:** ✓ Complete (1 plan)

**Plan:** 32-01-PLAN.md — Inventory all src/ and tests/ comments, classify using 8 categories, create cleanup targets for Phase 33

**Success Criteria:**
1. ✓ All src/*.rs and src/**/*.rs files listed (44 src + 8 tests = 52 files)
2. ✓ Each file's comments categorized (comment types counted per file)
3. ✓ Inventory document created in .planning/comment-inventory.md
4. ✓ Total comment count per file recorded (~615 total comments)

**Tasks:**
- [x] List all Rust source files (52 files total)
- [x] For each file, categorize comments by type
- [x] Identify rationale comments to preserve
- [x] Identify noise comments to remove
- [x] Create inventory summary (198 lines)

---

### Phase 33: Module-by-Module Cleanup

**Goal:** Clean each module, preserving rationale, removing noise.

**Requirements:** COMM-02, COMM-03, COMM-04, COMM-05, COMM-06, COMM-07, COMM-08

**Status:** ○ Planned (1 plan)

**Plan:** 33-01-PLAN.md — Clean all src/ and tests/ files in processing order: application core → hardware drivers → protocol modules → control logic → entry points

**Success Criteria:**
1. ✓ All 44 src/ files cleaned (COMM-02-07)
2. ✓ All 8 tests/ files cleaned
3. ✓ All rationale comments preserved (COMM-08)
4. ✓ cargo build passes
5. ✓ cargo test passes

**Tasks:**
- [ ] Clean src/artisan/ module
- [ ] Clean src/uart/ module
- [ ] Clean src/temperature/ module
- [ ] Clean src/control/ module
- [ ] Clean src/util/ module
- [ ] Clean src/main.rs
- [ ] Clean src/lib.rs
- [ ] Review each file to verify rationale preserved

---

### Phase 34: Verification & Ship

**Goal:** Verify build and tests pass, commit changes.

**Requirements:** COMM-09, COMM-10

**Success Criteria:**
1. `cargo build` passes without errors (COMM-09)
2. `cargo test` passes all tests (COMM-10)
3. Changes committed with clear commit message
4. STATE.md updated to complete milestone

**Tasks:**
- [ ] Run `cargo build` and fix any issues
- [ ] Run `cargo test` and verify all pass
- [ ] Review diff to ensure no rationale lost
- [ ] Commit changes
- [ ] Update STATE.md to complete milestone

---

## Phase Summary

| # | Phase | Goal | Requirements | Success Criteria |
|---|-------|------|--------------|------------------|
| 32 | Comment Inventory | Audit and categorize src/ comments | COMM-01 | 4 criteria |
| 33 | Module Cleanup | Clean all modules, preserve rationale | COMM-02-08 | 7 modules cleaned |
| 34 | Verification & Ship | Build/test pass, commit | COMM-09-10 | 4 criteria |

---

*Roadmap created: 2026-02-06*
*For current project status, see .planning/PROJECT.md*
