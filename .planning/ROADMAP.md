# Roadmap: LibreRoaster

## Overview

v4.5 is the next milestone. (Start with `/gsd-new-milestone` to define requirements.)

## Milestones

- ✅ **v4.1 Documentation Update** — Phases 62-69 (shipped 2026-02-23)
- ✅ **v4.2 Anti-windup integral** — Phases 70-72 (shipped 2026-02-24)
- ✅ **v4.3 Code Cleanup** — Phases 73-74 (shipped 2026-02-24)
- ✅ **v4.4 SSR Refactoring & Test Stubs** — Phases 75-76 (shipped 2026-02-25) — *[details](milestones/v4.4-ROADMAP.md)*
- 🚧 **v4.5** — Phases 77+ (in progress)

## Phases

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

### 🚧 v4.5 (In Progress / Planned)

- [ ] Phase 77: [Name] ([N] plans)

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
