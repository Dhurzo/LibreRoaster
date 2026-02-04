# ROADMAP: LibreRoaster ARTISAN+ Testing

**Created:** 2026-02-04  
**Phase Count:** 3  
**Coverage:** 12/12 requirements mapped ✓

## Overview

This roadmap focuses on verifying ARTISAN+ protocol implementation through comprehensive testing. The approach is vertical slices: parser tests first, then formatter tests, ending with integration tests that verify the complete command → response flow.

## Phase Dependencies

```
Phase 1 (Parser Tests)
    ↓
Phase 2 (Formatter Tests)
    ↓
Phase 3 (Integration Tests)
```

Each phase builds on the previous: formatter tests depend on parser working, integration tests depend on both parser and formatter.

---

## Phase 1: Parser Tests

**Goal:** Parser correctly handles OT1 and IO3 commands including all boundary values (0, 100) and rejects invalid values (>100)

**Requirements:** TEST-01, TEST-02, TEST-03, TEST-04, TEST-05, TEST-06

**Plans:**
- [x] 01-01-PLAN.md — Add 5 boundary value tests to parser.rs ✓ Complete

**Status:** ✓ Complete (2026-02-04)

### Success Criteria

1. **Boundary value acceptance:** Parser accepts OT1 0 and OT1 100 (heater at 0% and 100%) without error
2. **Fan boundary acceptance:** Parser accepts IO3 0 and IO3 100 (fan at 0% and 100%) without error
3. **Heater validation:** Parser rejects OT1 values greater than 100 with appropriate error response
4. **Fan validation:** Parser rejects IO3 values greater than 100 with appropriate error response

### Verification Method

Run `cargo test` on parser module, observing 6 passing tests for boundary conditions.

---

## Phase 2: Formatter Tests

**Goal:** ArtisanFormatter and MutableArtisanFormatter produce correctly formatted output matching ARTISAN+ protocol specification

**Requirements:** TEST-07, TEST-08, TEST-09, TEST-10

**Plans:**
- [x] 02-01-PLAN.md — Create comprehensive unit tests for ArtisanFormatter and MutableArtisanFormatter ✓ Complete

**Status:** ✓ Complete (2026-02-04)

### Success Criteria

1. **READ response format:** ArtisanFormatter produces correctly structured READ command response
2. **CSV output:** MutableArtisanFormatter generates valid CSV-formatted output for Artisan consumption
3. **ROR calculation:** Rate of Rise calculated from BT (bean temperature) history matches expected values
4. **Time formatting:** All time values formatted as X.XX seconds (two decimal places) per protocol

### Verification Method

Run `cargo test` on formatter module, observing 4 passing tests covering all output formatting scenarios.

---

## Phase 3: Integration Tests

**Goal:** End-to-end command → response flow works correctly with mocked UART, and example file executes successfully

**Requirements:** TEST-11, TEST-12

**Plans:**
- [x] 03-01-PLAN.md — Fix example file and create integration tests with mock UART ✓ Complete

**Status:** ✓ Complete (2026-02-04)

### Success Criteria

1. **Example compiles:** `examples/artisan_test.rs` compiles without errors (API mismatch resolved)
2. **Example runs:** Example file executes successfully without panics or crashes
3. **Command flow:** Mocked UART sends command → receives expected response → command processed correctly
4. **Full cycle:** Complete READ → Parse → Format → Response flow verified with mocked hardware layer

### Verification Method

Run `cargo run --example artisan_test` and observe successful execution with mocked UART communication.

---

## Coverage Summary

| Phase | Goal | Requirements | Success Criteria |
|-------|------|--------------|-------------------|
| 1 - Parser Tests | Parser handles OT1/IO3 correctly | TEST-01 to TEST-06 | 4 |
| 2 - Formatter Tests | Formatter produces correct output | TEST-07 to TEST-10 | 4 |
| 3 - Integration Tests | End-to-end flow works | TEST-11 to TEST-12 | 4 |

**Total:** 12 requirements → 3 phases → 12 success criteria

---

*Last updated: 2026-02-04*
