# Requirements: LibreRoaster v3.0

**Defined:** 2026-02-18
**Core Value:** Artisan can read temperatures and control heater/fan during a roast session via serial connection.

## v3.0 Requirements

Critical safety fixes for embedded reliability.

### Safety

- [x] **SAFE-01**: Replace make_static Use-After-Free in main.rs with StaticCell pattern (already used elsewhere in codebase)
- [x] **SAFE-02**: Replace mutable static in driver.rs get_usb_cdc_driver() with StaticCell (or add safety documentation if acceptable)
- [x] **SAFE-03**: Replace mutable static in driver.rs get_uart_driver() with StaticCell (or add safety documentation if acceptable)
- [x] **SAFE-04**: Replace ServiceContainer::get_instance() unsafe static mut with StaticCell pattern

### Test Fixes

- [x] **TEST-01**: Fix test_parse_ot2_partial_command - parser should return Err(ParseError::InvalidValue) for "OT2" without value, not Ok(SetFanSpeed(0, false))

### Documentation

- [ ] **DOCS-01**: Update README.md to reflect PROTOCOL.md format (4 values: ET,BT,HEATER,FAN instead of 7)

### Performance

- [ ] **PERF-01**: Replace blocking MAX31856 temperature read (~160ms busy-wait) with embassy-time::Timer async delay
- [ ] **PERF-02**: Separate SSR and Fan LEDC timers (SSR needs ~1Hz, Fan needs 25kHz) - use Timer0 and Timer1 separately

## Out of Scope

| Feature | Reason |
|---------|--------|
| PID control implementation | Not part of safety fixes |
| Roast profile automation | Not part of safety fixes |
| WiFi/Web UI | Not part of safety fixes |
| Telemetry channel expansion | Deferred until safety baseline stable |
| Dynamic PWM frequency reconfiguration | LEDC config validated, no changes needed |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| SAFE-01 | Phase 49 | Complete |
| SAFE-02 | Phase 49 | Complete |
| SAFE-03 | Phase 49 | Complete |
| SAFE-04 | Phase 49 | Complete |
| TEST-01 | Phase 50 | Complete |
| DOCS-01 | Phase 51 | Pending |
| PERF-01 | Phase 52 | Pending |
| PERF-02 | Phase 52 | Pending |

**Coverage:**
- v3.0 requirements: 8 total
- Mapped to phases: 8
- Unmapped: 0 ✓

---

*Requirements defined: 2026-02-18*
*Last updated: 2026-02-18 after v3.0 milestone start*
