# Project Research Summary

**Project:** LibreRoaster v4.5 Refactoring
**Domain:** Embedded Firmware Refactoring (ESP32-C3 Coffee Roaster Control)
**Researched:** 2026-02-28
**Confidence:** HIGH

## Executive Summary

LibreRoaster v4.5 is a focused refactoring milestone targeting four specific code quality improvements in an embedded Rust firmware project. Unlike typical feature development, this release eliminates technical debt: duplicate SSR control implementations, inaccessible test stubs, heap allocations in hot paths, and a large match statement in command processing. All four refactoring tasks can be completed with **zero new dependencies** — the existing heapless v0.9.2 provides the `Deque<f32, N>` type needed for the memory optimization, and the handler delegation pattern is already established in v4.4.

The key risk is **scope creep**: these refactorings must remain pure refactoring without adding new functionality. The embedded context introduces additional constraints (no heap allocations, Send+Sync requirements for Embassy async tasks, no_std compatibility) that must be preserved. The research identifies 19 potential pitfalls organized by severity, with critical issues centered on breaking embedded-hal trait bounds and losing Send+Sync safety during deduplication.

## Key Findings

### Recommended Stack

All v4.5 refactoring tasks use existing dependencies with no changes required:

- **heapless 0.9.2** — Already provides `Deque<f32, N>` for zero-allocation ring buffer; no upgrade needed
- **embedded-hal 1.0.0** — Stable trait bounds must be preserved during SSR refactoring
- **embassy-rs 0.5.0+** — Async executor requires Send+Sync bounds on shared types
- **portable-atomic 1.13** — Already in use for atomic operations

**No new dependencies needed.** The v4.5 work is purely refactoring within existing patterns.

### Expected Features

The v4.5 milestone has four specific refactoring tasks (no new functionality):

**Table Stakes (Must Achieve):**

- **Task 1: detect_heat_source() extraction** — Move duplicate ~30 lines from both `SsrControl` and `SsrControlSimple` to `SsrControlBase`, following the v4.4 delegation pattern
- **Task 2: Test stubs migration** — Move `StubHeater`, `StubFan`, `StubThermometer` from `tests/common/mod.rs` to `src/common/mod.rs` with `pub(crate)` visibility
- **Task 3: heapless::Deque migration** — Replace `Vec<f32>` with `Deque<f32, 5>` in `ArtisanFormatter.bt_history`, eliminating heap allocation from hot path
- **Task 4: Handler pattern completion** — Refactor `process_artisan_command()` to delegate to existing `ArtisanCommandHandler`

**Differentiators (Value-Adding Outcomes):**

- Unified periodic health check across SSR implementations
- Doctests and example binaries can use shared stubs
- Paves the way for `#![no_std]` builds
- Individual handlers testable in isolation; enables future middleware

**Defer to v2+:**

- New SSR hardware variants (beyond SsrControl/SsrControlSimple)
- Additional command handlers beyond ArtisanCommandHandler
- Advanced middleware (logging, rate limiting) — depends on Task 4 completion

### Architecture Approach

The v4.5 architecture builds directly on the v4.4 foundation:

```
Application Layer
├── RoasterControl (command routing, state management)
└── Control Layer
    ├── TemperatureHandler, SafetyHandler, ArtisanHandler, SystemHandler
    └── Heater trait (hardware abstraction)
└── Hardware Layer
    ├── SsrControlBase (shared SSR logic)
    ├── SsrControl / SsrControlSimple (concrete implementations)
    └── ArtisanFormatter (CSV output with bt_history)
```

**Key pattern:** The v4.4 extraction of `SsrControlBase` established delegation as the standard pattern. The v4.5 tasks extend this same pattern: method extraction to base types, not trait polymorphism.

### Critical Pitfalls

1. **Breaking embedded-hal trait bounds** — SSR deduplication must preserve `PIN: OutputPin<Error = ()>`, `DETECT: InputPin<Error = ()>` constraints. Define bounds before refactoring.

2. **Losing Send+Sync safety** — SSR types must remain Sendable for Embassy task boundaries. Avoid `RefCell`/`Cell` in refactored types; preserve existing `unsafe impl Send` pattern.

3. **Zero-duration deduplication race** — Commands arriving within same task tick create race windows. Use embassy `Instant` for temporal comparisons, not raw u32.

4. **heapless::Deque capacity overflow** — Unlike Vec, `Deque::push_back` is fallible. Use `.try_push_back().ok()` for graceful degradation when capacity reached.

5. **Handler state loss** — When splitting SSR control into handlers, deduplication state must persist. Keep deduplication state in dedicated component wrapping handler chain.

## Implications for Roadmap

Based on research, the v4.5 refactoring tasks can be organized into a single coherent release:

### Phase 1: Foundation & Memory Optimization
**Rationale:** Task 3 (heapless::Deque) is the lowest-risk, highest-impact change. It eliminates heap allocation from the artisan CSV formatting hot path, enabling no_std compatibility. This should be completed first as a confidence-builder.

