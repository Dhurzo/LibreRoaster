# ROADMAP: LibreRoaster v2.3 Documentation Update

**Milestone:** v2.3 Documentation Update  
**Previous Milestone:** v2.2 Comandos de Entrada (ended at Phase 37)  
**Starting Phase:** 38  
**Defined:** 2026-02-07  

---

## Overview

This roadmap covers the v2.3 milestone focused on updating all internal documentation to accurately reflect the v2.2 implementation. The primary goal is ensuring architecture diagrams, protocol specifications, and quality metrics match the actual code state, enabling reliable onboarding and maintenance.

**Scope:** 5 phases covering ARCHITECTURE.md, PROTOCOL.md, CODE_QUALITY files, hardware.md, and cross-reference validation.  
**Total Requirements:** 18 v1 requirements mapped across phases.  
**Dependencies:** Sequential; each phase builds on v2.2 code completion.  

---

## Phase Structure

| Phase | Name | Goal | Requirements | Success Criteria |
|-------|------|------|--------------|------------------|
| 38 | ARCHITECTURE.md Updates | ARCHITECTURE.md accurately documents v2.2 implementation including OT2, READ, and UNITS command flows | 5 (ARCH-01 to ARCH-05) | 4 criteria |
| 39 | PROTOCOL.md Creation | PROTOCOL.md exists with complete Artisan protocol specification | 6 (PROT-01 to PROT-06) | 5 criteria |
| 40 | CODE_QUALITY Updates | CODE_QUALITY files reflect current v2.2 implementation state | 3 (QUAL-01 to QUAL-03) | 3 criteria |
| 41 | hardware.md Review | hardware.md accurately reflects v2.2 pin assignments and hardware configuration | 3 (HW-01 to HW-03) | 3 criteria |
| 42 | Cross-Reference Validation | All documentation references are current and consistent | 3 (XREF-01 to XREF-03) | 3 criteria |

---

## Phase Details

### Phase 38: ARCHITECTURE.md Updates

**Goal:** ARCHITECTURE.md accurately documents v2.2 implementation including OT2, READ, and UNITS command flows

**Dependencies:** None (v2.2 implementation complete)

**Requirements Covered:**
- ARCH-01: Document OT2 command handling flow (parser → handler → fan control)
- ARCH-02: Document READ telemetry generation (status → formatter → output)
- ARCH-03: Document UNITS command state management (parsing, storage, no conversion)
- ARCH-04: Verify task descriptions match v2.2 implementation (control_loop_task, dual_output_task)
- ARCH-05: Update command handler chain diagram if commands changed

**Success Criteria:**
1. OT2 command flow documented from parser through fan control output
2. READ telemetry generation documented with 4-value CSV format (ET,BT,HEATER,FAN)
3. UNITS command documented showing parse → store (no conversion) behavior
4. Task descriptions match actual v2.2 implementation code

**Plans:**
- [x] 38-01-PLAN.md — Audit and update ARCHITECTURE.md with v2.2 command flows

---

### Phase 39: PROTOCOL.md Creation

**Goal:** PROTOCOL.md exists with complete Artisan protocol specification

**Dependencies:** Phase 38 (architecture understanding required for protocol accuracy)

**Requirements Covered:**
- PROT-01: Document all supported Artisan commands with syntax
- PROT-02: Document READ response format (4-value CSV: ET,BT,HEATER,FAN)
- PROT-03: Document OT2 command with decimal rounding and clamping behavior
- PROT-04: Document UNITS command (parse only, Celsius stored internally)
- PROT-05: Document error responses (ERR format)
- PROT-06: Document BT2/ET2 placeholder behavior (-1 values)

**Success Criteria:**
1. All 5+ Artisan commands documented with exact syntax and parameters
2. READ response format documented (4-value CSV: ET,BT,HEATER,FAN)
3. OT2 behavior documented showing 0-100% rounding and clamping
4. Error response format documented with all error codes
5. Placeholder values documented for ET2/BT2 (-1) when not configured

**Plans:**
- [x] 39-01-PLAN.md — Create PROTOCOL.md with complete Artisan protocol specification

---

### Phase 40: CODE_QUALITY Updates

**Goal:** CODE_QUALITY files reflect current v2.2 implementation state

**Dependencies:** Phase 39 (protocol documentation may inform quality assessment)

**Requirements Covered:**
- QUAL-01: Review CODE_QUALITY_ISSUES.md for v2.2 impact (new unsafe blocks?)
- QUAL-02: Update CODE_QUALITY_REMEDIATION.md if v2.2 addressed any issues
- QUAL-03: Verify unsafe block count still accurate (22 blocks baseline from v2.0)

**Success Criteria:**
1. CODE_QUALITY_ISSUES.md reviewed and updated for any v2.2 changes
2. CODE_QUALITY_REMEDIATION.md updated if v2.2 resolved existing issues
3. Unsafe block count verified and documented (current vs v2.0 baseline)

**Plans:**
- [x] 40-01-PLAN.md — Run cargo-geiger, review v2.2 files, update CODE_QUALITY documentation
- [ ] 40-02-PLAN.md — Fix unsafe block count discrepancy (24 vs 22 documented)

---

### Phase 41: hardware.md Review

**Goal:** hardware.md accurately reflects v2.2 pin assignments and hardware configuration

**Dependencies:** Phase 40 (quality review may reveal hardware-related code changes)

**Requirements Covered:**
- HW-01: Verify pin assignments still accurate after v2.2
- HW-02: Document any hardware implications of OT2/fan control
- HW-03: Check thermocouple configuration matches v2.2 (BT/ET only, no ET2/BT2)

**Success Criteria:**
1. All pin assignments verified against current v2.2 code
2. OT2 fan control hardware implications documented
3. Thermocouple configuration confirmed as BT/ET only with placeholders

**Plans:**
- [x] 41-01-PLAN.md — Update hardware.md to v2.2 specifications (READ format, OT2 command, timestamp)

---

### Phase 42: Cross-Reference Validation

**Goal:** All documentation references are current and consistent

**Dependencies:** Phases 38-41 (all document updates complete)

**Requirements Covered:**
- XREF-01: Verify all internalDoc files reference correct milestone versions
- XREF-02: Ensure no stale references to pre-v2.2 features
- XREF-03: Add "Last updated" timestamps to all modified docs

**Success Criteria:**
1. All internalDoc files reference v2.2/v2.3 milestone versions
2. No stale references to pre-v2.2 features remain
3. All modified documentation includes "Last updated" timestamps

---

## Progress Tracking

| Phase | Status | Plans Complete | Started | Completed |
|-------|--------|----------------|---------|-----------|
| 38 | ● Complete | 1/1 | 2026-02-07 | 2026-02-07 |
| 39 | ● Complete | 1/1 | 2026-02-08 | 2026-02-08 |
| 40 | ● Complete | 2/2 | 2026-02-08 | 2026-02-08 |
| 41 | ● Complete | 1/1 | 2026-02-08 | 2026-02-08 |
| 42 | ● Complete | 1/1 | 2026-02-08 | 2026-02-08 |

**Legend:**
- ○ Not Started
- ◐ In Progress
- ◈ Planned
- ◆ Ready to Execute
- ● Complete

---

## Coverage Validation

**v1 Requirements:** 18 total  
**Mapped to Phases:** 18  
**Unmapped:** 0 ✓

---

*Roadmap archived: 2026-02-08*  
*See: .planning/milestones/v2.3-DOCUMENTATION_UPDATE.md for full details*
