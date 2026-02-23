# Project Research Summary

**Project:** LibreRoaster
**Domain:** Embedded Rust / ESP32-C3 Firmware Safety Fixes & Async Synchronization
**Researched:** 2026-02-20
**Confidence:** HIGH

## Executive Summary

LibreRoaster is an embedded Rust application running on the ESP32-C3, focusing on hardware reliability (SSR duty clamps, LEDC fan control, responsive UART/USB). The research identifies critical safety vulnerabilities in the current codebase, primarily stemming from unsafe static initialization (`make_static` causing Use-After-Free), race conditions in async sensor reads (the `take/replace` pattern), and blocking I/O starving the async executor.

The recommended approach is a comprehensive refactor utilizing established, safe patterns provided by the existing stack. This includes replacing unsafe manual statics with `static_cell::StaticCell`, migrating from `critical_section::Mutex` to `embassy_sync::Mutex<CriticalSectionRawMutex, T>` for async-safe mutual exclusion without the `take/replace` vulnerability, and ensuring all blocking delays and hardware abstractions properly utilize async `.await` semantics.

Key risks involve improper initialization of `StaticCell` (which panics if initialized twice), incorrectly bridging synchronous interrupt contexts with asynchronous tasks, and failing to properly isolate hardware peripherals like LEDC timers. Mitigating these requires strict adherence to initialization patterns at the module scope and thorough validation against ESP32-C3 hardware constraints.

## Key Findings

### Recommended Stack

The project already possesses the necessary dependencies to implement these fixes without introducing new crates. The focus is on correctly applying the tools provided by `esp-hal` (~1.0) and `embassy-rs` (0.9.1).

**Core technologies:**
- `embassy_sync::Mutex`: Async-safe mutual exclusion — allows holding logical locks across `.await` points without blocking the executor.
- `static_cell` (2.1.1): Safe static initialization — completely replaces dangerous `make_static` transmutes and `static mut` singletons with safe, one-time initialization.
- `esp-hal` + `embassy-usb` + `embedded-io-async`: Non-blocking peripheral access — ensures USB CDC and UART communications do not starve the executor.

### Expected Features

**Must have (table stakes):**
- Safe static initialization using `StaticCell` for SSR and Fan controllers.
- Elimination of the race condition window in `roaster_async_sensor_read()` via `embassy_sync::Mutex`.
- Non-blocking UART/USB I/O implementation preventing executor starvation.

**Should have (competitive):**
- Updated, accurate documentation reflecting the new async architecture and build instructions.
- Proper hardware separation for LEDC timers.

**Defer (v2+):**
- Introduction of external static site generators for documentation (stick to pure Markdown).

### Architecture Approach

The architecture relies heavily on the Embassy async executor driving ESP32-C3 peripherals via esp-hal. The critical shift is moving from unsafe/synchronous state management to robust async-first patterns.

**Major components:**
1. **ServiceContainer:** Dependency injection container — updated to use `embassy_sync::Mutex` for async-safe shared state.
2. **Hardware Drivers (UART, USB, LEDC):** Peripheral interfaces — leveraging `embedded-io-async` to keep operations non-blocking.
3. **Documentation Architecture:** Codebase documentation — single source of truth in `README.md` and `internalDoc/`.

### Critical Pitfalls

1. **StaticCell Double Initialization** — Avoid by ensuring `.init()` is only called once per cell.
2. **Race Conditions in Async Gaps** — Avoid using `take()` and `replace()` around `.await` points; use `embassy_sync::Mutex`.
3. **Blocking in Async Contexts** — Avoid using synchronous delays; always use `embassy_time::Timer` and `.await`.

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Static Safety Refactoring
**Rationale:** Eliminates the most severe undefined behavior (Use-After-Free) and establishes a safe foundation.
**Delivers:** Replacement of `make_static` and `static mut` with `StaticCell` across `main.rs`, `service_container.rs`, and UART drivers.
**Addresses:** Safe static initialization.
**Avoids:** StaticCell double initialization and function-scope definitions.

### Phase 2: Async Mutex Migration
**Rationale:** Resolves the critical race condition in sensor reading logic, which is the core operational loop.
**Delivers:** Implementation of `embassy_sync::Mutex<CriticalSectionRawMutex, T>` in `ServiceContainer`, removing the `take/replace` pattern.
**Uses:** `embassy_sync` 0.6.1.
**Implements:** ServiceContainer dependency management.

### Phase 3: Hardware & I/O Validation
**Rationale:** Ensures peripheral interactions (LEDC, UART, USB) are truly non-blocking and conflict-free.
**Delivers:** Correct LEDC timer separation and async I/O verification.

### Phase 4: Documentation Alignment
**Rationale:** Must follow code changes to ensure accuracy; updates `README.md` and internal docs.
**Delivers:** Updated documentation architecture.

### Phase Ordering Rationale

- Memory safety (Phase 1) is the prerequisite for all other operations.
- Logical concurrency fixes (Phase 2) depend on a stable static foundation.
- Hardware verification (Phase 3) tests the fully integrated async system.
- Documentation (Phase 4) finalizes the milestone.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 3:** UART driver transmute fix. The underlying issue with `esp-hal` lifetime requirements (`UartTx<'static>`) may require investigating specific API changes.

Phases with standard patterns (skip research-phase):
- **Phase 1 & 2:** `StaticCell` and `embassy_sync::Mutex` patterns are highly standardized.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Validated against existing `Cargo.toml` dependencies. |
| Features | HIGH | The async mutex pattern directly addresses the identified race condition. |
| Architecture | HIGH | Matches standard Embassy/Embedded Rust architectural models. |
| Pitfalls | HIGH | Specific, known issues with embedded Rust mapped directly to the codebase. |

**Overall confidence:** HIGH

### Gaps to Address

- **UART Driver Lifetime:** The exact API method in `esp-hal` ~1.0 to safely instantiate a `'static` UART driver without `mem::transmute` requires code-level exploration.

## Sources

### Primary (HIGH confidence)
- `embassy_sync::Mutex` official docs — Async-safe mutual exclusion.
- `static_cell` (2.1.1) crate docs — Safe static initialization patterns.
- `esp-hal` LEDC and UART documentation.

### Secondary (MEDIUM confidence)
- The Embedded Rustacean Blog — Sharing data among tasks in Embassy.

---
*Research completed: 2026-02-20*
*Ready for roadmap: yes*