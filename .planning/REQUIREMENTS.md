# Requirements: LibreRoaster

**Defined:** 2026-04-16
**Core Value:** Artisan can read temperatures and control heater/fan during a roast session via serial connection.

## v5.3 Requirements

Requirements for the `v5.3 Deep Bug Analysis & Defect Report` milestone.

### Audit Foundation

- [ ] **AUD-01**: The milestone defines a whole-repo coverage map that names the firmware, scripts, tooling, and evidence surfaces included in the audit.
- [ ] **AUD-02**: The milestone defines a criticity rubric that ranks defects by impact on runtime behavior, safety, or evidence integrity.
- [ ] **AUD-03**: The milestone defines confidence labels that distinguish confirmed defects from likely defects and items needing more validation.
- [ ] **AUD-04**: The milestone defines a defect record schema that requires bug summary, criticity, evidence, affected areas, fix description, and post-fix validation path.

### Evidence Validation

- [ ] **EVID-01**: The milestone verifies that existing TRACE, replay, and HIL evidence paths are trustworthy before they are used as primary proof for findings.
- [ ] **EVID-02**: The milestone documents known evidence blind spots and trust limits so the report does not over-claim certainty.

### Investigation

- [ ] **INV-01**: The milestone audits firmware runtime paths including control, protocol, safety, and diagnostics behavior for likely defects.
- [ ] **INV-02**: The milestone audits scripts and tooling used for replay, reporting, and diagnostics automation for likely defects.
- [ ] **INV-03**: The milestone correlates defects across firmware and tooling boundaries and deduplicates them by broken contract or end-to-end symptom.

### Reporting

- [ ] **REP-01**: The milestone confirms high-risk findings with targeted reproduction or artifact-backed evidence appropriate to the bug class.
- [ ] **REP-02**: The milestone produces a Markdown defect report that lists each finding with bug summary, criticity, evidence, impact, and status.
- [ ] **REP-03**: The milestone produces a machine-readable defect inventory that mirrors the report's finding list.
- [ ] **REP-04**: Each confirmed or likely defect in the milestone output includes an implementation-ready fix description and a suggested post-fix validation path.

## Future Requirements

Deferred beyond this milestone.

### Evidence Expansion

- **EVID-03**: The project runs standalone automated checks for host-side evidence tooling as a dedicated audit quality gate.
- **EVID-04**: The project defines a host-vs-target proof matrix for each defect class to standardize when hardware confirmation is mandatory.

### Planning Visibility

- **PLAN-01**: The project audits planning-visible contract drift and documentation mismatches as a dedicated track.

## Out of Scope

Explicitly excluded from `v5.3`.

| Feature | Reason |
|---------|--------|
| Implementing bug fixes | This milestone is for analysis, evidence, and remediation planning only. |
| New dashboards or bug-tracking platforms | The milestone should extend current artifacts instead of creating a new platform. |
| Large observability or architecture rewrites | Out of scope unless a finding proves a minimal instrumentation change is required for evidence. |

## Traceability

Which phases cover which requirements. This section is populated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| AUD-01 | Phase 104 | Pending |
| AUD-02 | Phase 104 | Pending |
| AUD-03 | Phase 104 | Pending |
| AUD-04 | Phase 104 | Pending |
| EVID-01 | Phase 105 | Pending |
| EVID-02 | Phase 105 | Pending |
| INV-01 | Phase 106 | Pending |
| INV-02 | Phase 106 | Pending |
| INV-03 | Phase 106 | Pending |
| REP-01 | Phase 107 | Pending |
| REP-02 | Phase 109 | Pending |
| REP-03 | Phase 109 | Pending |
| REP-04 | Phase 108 | Pending |

**Coverage:**
- v5.3 requirements: 13 total
- Mapped to phases: 6 phases (104-109)
- Mapped: 13/13 ✓

---
*Requirements defined: 2026-04-16*
*Last updated: 2026-04-16 after roadmap creation*
