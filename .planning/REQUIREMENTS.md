# Requirements: LibreRoaster v4.5
> ✅ v4.1 Documentation Update requirements archived to `.planning/milestones/v4.1-REQUIREMENTS.md`. The section below now tracks the active v4.5 goals.

**Defined:** 2026-02-28
**Core Value:** Artisan can read temperatures and control heater/fan during a roast session via serial connection.

## v4.5 Requirements

### SSR Deduplication

- [x] **SSR-06**: Extract detect_heat_source() method from SsrControl and SsrControlSimple into SsrControlBase as trait default method or base implementation, eliminating ~30 lines of duplicate code

### Test Infrastructure

- [x] **TEST-06**: Migrate StubHeater, StubFan, StubThermometer from tests/common/mod.rs to src/common/mod.rs with pub(crate) visibility, enabling library code to use them

### Memory Optimization

- [x] **PERF-03**: Replace Vec<f32> BT history in ArtisanFormatter with heapless::Deque<f32, 5> and replace alloc::format! with core::write! in heapless::String, eliminating heap allocation in hot path

### Handler Pattern

- [x] **REF-01**: Refactor process_artisan_command() match statement (~125 lines in roaster_refactored.rs) to delegate to ArtisanCommandHandler following existing RoasterCommandHandler trait pattern from handlers.rs

## Out of Scope

| Feature | Reason |
|---------|--------|
| Adding new Artisan commands | Beyond refactoring scope |
| PID control implementation | Not part of v4.5 |
| Binary size reduction beyond heapless | Current size acceptable |
| WiFi/Web UI | Long-term roadmap |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| PERF-03 | Phase 77 | Complete |
| SSR-06 | Phase 78 | Complete |
| TEST-06 | Phase 79 | Complete |
| REF-01 | Phase 80 | Complete |

**Coverage:**
- v4.5 requirements: 4 total
- Mapped to phases: 4
- Unmapped: 0
- **Verification:** REF-01 verified 3/3 must-haves via `.planning/phases/80-handler-pattern/80-VERIFICATION.md` after targeted handler-path tests.

---
*Requirements defined: 2026-02-28*
*Last updated: 2026-02-28 after phase 77 completion*
