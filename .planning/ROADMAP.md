# Roadmap: LibreRoaster v3.0 Critical Safety Fixes

**Milestone:** v3.0
**Defined:** 2026-02-18
**Start Phase:** 49

## Overview

| Phase | Name | Goal | Requirements | Success Criteria |
|-------|------|------|--------------|------------------|
| 49 | Safety Static Fixes | Replace unsafe static patterns with StaticCell | SAFE-01, SAFE-02, SAFE-03, SAFE-04 | 4 |
| 50 | Test Fix | Fix parser test failure | TEST-01 | 1 |
| 51 | Documentation | Update README to match PROTOCOL | DOCS-01 | 1 |
| 52 | Performance Fixes | Async temp read, separate LEDC timers | PERF-01, PERF-02 | 2 |

**Total:** 4 phases | 8 requirements | All milestone requirements covered ✓

---

## Phase 49: Safety Static Fixes

**Goal:** Replace all unsafe static/mutable patterns with StaticCell

**Requirements:**
- SAFE-01: Replace make_static Use-After-Free in main.rs
- SAFE-02: Fix mutable static in driver.rs get_usb_cdc_driver()
- SAFE-03: Fix mutable static in driver.rs get_uart_driver()
- SAFE-04: Replace ServiceContainer::get_instance() unsafe static mut

**Success Criteria:**
1. No unsafe fn make_static in main.rs (replaced with StaticCell::init())
2. driver.rs get_usb_cdc_driver() uses StaticCell or has documented safety reasoning
3. driver.rs get_uart_driver() uses StaticCell or has documented safety reasoning
4. ServiceContainer::get_instance() uses StaticCell pattern

**Files to Modify:**
- src/main.rs (make_static function)
- src/driver.rs (get_usb_cdc_driver, get_uart_driver)
- src/service_container.rs (get_instance)

**Plans:**
- [x] 49-01-PLAN.md — Replace 4 unsafe static patterns with StaticCell (SAFE-01 to SAFE-04)

---

## Phase 50: Test Fix

**Goal:** Fix test_parse_ot2_partial_command test failure

**Requirements:**
- TEST-01: Parser returns Err(ParseError::InvalidValue) for "OT2" without value

**Success Criteria:**
1. Parser treats "OT2" (no value) as invalid, not SetFanSpeed(0)
2. test_parse_ot2_partial_command passes
3. Artisan sending malformed OT2 gets error response

**Files to Modify:**
- src/input/parser.rs (OT2 pattern matching)

**Plans:**
- [x] 50-01-PLAN.md — Fix OT2 pattern to return InvalidValue (TEST-01)

---

## Phase 51: Documentation

**Goal:** Update README to match PROTOCOL.md

**Requirements:**
- DOCS-01: README.md reflects 4-value format (ET,BT,HEATER,FAN)

**Success Criteria:**
1. README.md states READ returns 4 values
2. README.md examples show ET,BT,HEATER,FAN format
3. Matches PROTOCOL.md exactly

**Files to Modify:**
- README.md

**Plans:**
- [x] 51-01-PLAN.md — Update README Protocol section to 4-value format

---

## Phase 52: Performance Fixes

**Goal:** Fix blocking I/O and LEDC timer issues

**Requirements:**
- PERF-01: Replace blocking MAX31856 temperature read with async delay
- PERF-02: Separate SSR and Fan LEDC timers (Timer0 vs Timer1)

**Success Criteria:**
1. MAX31856 read uses embassy-time::Timer (non-blocking)
2. SSR uses separate LEDC timer from Fan
3. SSR PWM frequency is ~1Hz (zero-crossing appropriate)
4. Fan PWM remains at 25kHz

**Files to Modify:**
- src/hardware/max31856.rs (blocking read)
- src/main.rs (LEDC timer configuration)

**Plans:**
- [ ] 52-01-PLAN.md — Async MAX31856 temperature reading (PERF-01)
- [ ] 52-02-PLAN.md — Separate LEDC timers for SSR and Fan (PERF-02)

---

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| SAFE-01 | Phase 49 | Complete |
| SAFE-02 | Phase 49 | Complete |
| SAFE-03 | Phase 49 | Complete |
| SAFE-04 | Phase 49 | Complete |
| TEST-01 | Phase 50 | Complete |
| DOCS-01 | Phase 51 | Complete |
| PERF-01 | Phase 52 | Pending |
| PERF-02 | Phase 52 | Pending |

---

*Roadmap created: 2026-02-18*
