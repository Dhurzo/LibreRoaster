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
| 53 | Integrate Async Temp | Wire async temperature into control loop | PERF-01 (integration) | 1 |
| 54 | Clean Up Tech Debt | Remove dead code, fix warnings | DEBT-01, DEBT-02, DEBT-03 | 3 |
| 55 | Fix Fan Telemetry | Add get_speed override for proper fan readout | Integration gap | 1 |
| 56 | Complete Phase 51 Verification | Create VERIFICATION.md for documentation phase | Tech debt | 1 |
| 57 | Update Protocol References | Fix stale line references in PROTOCOL.md | Tech debt | 1 |

**Total:** 9 phases | 11 requirements | All milestone requirements covered ✓

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
- [x] 52-01-PLAN.md — Async MAX31856 temperature reading (PERF-01)
- [x] 52-02-PLAN.md — Separate LEDC timers for SSR and Fan (PERF-02)

---

## Phase 53: Integrate Async Temperature Reading

**Goal:** Wire async temperature reading into control loop (PERF-01 integration)

**Requirements:**
- PERF-01 (integration): Use async temperature methods in control loop

**Gap Closure:** Closes integration gap from audit - async methods exist but not called

**Success Criteria:**
1. Control loop uses `read_temperature_async()` or `read_with_retry()` instead of blocking `read_temperature()`
2. Temperature reading no longer blocks async executor

**Files to Modify:**
- src/control/tasks.rs or src/control/roaster_refactored.rs (control loop)
- src/hardware/max31856.rs (if trait changes needed)

**Plans:**
- [x] 53-01-PLAN.md — Wire async temperature into control loop
- [x] 53-02-PLAN.md — Gap closure: Fix control loop call site (not executed)
- [ ] 53-03-PLAN.md — Gap closure: Use concrete sensor types for true async

---

## Phase 54: Clean Up Tech Debt

**Goal:** Remove dead code and fix compilation issues

**Requirements:**
- DEBT-01: Remove unused fan_timer, ssr_timer fields in ledc_bus.rs
- DEBT-02: Fix 12+ compilation warnings
- DEBT-03: Fix integration tests compilation with std feature

**Success Criteria:**
1. No dead code in ledc_bus.rs
2. Zero or minimal compiler warnings
3. Integration tests compile with --features std

**Files to Modify:**
- src/hardware/ledc_bus.rs
- Various files with warnings

**Plans:**
- [x] 54-01-PLAN.md — Remove dead code (fields and functions)
- [x] 54-02-PLAN.md — Fix compilation warnings
- [x] 54-03-PLAN.md — Fix integration tests with std feature
- [x] 54-04-PLAN.md — Gap closure: Fix remaining warnings and linker errors
- [x] 54-05-PLAN.md — Gap closure: Fix uart_reader_task unused import

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
| PERF-01 | Phase 52 | Complete (not integrated) |
| PERF-02 | Phase 52 | Complete |
| PERF-01 | Phase 53 | Complete |
| DEBT-01 | Phase 54 | Complete |
| DEBT-02 | Phase 54 | Complete (3 unused import warnings fixed, 7 static_mut_refs left as-is) |
| DEBT-03 | Phase 54 | Complete (tests compile with std feature) |

---

## Phase 55: Fix Fan Telemetry

**Goal:** Add get_speed() override to FanController to fix fan telemetry

**Gap Closure:** Closes integration gap from audit - Fan::get_speed() not overridden

**Success Criteria:**
1. FanController implements get_speed() returning current_speed
2. READ response shows actual fan speed (not always 0.0)
3. Artisan telemetry displays correct fan value

**Files to Modify:**
- src/hardware/fan.rs

---

## Phase 56: Complete Phase 51 Verification

**Goal:** Create VERIFICATION.md for Phase 51 documentation phase

**Gap Closure:** Closes tech debt - Phase 51 missing verification file

**Success Criteria:**
1. VERIFICATION.md exists for Phase 51
2. Documents DOCS-01 verification (README matches PROTOCOL.md)

**Files to Create:**
- .planning/phases/51-documentation/VERIFICATION.md

---

## Phase 57: Update Protocol References

**Goal:** Fix stale line references in PROTOCOL.md

**Gap Closure:** Closes tech debt - 3 outdated line references

**Success Criteria:**
1. PROTOCOL.md line references accurate (3 locations fixed)

**Files to Modify:**
- PROTOCOL.md

---

*Roadmap created: 2026-02-18*
*Gap closure phases added: 2026-02-18*
