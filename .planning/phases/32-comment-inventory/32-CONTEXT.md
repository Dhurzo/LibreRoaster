# Phase 32: Comment Inventory & Classification - Context

**Gathered:** 2026-02-06
**Status:** Ready for planning

<domain>
## Phase Boundary

Audit src/ directory and create an inventory of all comments. For each file:
- List all source files
- Count and categorize all comments
- Classify each comment as Keep (rationale) or Remove (noise)
- Create markdown table inventory in .planning/

This phase DELIVERS: Inventory document with classification, ready for Phase 33 cleanup.

</domain>

<decisions>
## Implementation Decisions

### Comment Classification Rules

**REMOVE (noise):**
- Obvious statements that repeat what code clearly shows (e.g., "loop forever" on `loop {}`)
- Code duplication comments (describing what code does)
- TODO/FIXME comments (should be tracked in issues)

**KEEP (rationale):**
- Non-obvious logic and complex algorithms
- Artisan protocol references (keep even as doc comments)
- Historical context for disabled/changed code
- Workarounds for hardware/compiler quirks

**DOC COMMENTS:**
- REMOVE all API documentation comments (/// for public items)
- REMOVE module-level documentation comments (//!)

**COMMENTED CODE:**
- Keep only if it provides historical context
- Remove alternatives or unused implementations (git history preserves them)

### Inventory Format

**Structure:** Markdown table

**Columns:**
| File | Count | Breakdown | Classification |
|------|-------|-----------|-----------------|
| src/main.rs | 42 | Line: 35, Doc: 5, Block: 2 | Noise: 12, Rationale: 8, Protocol: 5, etc. |

**Classification Categories (8 total):**
1. Rationale — explains WHY code is written a certain way
2. Non-obvious logic — complex behavior not apparent from code
3. Protocol ref — references Artisan protocol specification
4. Historical — context for disabled/changed code
5. Workarounds — hardware/compiler quirk explanations
6. TODO — to be removed
7. Noise — obvious statements, code duplication
8. Commented code — block comments

### Uncertain Comment Handling

**Threshold:** High — remove if any doubt whether a comment is rationale

**Action:** Mark uncertain comments with '?' in classification

**Deferral:** Review during Phase 33 (cleanup phase), not Phase 32

### Scope of Inventory

**All comment types included:**
- Line comments (`//`)
- Documentation comments (`///`, `//!`)
- Block comments (`/* */`)
- Commented-out code blocks
- Inner comments (`#![...]`)

**Files included:**
- All `src/**/*.rs` files
- Test files `tests/**/*.rs` included

</decisions>

<specifics>
## Specific Ideas

- Protocol references take precedence — always keep even if formatted as doc comments
- Historical context for commented code: "This explains why we disabled X in PR #42"
- "Err on the side of removing" — if uncertain, flag and defer

</specifics>

<deferred>
## Deferred Ideas

- README.md comment audit — future cleanup phase
- internalDoc/ markdown comment review — separate documentation phase
- Any comments that become "uncertain" — resolved in Phase 33

</deferred>

---

*Phase: 32-comment-inventory*
*Context gathered: 2026-02-06*
