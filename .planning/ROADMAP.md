# Roadmap: LibreRoaster v5.3

## Milestones

- ✅ **v5.2 Architecture Hardening & Validation** - Phases 95-103 (shipped 2026-03-20)
- 🚧 **v5.3 Deep Bug Analysis & Defect Report** - Phases 104-109 (in progress)

## Phases

<details>
<summary>✅ v5.2 Architecture Hardening & Validation (Phases 95-103) - SHIPPED 2026-03-20</summary>

### Phase 95: Embeds Build Pipeline Fix
**Goal**: Remove duplicate embassy-time stubs and produce a flashable embedded build
**Plans**: 3 plans

Plans:
- [x] 95-01: Remove duplicate embassy-time stubs
- [x] 95-02: Build and verify flashable .bin
- [x] 95-03: Document embedded build command

### Phase 96: Error Taxonomy Normalization
**Goal**: Implement unified error hierarchy from hardware through AppError
**Plans**: 2 plans

Plans:
- [x] 96-01: Implement RoasterError/AppError conversions
- [x] 96-02: Expand AppError tests with error_category/error_source

### Phase 97: TRACE Instrumentation
**Goal**: Instrument full TRACE flow for regression evidence
**Plans**: 2 plans

Plans:
- [x] 97-01: Instrument TRACE flow
- [x] 97-02: Document TraceId-driven regression playback

### Phase 98: Manifest-Aware HIL Validation
**Goal**: Build manifest-aware HIL validation artifacts
**Plans**: 3 plans

Plans:
- [x] 98-01: Build manifest-aware HIL runs
- [x] 98-02: Build analysis/playbook
- [x] 98-03: Archive artifacts with metadata

### Phase 99: Diagnostics Replay Automation
**Goal**: Automate safe-shutdown replay for diagnostics audits
**Plans**: 4 plans

Plans:
- [x] 99-01: Implement safe-shutdown replay
- [x] 99-02: Generate traceability-replay.csv
- [x] 99-03: Emit replay-report.json
- [x] 99-04: Package safe-shutdown-replay.zip

</details>

### 🚧 v5.3 Deep Bug Analysis & Defect Report (In Progress)

**Milestone Goal:** Audit the whole repository to identify likely bugs, rank their criticity, and produce an implementation-ready defect report for a follow-up milestone.

- [ ] **Phase 104: Audit Foundation** - Define coverage map, criticity rubric, confidence labels, and defect record schema
- [ ] **Phase 105: Evidence Pipeline Verification** - Verify TRACE/replay/HIL evidence paths are trustworthy
- [ ] **Phase 106: Static Whole-Repo Audit** - Audit firmware, scripts, and tooling for likely defects
- [ ] **Phase 107: Targeted Reproduction & Correlation** - Confirm high-risk findings with targeted evidence
- [ ] **Phase 108: Criticity Ranking & Defect Catalog** - Prioritize and deduplicate defect findings
- [ ] **Phase 109: Roadmap Handoff Package** - Produce Markdown defect report and machine-readable inventory

## Phase Details

### Phase 104: Audit Foundation
**Goal**: Define the audit charter, coverage map, criticity rubric, confidence labels, and defect record schema that governs all subsequent investigation work.
**Depends on**: Nothing (first phase of milestone)
**Requirements**: AUD-01, AUD-02, AUD-03, AUD-04
**Success Criteria** (what must be TRUE):
  1. A documented whole-repo coverage map exists that names firmware, scripts, tooling, and evidence surfaces included in the audit.
  2. A criticity rubric exists that ranks defects by impact on runtime behavior, safety, or evidence integrity.
  3. Confidence labels exist that distinguish confirmed defects from likely defects and items needing more validation.
  4. A defect record schema exists that requires bug summary, criticity, evidence, affected areas, fix description, and post-fix validation path.
**Plans**: 2 plans

Plans:
- [ ] 104-01: Define whole-repo coverage map (firmware source, scripts/, tooling, evidence artifacts)
- [ ] 104-02: Define criticity rubric, confidence labels, and defect record schema

