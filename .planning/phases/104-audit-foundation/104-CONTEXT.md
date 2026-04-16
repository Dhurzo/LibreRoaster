# Phase 104: Audit Foundation - Context

**Gathered:** 2026-04-16
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase defines the audit charter, coverage map, criticity rubric, confidence labels, and defect record schema that governs all subsequent investigation work in the v5.3 milestone.

</domain>

<decisions>
## Implementation Decisions

### Coverage Map Scope
- **D-01:** Audit covers source code + scripts only (firmware src/, tests/, scripts/, tooling). Excludes build artifacts and intermediate files.

### Criticity Rubric Design
- **D-02:** 4-level criticity rubric: Critical, High, Medium, Low
- **D-03:** Full criteria definitions for each level (not just labels) covering:
  - Runtime safety impact
  - Data corruption risk
  - Protocol breakage risk
  - Diagnostics falsification risk

### Confidence Label Definitions
- **D-04:** 3-tier confidence system: Confirmed / Likely / Needs Validation
  - **Confirmed:** Reproducible with evidence
  - **Likely:** Evidence points to it but not reproduced
  - **Needs Validation:** Cannot confirm with available evidence

### Defect Record Schema
- **D-05:** All 6 fields required: bug summary, criticity, evidence, affected areas, fix description, validation path
- **D-06:** Fix descriptions must be implementation-ready (specific file, function, approach)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — AUD-01, AUD-02, AUD-03, AUD-04 define what this phase must deliver

### Project Context
- `.planning/PROJECT.md` — v5.3 milestone goals, core value statement
- `.planning/STATE.md` — Current position, milestone decisions

### Roadmap
- `.planning/ROADMAP.md` — Phase 104 success criteria and plan structure

</canonical_refs>

<code_context>
## Existing Code Insights

### Project Structure
- **Firmware**: ~90+ Rust source files in `src/` covering hardware drivers, control logic, safety systems, logging
- **Scripts**: Python scripts in `scripts/` (traceability_matrix.py, replay_safe_shutdown.py, etc.)
- **Tooling**: Shell scripts in `scripts/` for quality checks, build, regression

### Reusable Assets
- No existing audit framework — this phase creates it from scratch

### Integration Points
- The audit foundation created here feeds into Phase 105 (evidence verification) and Phase 106 (static audit)

</code_context>

<specifics>
## Specific Ideas

No specific "I want it like X" moments captured — using standard audit methodology.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 104-audit-foundation*
*Context gathered: 2026-04-16*