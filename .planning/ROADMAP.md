# Roadmap: LibreRoaster

## Overview

v4.5 is the next milestone. (Start with `/gsd-new-milestone` to define requirements.)

## Milestones

- ✅ **v4.1 Documentation Update** — Phases 62-69 (shipped 2026-02-23) — *[details](milestones/v4.1-ROADMAP.md)*
- ✅ **v4.2 Anti-windup integral** — Phases 70-72 (shipped 2026-02-24)
- ✅ **v4.3 Code Cleanup** — Phases 73-74 (shipped 2026-02-24)
- ✅ **v4.4 SSR Refactoring & Test Stubs** — Phases 75-76 (shipped 2026-02-25) — *[details](milestones/v4.4-ROADMAP.md)*
- 🚧 **v4.5 Alta Prioridad** — Phases 77-80 (in progress)

## Phases

<details>
<summary>🚧 v4.5 Alta Prioridad (In Progress)</summary>

### Phase 77: Memory Optimization
**Goal**: Eliminate heap allocation from ArtisanFormatter's BT history tracking using heapless::Deque
**Requirements**: PERF-03
**Plans**: 1 plan

- [x] 77-01 — Replace Vec<f32> with Deque<f32, 5>, replace format! with heapless::String ✓ COMPLETE

**Success Criteria**:
1. `ArtisanFormatter.bt_history` uses `heapless::Deque<f32, 5>` instead of `Vec<f32>`
2. `MutableArtisanFormatter.bt_history` uses `heapless::Deque<f32, 5>` instead of `Vec<f32>`
3. No `alloc::format!` calls in hot path; uses `core::write!` with `heapless::String`
4. All existing tests pass without modification

### Phase 78: SSR Deduplication
**Goal**: Extract detect_heat_source()Base, eliminating duplicate to SsrControl code
**Requirements**: SSR-06
**Plans**: 1 plan

- [x] 78-01 — Extract detect_heat_source to SsrControlBase ✓ COMPLETE

### Phase 79: Test Infrastructure
**Goal**: Migrate test stubs to library-accessible location
**Requirements**: TEST-06
**Plans**: 5 plans

- [x] 79-01 — Migrate stubs to src/common/mod.rs with pub(crate) visibility ✓ COMPLETE
- [x] 79-02 — Close integration-stub imports and host test target gaps (gap closure) ✓ COMPLETE
- [x] 79-03 — Publish core/alloc stub helpers through `libreroaster::common` so the shim re-export compiles (gap closure) ✓ COMPLETE
- [x] 79-04 — Host mock UART suites match emulator buffers and complete on x86_64 (gap closure) ✓ COMPLETE
- [x] 79-05 — Guard roaster_sync critical section so concurrent sensor reads no longer panic (gap closure) ✓ COMPLETE

**Success Criteria**:
1. `StubHeater`, `StubFan`, `StubThermometer` exist in `src/common/mod.rs` with `pub(crate)` visibility
2. Tests can import stubs via `crate::common::{StubHeater, StubFan, StubThermometer}`
3. Both unit tests (`#[cfg(test)]`) and integration tests (`tests/*.rs`) use the same stubs
4. All tests pass after migration

**Verification:** ✅ `passed` — host mock UART suites and `concurrent_sensor_reads_verify_async_mutex` now pass on x86_64 after draining buffers, capping heapless growth, and serializing `roaster_sync` via `TestCriticalSection`.

### Phase 80: Handler Pattern
**Goal**: Refactor process_artisan_command() to delegate to ArtisanCommandHandler
**Requirements**: REF-01
**Success Criteria**:
1. `process_artisan_command()` delegates to handler structs (no direct match in RoasterControl)
2. Large ~125-line match statement removed from roaster_refactored.rs
3. Existing Artisan command behavior unchanged (READ, START, STOP, OT1, IO3, UP, DOWN all work)
4. All integration tests pass
**Plans:** 4 plans

- [x] 80-01 — Route manual Artisan commands through the handler chain and keep the handler authoritative for manual outputs
- [ ] 80-02 — To be defined
- [ ] 80-03 — To be defined
- [ ] 80-04 — To be defined

**Verification:** ✅ `passed` — Handler Pattern manual command refactor verified (score 3/3 must-haves) with `cargo check --lib` and `cargo test --test fan_serialization --test mock_uart_integration`; report: .planning/phases/80-handler-pattern/80-VERIFICATION.md

</details>

<details>
<summary>✅ v4.1 Documentation Update (Shipped 2026-02-23)</summary>

- Phases 62-69 archived — documentation cleanup, watchdog/LEDC/regression instrumentation, automation hook docs
- README/FLASH_GUIDE now match current pinouts, targets, and automation hooks; instrumentation guide points at STATUS/REG telemetry
- WatchdogFeeder, LEDC guard, and regression runner telemetry now log `SAFETY WATCHDOG`, `SAFETY LEDC-GUARD`, and `SAFETY OT-REGRESSION` while `run_overtemp_regression` stays private
- Full phase details live in `.planning/milestones/v4.1-ROADMAP.md`

</details>

<details>
<summary>✅ v4.4 SSR Refactoring & Test Stubs (Shipped 2026-02-25)</summary>

#### Phase 75: SSR Refactoring
**Goal**: Extract common state into SsrControlBase and define SsrControlTrait to eliminate code duplication between SsrControl and SsrControlSimple.
**Requirements**: SSR-01, SSR-02, SSR-03, SSR-04, SSR-05
**Plans**: 2 plans

- [x] 75-01 — Extract SsrControlBase and traits, refactor both SSR types
- [x] 75-02 — Add missing trait implementations (gap closure)

#### Phase 76: Test Infrastructure
**Goal**: Create shared test stubs module to eliminate ~5x duplication in test helpers.
**Requirements**: TEST-01, TEST-02, TEST-03, TEST-04, TEST-05
**Plans**: 1 plan

- [x] 76-01 — Create tests/common/mod.rs with StubHeater, StubFan, StubThermometer

**Key outcomes:**
- Created SsrControlBase struct with ~90 lines of duplicated code eliminated
- Both SSR types now implement HeatSourceDetector, PeriodicCheck, StatusGetters traits
- Created tests/common/mod.rs with StubHeater, StubFan, StubThermometer using RefCell
- Added reset_channels() and collect_output() helper functions
- Fixed Cargo.toml to enable host target compilation
- Completed 2 phases (75-76) and 3 plans total

</details>

## Progress

| Phase | Milestone | Plans | Status | Completed |
|-------|-----------|-------|--------|-----------|
| 70    | v4.2      | 2/2   | Complete | 2026-02-24 |
| 71    | v4.2      | 3/3   | Complete | 2026-02-24 |
| 72    | v4.2      | 3/3   | Complete | 2026-02-24 |
| 73    | v4.3      | 1/1   | Complete | 2026-02-24 |
| 74    | v4.3      | 1/1   | Complete | 2026-02-24 |
| 75    | v4.4      | 2/2   | Complete | 2026-02-24 |
| 76    | v4.4      | 1/1   | Complete | 2026-02-25 |
| 77    | v4.5      | 1/1   | Complete | 2026-02-28 |
| 78    | v4.5      | 1/1   | Complete    | 2026-02-28 |
| 79    | v4.5      | 5/5   | ✓ Complete | 2026-02-28 |
| 80    | v4.5      | 1/4   | In progress | — |

---

## Coverage

- **v4.5 requirements:** 4 total
- **Mapped to phases:** 4 (100%)
- **Unmapped:** 0

---

*Ready for planning: `/gsd-plan-phase 77`*
