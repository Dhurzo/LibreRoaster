# Requirements: LibreRoaster

**Defined:** 2026-02-06
**Core Value:** Artisan can read temperatures and control heater/fan during a roast session via serial connection.

## v2.1 Requirements

Comment cleanup across all Rust source files. Keep rationale (WHY), remove noise (WHAT).

### COMMENT-CLEANUP Scope

**Files in scope:**
- `src/**/*.rs` — all Rust source files

**Comment types to REMOVE (noise):**
- Comments that duplicate what the code clearly does
- Obvious statements adding no new information
- Closing brace comments (`// }`)
- Commented-out code blocks
- Redundant single-line explanations
- TODO/FIXME without ticket references (track separately)

**Comment types to KEEP (rationale):**
- Design decisions explaining WHY a particular approach was chosen
- Non-obvious behavior that isn't apparent from code
- References to external specifications (Artisan protocol, etc.)
- Workarounds for hardware/compiler quirks with explanation
- Complex algorithm rationale (only if not self-documenting)

### COMMENT-CLEANUP Categories

- [x] **COMM-01**: Audit src/ and identify all comment categories
- [ ] **COMM-02**: Remove obvious/WHAT comments from artisan/ module
- [ ] **COMM-03**: Remove obvious/WHAT comments from uart/ module
- [ ] **COMM-04**: Remove obvious/WHAT comments from temperature/ module
- [ ] **COMM-05**: Remove obvious/WHAT comments from control/ module
- [ ] **COMM-06**: Remove obvious/WHAT comments from util/ module
- [ ] **COMM-07**: Remove obvious/WHAT comments from main.rs and lib.rs
- [ ] **COMM-08**: Preserve rationale comments with design explanations
- [ ] **COMM-09**: Verify build passes after cleanup
- [ ] **COMM-10**: Verify tests pass after cleanup

## Out of Scope

| Feature | Reason |
|---------|--------|
| Doc comments (///) | API documentation is valuable |
| Protocol reference comments | Links to Artisan spec are useful context |
| Embedded doc comments (//! | Module-level docs serve a purpose |
| TODO/FIXME with ticket references | Should be resolved, not removed |
| Comment style reformatting | Focus on content, not style |

## Deferred (v2+)

- README.md comment audit
- internalDoc/ *.md comment audit
- Cargo.toml comment review

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| COMM-01 | Phase 32 | Complete |
| COMM-02 | Phase 33 | In Progress |
| COMM-03 | Phase 33 | In Progress |
| COMM-04 | Phase 33 | In Progress |
| COMM-05 | Phase 33 | In Progress |
| COMM-06 | Phase 33 | In Progress |
| COMM-07 | Phase 33 | In Progress |
| COMM-08 | Phase 33 | In Progress |
| COMM-09 | Phase 34 | Pending |
| COMM-10 | Phase 34 | Pending |

**Coverage:**
- v2.1 requirements: 10 total
- Mapped to phases: 10
- Unmapped: 0 ✓

---

*Requirements defined: 2026-02-06*
*Last updated: 2026-02-06 after milestone v2.1 started*
