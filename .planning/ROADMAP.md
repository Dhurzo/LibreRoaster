# ROADMAP: LibreRoaster v2.4 UART Logging

**Milestone:** v2.4 UART Logging
**Previous Milestone:** v2.3 Documentation Update (ended at Phase 42)
**Starting Phase:** 43
**Defined:** 2026-02-08

---

## Overview

This roadmap covers the v2.4 milestone focused on redirecting all logging to UART0 while keeping USB Serial dedicated to Artisan commands. The goal is clean separation between debug output and protocol communication.

**Scope:** 1-2 phases covering logging redirection
**Total Requirements:** 7 LOG requirements
**Dependencies:** None (builds on v2.3 completion)

---

## Phase Structure

| Phase | Name | Goal | Requirements | Success Criteria |
|-------|------|------|--------------|------------------|
| 43 | UART Logging Redirect | All logging goes to UART0, USB Serial for Artisan only | LOG-01 to LOG-07 | 4 criteria |

---

## Phase Details

### Phase 43: UART Logging Redirect

**Goal:** All logging output redirected to UART0, USB Serial handles Artisan commands only

**Dependencies:** None (v2.3 complete)

**Status:** ✅ COMPLETED (2026-02-08)

**Requirements Covered:**
- LOG-01: All logging output redirected to UART0 ✅
- LOG-02: USB Serial handles Artisan commands only ✅
- LOG-03: No log interference on Artisan channel ✅
- LOG-04: UART logging at 115200 baud ✅
- LOG-05: Logging infrastructure uses UART0 peripheral ✅
- LOG-06: USB Serial dedicated to Artisan traffic ✅
- LOG-07: Clean separation between debug output and protocol ✅

**Success Criteria:**
1. ✅ All log macros (info!, debug!, warn!, error!) output to UART0
2. ✅ USB Serial channel shows no log output
3. ✅ Artisan commands/responses work correctly on USB Serial
4. ✅ UART0 logging verified at 115200 baud

**Plans:**
- [x] 43-01-PLAN.md — Redirect logging to UART0, clean USB Serial ✅

---

## Progress Tracking

| Phase | Status | Plans Complete | Started | Completed |
|-------|--------|----------------|---------|-----------|
| 43 | ● Complete | 1/1 | 2026-02-08 | 2026-02-08 |

**Legend:**
- ○ Not Started
- ◐ In Progress
- ◈ Planned
- ◆ Ready to Execute
- ● Complete

---

## Coverage Validation

**v2.4 Requirements:** 7 total
**Mapped to Phases:** 7
**Unmapped:** 0 ✓

| Requirement | Phase | Status |
|-------------|-------|--------|
| LOG-01 | 43 | Pending |
| LOG-02 | 43 | Pending |
| LOG-03 | 43 | Pending |
| LOG-04 | 43 | Pending |
| LOG-05 | 43 | Pending |
| LOG-06 | 43 | Pending |
| LOG-07 | 43 | Pending |

---

*Roadmap created: 2026-02-08*
*Next: Plan Phase 43 (UART Logging Redirect)*
