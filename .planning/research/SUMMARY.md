# Project Research Summary

**Project:** LibreRoaster  
**Domain:** Embedded Rust firmware for coffee roaster control (ESP32-C3)  
**Researched:** 2026-02-19  
**Confidence:** HIGH

---

## Executive Summary

LibreRoaster is an ESP32-C3-based coffee roaster firmware requiring critical safety fixes to eliminate Use-After-Free bugs, unsafe static initialization, and race conditions. Research confirms the recommended approach uses **StaticCell** patterns for safe static initialization and **embassy_sync::Mutex** with **CriticalSectionRawMutex** to replace the unsafe `take()/replace()` pattern that causes race windows during async operations. The existing embassy-rs async executor (0.9.1) and esp-hal (1.0.0) stack are validated—no new dependencies required. Key risks include blocking I/O in async contexts, LEDC timer conflicts between SSR (1Hz) and Fan (25kHz) PWM channels, and documentation drift. The architecture follows a handler chain pattern with safety handlers first, dual verification at control boundaries, and fail-safe defaults.

---

## Key Findings

### Recommended Stack

**Core technologies** (from STACK.md):
- **esp-hal 1.0.0** — LEDC, UART, USB CDC peripherals with async drivers; `unstable` feature unlocks LEDC timer access
- **embassy-rs 0.9.1** — Async executor; already present and validated
- **embedded-io-async 0.6.1** — Async byte-stream traits for UART/USB; matches embassy-usb dependencies
- **embassy-usb 0.5.1** — Async CDC ACM stack with lock-free endpoints; prevents USB from starving SSR/Fan tasks
- **fixed 1.30.0** — Deterministic saturating arithmetic for SSR duty clamp math; avoids floating-point errors
- **fugit 0.3.9** — Rate/Duration conversions matching esp-hal timer examples
- **heapless 0.8.0** — Static ring buffers (spsc::Queue) for non-blocking command buffering
- **static_cell 2.1.1** — Safe static initialization; already in Cargo.toml, use consistently
- **embassy-sync 0.6.1** — Async mutex (already present); provides `Mutex<CriticalSectionRawMutex, T>` for race-free async access

### Expected Features

**Required for v3.0 safety fixes:**
- Replace `critical_section::Mutex<RefCell<Option<T>>>` with `embassy_sync::Mutex<CriticalSectionRawMutex, T>`
- Remove `take()/replace()` pattern from `roaster_async_sensor_read()` — eliminates race window
- Fix unsafe `make_static()` function in main.rs — use StaticCell
- Fix ServiceContainer singleton pattern — eliminate `&'static mut` aliasing
- Fix UART driver lifetime transmute — use StaticCell or updated esp-hal API

**Must have (table stakes):**
- SSR duty clamping with deterministic math (fixed::Saturating)
- Async USB CDC + UART output (embassy-usb)
- Temperature safety checks with emergency shutdown
- SSR cycle guard (1000ms minimum per datasheet)

**Should have (competitive):**
- Fan speed verification after LEDC writes
- Hardware status reporting for graceful degradation

**Defer (v2+):**
- Advanced PID tuning algorithms
- Telemetry logging/history

### Architecture Approach

The system uses a **layered async architecture** with Embassy-driven control loops and distributed safety mechanisms:

- **Control Layer:** RoasterControl orchestrates handlers; safety handler is first in chain
- **Hardware Abstraction:** esp-hal drivers for LEDC, SPI (MAX31856), UART, USB
- **Concurrency:** ServiceContainer uses critical_section mutexes for atomic state; migrate to embassy_sync::Mutex for async

**Major components:**
1. **RoasterControl** — Central safety coordinator; validates commands, enforces limits, triggers shutdown
2. **SafetyCommandHandler** — First in handler chain; intercepts emergency stops
3. **SsrCycleGuard** — Enforces 1000ms SSR minimum cycle time; prevents command flooding
4. **ServiceContainer** — Shared state via mutexes; migrate to async-safe patterns
5. **dual_output_task** — Non-blocking USB/UART dispatch via DMA

### Critical Pitfalls

1. **StaticCell double initialization** — Calling `.init()` twice panics; define at module scope, check before init
2. **Blocking→async without `.await`** — Using `Timer::after_millis()` without await still blocks executor
3. **LEDC timer conflict** — SSR (1Hz) and Fan (25kHz) must use different timer numbers (Timer0 vs Timer1); channels share timers
4. **Lifetime transmute without ownership** — Unsafe `mem::transmute` creates dangling pointers; use StaticCell
5. **embassy_sync::Mutex with RefCell** — Async mutex provides exclusive access; RefCell adds runtime overhead and panics
6. **Wrong RawMutex for context** — Use `CriticalSectionRawMutex` for ISR + task sharing; `ThreadModeRawMutex` fails in interrupts