### Phase 105: Evidence Pipeline Verification
**Goal**: Verify that existing TRACE, replay, and HIL evidence paths are trustworthy before they are used as primary proof for findings.
**Depends on**: Phase 104
**Requirements**: EVID-01, EVID-02
**Success Criteria** (what must be TRUE):
  1. TRACE evidence pipeline is verified as producing trustworthy output.
  2. Replay artifacts are verified as reproducible for regression evidence.
  3. HIL validation chain is verified as capturing target behavior accurately.
  4. Known evidence blind spots are documented with explicit trust limits.
**Plans**: 2 plans

Plans:
- [ ] 105-01: Verify TRACE/replay/HIL evidence paths
- [ ] 105-02: Document evidence blind spots and trust limits

### Phase 106: Static Whole-Repo Audit
**Goal**: Audit firmware runtime paths, scripts, and tooling for likely defects across all subsystems.
**Depends on**: Phase 105
**Requirements**: INV-01, INV-02, INV-03
**Success Criteria** (what must be TRUE):
  1. Firmware runtime paths (control, protocol, safety, diagnostics) are audited with at least draft findings documented.
  2. Scripts and tooling (replay, reporting, diagnostics automation) are audited with at least draft findings documented.
  3. Cross-boundary defects between firmware and tooling are identified and correlated.
**Plans**: 3 plans

Plans:
- [ ] 106-01: Audit firmware runtime paths (control, protocol, safety, diagnostics)
- [ ] 106-02: Audit scripts and tooling (replay, reporting, diagnostics automation)
- [ ] 106-03: Correlate defects across firmware/tooling boundaries

### Phase 107: Targeted Reproduction & Correlation
**Goal**: Confirm high-risk findings with targeted reproduction or artifact-backed evidence appropriate to the bug class.
**Depends on**: Phase 106
**Requirements**: INV-03, REP-01
**Success Criteria** (what must be TRUE):
  1. High-risk findings are confirmed with reproducible triggers.
  2. Findings have bounded failure chains documented with artifact references.
  3. Cross-boundary defects are deduplicated by broken contract or end-to-end symptom.
**Plans**: 2 plans

Plans:
- [ ] 107-01: Confirm high-risk findings with targeted reproduction
- [ ] 107-02: Deduplicate cross-boundary defects

### Phase 108: Criticity Ranking & Defect Catalog
**Goal**: Prioritize findings by criticity and produce a deduplicated defect catalog.
**Depends on**: Phase 107
**Requirements**: REP-04
**Success Criteria** (what must be TRUE):
  1. Each confirmed or likely defect has an implementation-ready fix description.
  2. Each defect includes a suggested post-fix validation path.
  3. Defects are clustered by root cause for remediation sequencing.
**Plans**: 2 plans

Plans:
- [ ] 108-01: Assign criticity rankings to all findings
- [ ] 108-02: Produce deduplicated defect catalog with fix guidance

### Phase 109: Roadmap Handoff Package
**Goal**: Produce Markdown defect report and machine-readable defect inventory.
**Depends on**: Phase 108
**Requirements**: REP-02, REP-03
**Success Criteria** (what must be TRUE):
  1. Markdown defect report exists with bug summary, criticity, evidence, impact, and status per finding.
  2. Machine-readable defect inventory (JSON) mirrors the report's finding list.
  3. Report includes roadmap implications and remediation sequencing hints for follow-up.
**Plans**: 2 plans

Plans:
- [ ] 109-01: Produce Markdown defect report (DEFECT_REPORT.md)
- [ ] 109-02: Produce machine-readable defect inventory (defects.json)

## Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 104. Audit Foundation | 0/2 | Not started | - |
| 105. Evidence Pipeline Verification | 0/2 | Not started | - |
| 106. Static Whole-Repo Audit | 0/3 | Not started | - |
| 107. Targeted Reproduction & Correlation | 0/2 | Not started | - |
| 108. Criticity Ranking & Defect Catalog | 0/2 | Not started | - |
| 109. Roadmap Handoff Package | 0/2 | Not started | - |

---

*Roadmap created: 2026-04-16*
*For milestone: v5.3 Deep Bug Analysis & Defect Report*