**Delivers:**
- `Vec<f32>` → `Deque<f32, 5>` in ArtisanFormatter and MutableArtisanFormatter
- No heap allocations in BT history tracking
- O(1) amortized sliding window (was O(n) with Vec::remove(0))

**Addresses:** Task 3 from FEATURES.md

**Avoids:** Pitfall 17 — Deque capacity overflow (use .try_push_back().ok())

### Phase 2: SSR Deduplication
**Rationale:** Task 1 extends the v4.4 SsrControlBase pattern. Requires careful attention to trait bounds and safety logic preservation. Both SsrControl and SsrControlSimple must delegate to the base implementation.

**Delivers:**
- `detect_heat_source()` moved to SsrControlBase
- ~30 lines of duplicate code eliminated
- Both SSR types delegate to base implementation

**Addresses:** Task 1 from FEATURES.md

**Avoids:**
- Pitfall 1 — Define trait bounds before extraction
- Pitfall 4 — Preserve all three state transitions (Available, NotDetected, Error)
- Pitfall 5 — Keep PWM readback contract intact
- Pitfall 15 — Zero-duration deduplication race

### Phase 3: Test Infrastructure
**Rationale:** Task 2 moves test stubs to library-accessible location. Enables better testing across modules without duplicating mock implementations.

**Delivers:**
- Test stubs accessible via `crate::common::{StubHeater, StubFan, StubThermometer}`
- Shared mocks usable by both unit and integration tests

**Addresses:** Task 2 from FEATURES.md

**Avoids:**
- Pitfall 8 — Host-side test execution (use #[cfg(not(target_arch = "riscv32"))])
- Pitfall 19 — Test migration failure between cfg(test) and /tests/

### Phase 4: Command Handler Delegation
**Rationale:** Task 4 extends the existing handler pattern from v4.4. The handler chain already exists; this task verifies all commands go through it and removes direct match statements.

**Delivers:**
- `process_artisan_command()` delegates to ArtisanCommandHandler
- Large match statement removed from RoasterControl
- Handlers testable in isolation

**Addresses:** Task 4 from FEATURES.md

**Avoids:**
- Pitfall 16 — Handler state loss (keep deduplication state in dedicated component)
- Pitfall 18 — Handler Send+Sync break (verify compiles with cargo check --features std)

### Phase Ordering Rationale

- **Why Tasks 1-3 are independent:** No dependencies between Vec→Deque migration, detect_heat_source extraction, and test stub migration. Can be parallelized if resources allow.
- **Why Task 4 is last:** Depends on handler pattern already existing in v4.4; requires careful state management across handler chain.
- **Why P1 before P2:** Task 3 (memory optimization) has clear bounded scope and immediate benefit. Task 1 (SSR deduplication) requires more architectural caution.

### Research Flags

Phases likely needing deeper research during planning:

- **Phase 2 (SSR Deduplication):** Complex trait bounds interaction between SsrControl, SsrControlSimple, and Heater trait. May need to verify embedded-hal version compatibility.
- **Phase 4 (Handler Pattern):** Send+Sync verification requires compile-time checks; state management across handler chain needs careful design.

Phases with standard patterns (skip research-phase):

- **Phase 1 (heapless::Deque):** Well-documented heapless API; straightforward drop-in replacement.
- **Phase 3 (Test Stubs):** Structural change only; pattern already exists in tests/common/mod.rs.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All dependencies verified; heapless 0.9.2 confirmed as latest stable |
| Features | HIGH | Codebase verified; all four tasks have clear technical outcomes |
| Architecture | HIGH | Follows established v4.4 patterns; handler delegation already exists |
| Pitfalls | MEDIUM-HIGH | Comprehensive list (19 pitfalls); embedded context introduces unknowns |

**Overall confidence:** HIGH

### Gaps to Address

- **PWM readback contract:** Not explicitly tested in current test suite. Flag for integration test verification during Phase 2.
- **Error path coverage:** Existing mocks implement happy path only. Need FakeDetectPinError variants for error branch testing.
- **Multi-SSR variant testing:** Current tests cover SsrControl and SsrControlSimple individually; need integration test for concurrent operation.

## Sources

### Primary (HIGH confidence)

- LibreRoaster codebase analysis — All refactoring targets verified in source
  - `src/hardware/ssr.rs` — SsrControlBase, detect_heat_source() implementations (lines 87-246, 329-362)
  - `src/output/artisan.rs` — Vec<f32> bt_history usage (lines 25, 51-55)
  - `tests/common/mod.rs` — Existing test stub pattern (317 lines)
  - `src/control/handlers.rs` — ArtisanCommandHandler implementation (471 lines)
- heapless crate documentation (docs.rs/heapless/0.9.2) — Deque API confirmed
- Template: /home/juan/.config/opencode/get-shit-done/templates/research-project/SUMMARY.md

### Secondary (MEDIUM confidence)

- embedded-hal-mock crate documentation — For test infrastructure patterns
- Embassy async patterns documentation — For Send+Sync requirements

### Tertiary (LOW confidence)

- Community discussions on handler patterns in embedded Rust — Varied approaches, recommend validating during Phase 4

---

*Research completed: 2026-02-28*
*Ready for roadmap: yes*