---

## Implications for Roadmap

### Phase 1: StaticCell Safety Fixes
**Rationale:** Fixes Use-After-Free bugs that cause intermittent crashes; foundational for all other work

**Delivers:**
- Replace unsafe `make_static()` in main.rs with StaticCell pattern
- Fix ServiceContainer singleton to return `&'static` not `&'static mut`
- Fix UART driver lifetime transmute issue

**Addresses:** Bug A (make_static), Bug D (mutable statics), Bug E (ServiceContainer)

**Avoids:** Pitfall 1 (double init), Pitfall 7 (incomplete unsafe replacement)

### Phase 2: Async Mutex Migration
**Rationale:** Eliminates race; required for reliableDelivers:**
- async operation

**::Mutex<Ref Replace `critical_section condition in sensor reading>` with `Mutex<RoasterControl>>Cell<OptionMutex, Option<CriticalSectionRaw`
- Remove take<RoasterControl>>/replace pattern from `roaster_async_sensor_read()`
- Update `with_roaster()` methods to async

**Addresses:** Race condition in roaster_async_sensor_read(), Feature: embassy_sync::Mutex migration

**Avoids:** Pitfall 9 (RefCell in async Mutex), Pittrait bound issues)

fall 10 (### Phase 3: Blocking I/O to Async Conversion
**Rationale:** MAX31856 sensor reads must not block executor; enables responsive control loops

**Delivers:**
- Convert blocking delay in MAX31856 read to async `Timer::after_millis().await`
- Verify USB CDC and UART output remain non-blocking
- Test control loop jitter under load

**Addresses:** Bug G (blocking MAX31856 read)

**Avoids:** Pitfall 3 (blocking without await), Pitfall 4 (fixing tests instead of code)

### Phase 4: LEDC Timer Separation + Documentation
**Rationale:** SSR and Fan must use independent timers; docs must match code

**Delivers:**
- Verify SSR uses Timer0, Fan uses Timer1 (or distinct timers)
- Update README/PROTOCOL.md to match actual command outputs
- Verify with integration tests

**Addresses:** Bug H (LEDC timer conflict), Bug F (documentation mismatch)

**Avoids:** Pitfall 6 (timer conflict), Pitfall 5 (docs without verification)

### Phase Ordering Rationale

- StaticCell fixes are foundational (no dependencies) → Phase 1
- Async mutex migration depends on understanding existing patterns → Phase 2
- Blocking→async conversion is isolated to sensor code → Phase 3
- LEDC timer + docs are validation/correctness → Phase 4 (can parallel)

### Research Flags

**Phases needing deeper research:**
- **Phase 2 (Async Mutex):** Verify all sync callers in interrupt context still work after migration; may need dual-mode API
- **Phase 3 (Blocking→Async):** MAX31856 driver may need API changes for true async; verify esp-hal SPI async support

**Phases with standard patterns (skip research):**
- **Phase 1:** StaticCell patterns are well-documented, already partially used in codebase
- **Phase 4:** LEDC timer config already validated in research; docs update is straightforward

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | esp-hal/embassy-rs stack validated; StaticCell already in Cargo.toml |
| Features | HIGH | Async mutex migration clearly documented; no new dependencies |
| Architecture | HIGH | Handler chain, dual verification, fail-safe patterns well-established |
| Pitfalls | HIGH | Comprehensive coverage of embedded-specific issues; Miri/Clippy detection strategies |

**Overall confidence:** HIGH

### Gaps to Address

- **UART driver transmute fix:** May require esp-hal API adjustment; test after StaticCell changes
- **Interrupt handler compatibility:** Verify sync `with_roaster()` callers work after async migration; may need `critical_section` path preserved
- **MAX31856 async conversion:** Need to verify esp-hal SPI async capabilities support true non-blocking reads

---

## Sources

### Primary (HIGH confidence)
- https://docs.rs/esp-hal/latest/esp_hal/ — LEDC, UART, USB drivers with async support
- https://docs.embassy.dev/embassy-sync/git/default/mutex/struct.Mutex.html — Async mutex API
- https://docs.rs/static_cell/2.1.1 — Static initialization patterns
- https://docs.rs/embassy-usb/latest/embassy_usb/ — Async USB CDC stack

### Secondary (MEDIUM confidence)
- https://blog.theembeddedrustacean.com/sharing-data-among-tasks-in-rust-embassy-synchronization-primitives — Tutorial on async sharing
- https://gist.github.com/benpeoples/3aa57bffc0f26ede6623ca520f26628c — ESP32 LEDC frequencies

### Tertiary (LOW confidence)
- Community forum discussions on RefCell + Mutex patterns (needs implementation verification)

---

*Research completed: 2026-02-19*
*Ready for roadmap: yes*
