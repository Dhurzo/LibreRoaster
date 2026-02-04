# Requirements: LibreRoaster ARTISAN+ Testing

**Defined:** 2026-02-04  
**Core Value:** Artisan can read temperatures and control heater/fan during a roast session.

## v1 Requirements

### Parser Tests

- [x] **TEST-01**: OT1 0 parses correctly (heater off)
- [x] **TEST-02**: OT1 100 parses correctly (heater max)
- [x] **TEST-03**: IO3 0 parses correctly (fan off)
- [x] **TEST-04**: IO3 100 parses correctly (fan max)
- [x] **TEST-05**: OT1 > 100 returns error
- [x] **TEST-06**: IO3 > 100 returns error

### Formatter Tests

- [x] **TEST-07**: ArtisanFormatter formats READ response correctly
- [x] **TEST-08**: MutableArtisanFormatter formats CSV output correctly
- [x] **TEST-09**: ROR calculation from BT history works
- [x] **TEST-10**: Time format (X.XX seconds) is correct

### Integration Tests

- [ ] **TEST-11**: Example file `examples/artisan_test.rs` compiles and runs
- [ ] **TEST-12**: Command → response flow works (mocked)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Hardware UART testing | Requires ESP32 hardware |
| Real Artisan integration | Test in 2 days with actual hardware |
| Async task testing | Complex, low value for protocol verification |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| TEST-01 | Phase 1 | Complete |
| TEST-02 | Phase 1 | Complete |
| TEST-03 | Phase 1 | Complete |
| TEST-04 | Phase 1 | Complete |
| TEST-05 | Phase 1 | Complete |
| TEST-06 | Phase 1 | Complete |
| TEST-07 | Phase 2 | Complete |
| TEST-08 | Phase 2 | Complete |
| TEST-09 | Phase 2 | Complete |
| TEST-10 | Phase 2 | Complete |
| TEST-11 | Phase 3 | Pending |
| TEST-12 | Phase 3 | Pending |

**Coverage:**
- v1 requirements: 12 total
- Mapped to phases: 12
- Completed: 10 (83%)
- Pending: 2 (17%)

---

*Requirements defined: 2026-02-04*
*Last updated: 2026-02-04 after roadmap creation*
