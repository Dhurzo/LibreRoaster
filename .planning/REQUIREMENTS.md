# Requirements: LibreRoaster v2.3

**Defined:** 2026-02-07
**Core Value:** Accurate internal documentation that reflects v2.2 implementation

## v1 Requirements (v2.3 Milestone)

### ARCHITECTURE.md Updates

- [x] **ARCH-01**: Document OT2 command handling flow (parser → handler → fan control)
- [x] **ARCH-02**: Document READ telemetry generation (status → formatter → output)
- [x] **ARCH-03**: Document UNITS command state management (parsing, storage, no conversion)
- [x] **ARCH-04**: Verify task descriptions match v2.2 implementation (control_loop_task, dual_output_task)
- [x] **ARCH-05**: Update command handler chain diagram if commands changed

### PROTOCOL.md Creation

- [x] **PROT-01**: Document all supported Artisan commands with syntax
- [x] **PROT-02**: Document READ response format (4-value CSV: ET,BT,HEATER,FAN)
- [x] **PROT-03**: Document OT2 command with decimal rounding and clamping behavior
- [x] **PROT-04**: Document UNITS command (parse only, Celsius stored internally)
- [x] **PROT-05**: Document error responses (ERR format)
- [x] **PROT-06**: Document BT2/ET2 placeholder behavior (-1 values)

### CODE_QUALITY Updates

- [ ] **QUAL-01**: Review CODE_QUALITY_ISSUES.md for v2.2 impact (new unsafe blocks?)
- [ ] **QUAL-02**: Update CODE_QUALITY_REMEDIATION.md if v2.2 addressed any issues
- [ ] **QUAL-03**: Verify unsafe block count still accurate (22 blocks baseline from v2.0)

### hardware.md Review

- [x] **HW-01**: Verify pin assignments still accurate after v2.2
- [x] **HW-02**: Document any hardware implications of OT2/fan control
- [x] **HW-03**: Check thermocouple configuration matches v2.2 (BT/ET only, no ET2/BT2)

### Cross-Reference Validation

- [x] **XREF-01**: Verify all internalDoc files reference correct milestone versions
- [x] **XREF-02**: Ensure no stale references to pre-v2.2 features
- [x] **XREF-03**: Add "Last updated" timestamps to all modified docs

## v2 Requirements (Future)

### Additional Documentation

- **DOC-01**: Developer onboarding guide
- **DOC-02**: Testing strategy document
- **DOC-03**: Release checklist

## Out of Scope

| Feature | Reason |
|---------|--------|
| README.md updates | End-user docs, separate from internalDoc |
| Flash/connection guides | Already covered in v1.8 documentation |
| Code changes | Pure documentation milestone |
| New features | Only document what exists in v2.2 |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| ARCH-01 | Phase 38 | Complete |
| ARCH-02 | Phase 38 | Complete |
| ARCH-03 | Phase 38 | Complete |
| ARCH-04 | Phase 38 | Complete |
| ARCH-05 | Phase 38 | Complete |
| PROT-01 | Phase 39 | Complete |
| PROT-02 | Phase 39 | Complete |
| PROT-03 | Phase 39 | Complete |
| PROT-04 | Phase 39 | Complete |
| PROT-05 | Phase 39 | Complete |
| PROT-06 | Phase 39 | Complete |
| QUAL-01 | Phase 40 | Pending |
| QUAL-02 | Phase 40 | Pending |
| QUAL-03 | Phase 40 | Pending |
| HW-01 | Phase 41 | Complete |
| HW-02 | Phase 41 | Complete |
| HW-03 | Phase 41 | Complete |
| XREF-01 | Phase 42 | Complete |
| XREF-02 | Phase 42 | Complete |
| XREF-03 | Phase 42 | Complete |
| XREF-01 | Phase 42 | Pending |
| XREF-02 | Phase 42 | Pending |
| XREF-03 | Phase 42 | Pending |

**Coverage:**
- v1 requirements: 18 total
- Mapped to phases: 18
- Unmapped: 0 ✓

---
*Requirements defined: 2026-02-07*
*Last updated: 2026-02-07 after milestone initialization*
