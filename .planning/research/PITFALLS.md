# Pitfalls Research: SSR Refactoring and Test Infrastructure

**Domain:** Embedded Rust firmware (ESP32-C3 coffee roaster control)
**Milestone:** SSR refactoring for deduplication and shared test stubs
**Researched:** 2026-02-24
**Confidence:** MEDIUM-HIGH

## Overview

This document catalogs common pitfalls when:
1. Refactoring SSR (Solid State Relay) control code to eliminate duplication
2. Creating shared test infrastructure (mocks/stubs) for embedded testing

---

## Critical Pitfalls: SSR Refactoring

### Pitfall 1: Breaking embedded-hal Trait Bounds During Deduplication

**What goes wrong:** Extracting shared logic into a base type or trait implementation changes the generic constraints in ways that break existing code. The `SsrControl` and `SsrControlSimple` types currently implement the `Heater` trait from `crate::control::traits`. Refactoring may introduce new trait bounds or change existing ones, causing downstream code to fail to compile or worse, silently change behavior.

**Why it happens:** The original `SsrControl` stores a `PIN: OutputPin` while `SsrControlSimple` does not. During deduplication, developers may try to unify these by making the pin optional or using a trait, but this changes the `Heater` trait implementation requirements. The `embedded-hal` traits have specific error types (`OutputPin::Error`, `InputPin::Error`) that must be handled consistently.

**Warning signs:**
- Compilation errors mentioning "trait bound not satisfied" for `Heater` trait
- Runtime behavior changes where SSR still compiles but no longer responds to commands
- Error type mismatches between the extracted logic and the `Heater` trait's `RoasterError`

**Prevention:**
1. Define clear trait bounds before refactoring: `PIN: OutputPin<Error = ()>`, `DETECT: InputPin<Error = ()>`
2. Ensure the extracted base type maintains the same error handling contract
3. Test compilation of all dependent code (control loop, safety system) after each extraction step
4. Keep the `Heater` trait implementation separate from the internal logic—delegate to private methods

**Phase to address:** Phase 1 (SSR deduplication) — validate trait bounds before merging duplicate code

---

### Pitfall 2: Losing Send+Sync Safety in Extracted Types

**What goes wrong:** The current `SsrControlSimple` has an explicit `unsafe impl Send` because it wraps hardware peripherals. After refactoring, if the deduplicated code introduces interior mutability (RefCell, Cell, atomic operations) or removes the `Send` marker, the type becomes non-Sendable, breaking the async task boundaries in Embassy.

**Why it happens:** When extracting shared logic, developers may add internal state tracking, logging buffers, or retry counters using `RefCell` or `Cell` for interior mutability. While this compiles and may work on a single-threaded system, it breaks the `Send` guarantee that allows the SSR control to be passed between async tasks.

**Warning signs:**
- Compiler error: "cannot send shared by-value closure of type `&mut SsrControl` between tasks safely"
- Runtime panics about "borrow rules violated" when SSR is accessed from multiple tasks
- Existing `unsafe impl Send` blocks suddenly don't compile

**Prevention:**
1. Preserve the existing `unsafe impl Send` pattern for the extracted type
2. Avoid `RefCell`/`Cell` in the refactored SSR types—use plain fields with exclusive access
3. If state tracking is needed, use atomics or dedicated mutex types, not interior mutability
4. After refactoring, verify the type can be stored in an Embassy task without modification

**Phase to address:** Phase 1 (SSR deduplication) — verify Send+Sync after each extraction

---

### Pitfall 3: Over-Abstraction: Trait Explosion

**What goes wrong:** Instead of simple code deduplication, developers create elaborate trait hierarchies (e.g., `SsrBackend`, `SsrPwmControl`, `SsrDetection`). This makes the code harder to read, increases compile times, and makes debugging harder because the call stack is now spread across multiple trait implementations.

**Why it happens:** The drive to eliminate duplication leads to creating abstractions for every shared method. With only two SSR implementations (`SsrControl` and `SsrControlSimple`), the abstraction adds more complexity than it removes.

**Warning signs:**
- New files: `ssr_traits.rs`, `ssr_backend.rs`, `ssr_pwm.rs`
- Trait bounds become longer than the function signatures
- Adding a new SSR variant requires implementing 5+ traits

**Prevention:**
1. Apply the "three strikes" rule: only extract to a trait after the third identical implementation
2. Start with simple delegation to private methods, not trait objects
3. Keep the refactoring scope limited: extract duplicate methods, don't create a framework
4. Prefer composition over trait polymorphism for this use case

**Phase to address:** Phase 1 (SSR deduplication) — prefer method extraction over trait abstraction

---

### Pitfall 4: Forgetting Safety-Critical Detection Logic in Extracted Code

**What goes wrong:** The `detect_heat_source` method contains safety-critical logic that transitions the SSR state to `Error` when the detection pin reports an error. If this logic is moved to a shared location but loses the error handling or state transition, the roaster could continue heating without proper detection, creating a fire hazard.

**Why it happens:** The detection pin logic is tightly coupled with state management. When extracting to a shared location, developers may simplify the error handling or miss the edge case where `InputPin::is_low()` returns `Err(_)`, which triggers the safety-critical `Error` state transition.

**Warning signs:**
- Safety system tests fail to detect missing heat source
- Error state is not reported when detection pin is in error
- Logs no longer show "SSR detection pin error" messages

**Prevention:**
1. Create a checklist of all state transitions before refactoring: `Available`, `NotDetected`, `Error`
2. Ensure the extracted code preserves all three branches: `Ok(true)`, `Ok(false)`, `Err(_)`
3. Add safety-specific tests that verify error state transitions
4. Keep the safety state machine visible in the main type, even if methods are extracted

**Phase to address:** Phase 1 (SSR deduplication) — validate safety state machine after extraction

---

### Pitfall 5: Breaking the PWM Readback Contract

**What goes wrong:** The existing code calls `monitor_ledc_after_set` after every `set_duty` to verify the PWM value was actually applied. This retry logic is essential for safety. If deduplication moves this to a shared location but the readback is skipped or the error handling changes, PWM drift goes undetected.

**Why it happens:** The readback function uses the `LedcDutyReader` trait. When extracting to a shared location, the trait bounds may become less specific, or the mock implementations in tests may not implement readback correctly.

**Warning signs:**
- Tests pass but hardware shows PWM drift in production
- Duty tolerance tolerance check (`SSR_DUTY_TOLERANCE_TICKS`) is bypassed
- No retry attempts even when PWM doesn't match commanded value

**Prevention:**
1. Preserve the readback call in the public API after deduplication
2. Ensure mock implementations in tests provide readback responses
3. Add integration tests that verify the retry mechanism activates
4. Keep the tolerance constant visible and assertable in tests

**Phase to address:** Phase 1 (SSR deduplication) — test PWM readback contract after extraction

---

## Critical Pitfalls: Test Infrastructure

### Pitfall 6: Mock/Stub API Drift from Real Implementation

**What goes wrong:** The existing `MockUartDriver` in tests differs from the real `UartDriver` implementation in subtle ways: error handling, buffer behavior, or timing assumptions. Tests pass but fail when run against real hardware.

**Why it happens:** Mocks are often written quickly to make tests compile. They implement only the happy path, using simplified error types (`Infallible`) or ignoring edge cases. Over time, the mock diverges from the real implementation.

**Warning signs:**
- Integration tests pass but unit tests fail on hardware
- Mock has `fn read_bytes()` but real driver uses `fn read()`
- Error types don't match between mock and real implementation

**Prevention:**
1. Use the `embedded-hal-mock` crate which provides mock implementations for standard traits
2. Create a test utility that can switch between mock and real implementations at compile time
3. Document which behaviors the mock approximates and which it simplifies
4. Run integration tests with the mock AND with real hardware before releases

**Phase to address:** Phase 2 (Test infrastructure) — align mock API with real implementation

---

### Pitfall 7: Tests That Never Fail (Over-Mocked Behavior)

**What goes wrong:** The mock returns predictable values that never trigger error paths. For example, `FakeDetectPin` in `ssr_monitor.rs` always returns `Ok(true)` for `is_low()`, so the "detection pin error" code path (`Err(_)`) is never tested.

**Why it happens:** Developers create mocks for the happy path to make tests pass. The error handling code becomes dead code in tests, and regressions in error handling go undetected.

**Warning signs:**
- Tests have 100% pass rate regardless of code changes in error paths
- No test exercises the `Err(_)` branch of `InputPin::is_low()`
- Code coverage tools show error paths as uncovered

**Prevention:**
1. Create variant mocks for error conditions: `FakeDetectPinError`, `FakeLedcChannelError`
2. Add test cases that specifically trigger each error variant
3. Use parameterized tests to verify behavior across success and failure modes
4. Track error path coverage separately from happy path coverage

**Phase to address:** Phase 2 (Test infrastructure) — add error path test coverage

---

### Pitfall 8: Host-Side Tests Can't Run Due to no_std Dependencies

**What goes wrong:** SSR code uses `esp_hal` types (LEDC, GPIO) which don't exist on the host. Tests that try to run `cargo test` on the host fail with "cannot find type `ChannelIFace`" or similar errors.

**Why it happens:** The embedded-hal traits are used, but the concrete types are ESP-specific. Moving from integration tests (which run on hardware via `probe-rs`) to host-side unit tests requires trait-based abstraction.

**Warning signs:**
- `cargo test` fails on host with "target riscv32 required" or missing type errors
- Tests are marked `#[cfg(target_arch = "riscv32")]` but don't compile on target either
- Cannot run tests in CI without hardware

**Prevention:**
1. Use the `#[cfg(not(target_arch = "riscv32"))]` pattern for host-side tests
2. Create host-side stub types that implement the same traits
3. Use `embedded-hal-mock` for tests that need to run on host
4. Separate the "pure logic" (percentage conversion, state machines) from hardware-specific code so it can be tested on host

**Phase to address:** Phase 2 (Test infrastructure) — enable host-side test execution

---

### Pitfall 9: Shared Test Fixtures That Share Mutable State

**What goes wrong:** Multiple tests use a shared mock instance that retains state between tests. Test B passes because it inherits state set by Test A, masking bugs. This is the classic "test pollution" problem.

**Why it happens:** In `ssr_monitor.rs`, `FakeLedcChannel` uses `RefCell` to store responses. If tests don't create fresh instances, they share state. This is especially problematic in integration tests that run in sequence.

**Warning signs:**
- Test order affects pass/fail results
- Running a single test passes, but running all tests fails
- Mutating shared fixtures between tests without reset

**Prevention:**
1. Each test should create its own mock instance
2. Use Rust's test framework isolation (tests run in separate binaries by default)
3. Add `teardown` or `reset` methods to mocks
4. Document fixture lifetime expectations in test comments

**Phase to address:** Phase 2 (Test infrastructure) — ensure test isolation

---

### Pitfall 10: Missing Test Coverage for the Heater Trait Implementation

**What goes wrong:** The `Heater` trait is implemented for `SsrControlSimple` and `SsrControl`, but there's no dedicated test that verifies the trait implementation works correctly. Changes to the trait or implementation could break the control loop without detection.

**Why it happens:** The trait implementation delegates to internal methods. Developers test the internal methods but not the trait boundary. The control loop uses `Heater` generically, so bugs at this boundary are hard to catch.

**Warning signs:**
- No tests directly call `ssr.set_power()` or `ssr.get_status()`
- Control loop tests mock the entire Heater rather than testing the real implementation
- Adding a new `Heater` implementer has no test template

**Prevention:**
1. Add integration tests that exercise the `Heater` trait implementation directly
2. Create a test module `tests/heater_trait_tests.rs` that tests both SSR types through the trait
3. Verify that `set_power` maps correctly to `set_percentage` and errors propagate as `RoasterError`
4. Add test for each status variant: `Available`, `NotDetected`, `Error`

**Phase to address:** Phase 2 (Test infrastructure) — add trait boundary tests

---

## Integration Pitfalls: SSR + Test Infrastructure

### Pitfall 11: Refactoring SSR Then Breaking All Existing Mocks

**What goes wrong:** After deduplicating SSR code, the internal types change. The mock implementations (`FakeDetectPin`, `FakeLedcChannel`) no longer implement the required traits or have the right method signatures. All existing tests fail.

**Why it happens:** The mocks were written for the specific struct field names and method signatures. When the internal implementation changes, the mocks are orphaned.

**Warning signs:**
- Compilation errors in `ssr_monitor.rs` after SSR refactoring
- Mock no longer implements required trait
- Field access errors on private fields

**Prevention:**
1. Keep mocks in a separate crate or module that re-exports the tested types
2. Update mocks as part of the refactoring PR
3. Use trait-based mocks (`impl OutputPin`) rather than concrete types where possible
4. Run all SSR tests as part of the refactoring CI pipeline

**Phase to address:** Phase 1 + Phase 2 (Concurrent) — update mocks during SSR refactoring

---

### Pitfall 12: Test Infrastructure Created Without Consideration for Future Hardware Variants

**What goes wrong:** Mocks are specific to ESP32-C3 LEDC channels. When adding support for a different PWM hardware (e.g., MCPWM), the mocks can't be reused and tests must be rewritten.

**Why it happens:** The mock directly implements `esp_hal::ledc::channel::ChannelIFace`. Supporting a new hardware platform requires creating new mocks.

**Prevention:**
1. Create mocks at the trait level (`embedded_hal::Pwm`) rather than HAL-specific levels
2. Abstract the PWM behavior in a project-specific trait that multiple HALs can implement
3. Document the mock interface contract so new hardware support knows what to implement
4. Consider using `embedded-hal-mock` which provides platform-agnostic mocks

**Phase to address:** Phase 2 (Test infrastructure) — design mocks for portability

---

---

## Critical Pitfalls: v4.5-Specific Refactoring

### Pitfall 15: Zero-Duration SSR Command Deduplication Creates Race Windows

**What goes wrong:**
When deduplicating SSR commands with identical values arriving in rapid succession, if the time window is zero (or too small), the second command arrives before the first has actually been applied to hardware. This creates a race where the "duplicate" detection is based on stale state.

**Why it happens:**
The deduplication logic compares `commanded_value == last_commanded_value`. If commands arrive within the same execution context (same task tick), there's no temporal separation. The fix of "only deduplicate if time delta > X" is often applied incorrectly—the guard gets added but uses the wrong time source (e.g., wall-clock instead of monotonic, or forgets to account for timer wraparound).

**How to avoid:**
- Use embassy time `Instant` for all temporal comparisons, not raw u32 timestamps
- Ensure the deduplication window is larger than the maximum SSR PWM update latency (typically 10-50ms for LEDC)
- Add integration tests that verify rapid successive commands (same value, <1ms apart) are correctly deduplicated while different values are not

**Warning signs:**
- SSR output jitters when Artisan sends repeated SET commands
- Test `ssr_scheduler` shows 100% duplicate rejection rate
- Adding debug logging causes the bug to disappear (timing dependency)

**Phase to address:** v4.5 SSR deduplication implementation

---

### Pitfall 16: Deduplication State Lost When Handler Pattern Splits Responsibility

**What goes wrong:**
When splitting SSR control into separate handler components (e.g., `TemperatureCommandHandler`, `ArtisanCommandHandler`), the deduplication state (last command value, last command time) gets duplicated or lost. Each handler maintains its own "last value" but the scheduler has another, causing contradictory deduplication decisions.

**Why it happens:**
The `RoasterCommandHandler` trait operates on `RoasterCommand` enums, but SSR deduplication requires state that persists across commands. If handlers are restructured without preserving this state, each new handler instance starts with fresh state.

**How to avoid:**
- Keep deduplication state in a dedicated component (e.g., `SsrCommandDeduplicator`) that wraps the handler chain
- Ensure the deduplicator is constructed once and lives across handler instantiations
- Verify with integration tests: send same command 3x rapidly, verify hardware receives only 1 actual write

**Warning signs:**
- `TemperatureCommandHandler` and `ArtisanCommandHandler` both have duplicate SSR setting logic
- Compiler warns about dead code after refactoring (old deduplication code wasn't removed)
- Integration test failures that only appear under load

**Phase to address:** v4.5 handler pattern refactoring

---

### Pitfall 17: heapless::Deque Capacity Overflow When Used as Drop-In Replacement

**What goes wrong:**
Replacing `Vec<f32>` with `heapless::Deque<f32, N>` causes runtime panics when the Deque fills up. Unlike `Vec` which reallocates, `Deque::push_back` returns `Err` (or panics if using the non-fallible API). Code that worked with unbounded `Vec` fails silently or crashes with `capacity error`.

**Why it happens:**
The heapless crate API diverges from std `Vec`. The `push` method is fallible (`Result<(), CapacityError>`) but many developers use `.unwrap()` or forget to handle the error.

**How to avoid:**
- Use `.try_push_back(value).ok()` for graceful degradation when capacity is reached
- Ensure the fixed capacity `N` is sized for the worst-case scenario (e.g., 5-element sliding window)
- Add integration tests that push past capacity to verify behavior
- Prefer the fallible API explicitly: `deque.push_back(value).map_err(|_| Error::QueueFull)`

**Warning signs:**
- Code uses `deque.push_back()` without error handling
- Original `Vec` had `.push()` that was assumed to always succeed
- `Cargo.toml` pins an old heapless version with panicking behavior

**Phase to address:** v4.5 heapless::Deque migration

---

### Pitfall 18: Handler Pattern Refactor Breaks Send + Sync Bounds

**What goes wrong:**
After refactoring to use the handler pattern (separate `TemperatureCommandHandler`, `ArtisanCommandHandler`, etc.), the code fails to compile with "type cannot be sent between threads" errors. This happens because handler state includes references to non-`Send` types (e.g., `embedded_hal` peripherals, mutex guards held across await points).

**Why it happens:**
The Embassy executor requires all types passed between tasks to be `Send`. If a handler holds a reference to SSR control (which wraps LEDC channels), and that handler is accessed from multiple tasks, the borrow checker and `Send` bound become problematic.

**How to avoid:**
- Use `heapless::spsc::Channel` or `embassy_sync::signal::Signal` for inter-task communication
- Ensure handlers are constructed in the `main` task and ownership is clear
- Add `unsafe impl Send for MyHandler` only after verifying the inner state is actually safe (document why!)
- Run `cargo check --features "std"` to catch `Send` issues early

**Warning signs:**
- Adding a new handler causes unrelated code to fail `Send` check
- Compiler error mentions "cannot send a shared reference across tasks"
- Code previously compiled with `Send` but handler refactor broke it

**Phase to address:** v4.5 handler pattern refactoring

---

### Pitfall 19: Test Stub Migration Breaks Between cfg(test) and /tests/

**What goes wrong:**
Unit tests in `#[cfg(test)]` modules use `crate::` paths and mock hardware via trait objects. When migrating to `/tests/` integration files, tests fail because:
- The `crate::` paths become `libreroaster::`
- Mock implementations that worked in-unit become `use` conflicts
- `embedded_hal` traits differ between `std` and `no_std` contexts

**Why it happens:**
The embedded Rust testing ecosystem has a split personality: `#[cfg(test)]` compiles against `std` (via `test` feature), while `/tests/` files require `no_std` compatible code or explicit feature gating.

**How to avoid:**
- Define mock types in a dedicated `tests/common/mod.rs` module
- Use `#[cfg(all(test, feature = "std"))]` for std-specific test code
- Keep hardware mocks as trait implementations that work in both contexts
- Document the expected feature flags for each test file

**Warning signs:**
- `error[E0433]: failed to resolve: use of undeclared type` in test files
- Mock implementations duplicate production code's trait bounds
- Tests pass in `cargo test` but fail in `cargo test --features std`

**Phase to address:** v4.5 test stub migration

---

## Minor Pitfalls

### Pitfall 13: Documentation Disconnect

**What goes wrong:** After refactoring, the code comments still refer to the old structure ("this method was duplicated in both SsrControl and SsrControlSimple"). This confuses future developers.

**Prevention:** Update or remove outdated comments as part of the refactoring PR.

### Pitfall 14: Test Naming Inconsistency

**What goes wrong:** Existing tests use inconsistent naming: `ssr_monitor`, `mock_uart`, `ssr_scheduler`. New tests don't follow the pattern.

**Prevention:** Follow the existing test naming convention in the project.

---

## Embedded-Specific Gotchas

| Issue | Common Mistake | Correct Approach |
|-------|----------------|------------------|
| Time handling | Using `u32` timestamps directly | Use `embassy_time::Instant` and `Duration` |
| Static lifetimes | Overusing `static mut` | Prefer `static` with `Mutex<CriticalSectionRawMutex, T>` |
| Hardware mocks | Implementing traits manually | Use `embedded-hal-mock` crate for common peripherals |
| Async exclusion | Blocking in async context | Use blocking variants or redesign to async |
| Stack overflow | Large stack allocations in tasks | Keep stack usage minimal, use `Box` for heap |

---

## Pitfall Summary Table

| Pitfall | Severity | Phase to Address | Detection Method |
|---------|----------|------------------|-------------------|
| Breaking trait bounds | CRITICAL | Phase 1 | Compilation errors |
| Losing Send+Sync | CRITICAL | Phase 1 | Compiler errors |
| Trait explosion | MODERATE | Phase 1 | Code review |
| Safety logic loss | CRITICAL | Phase 1 | Safety tests |
| PWM readback break | CRITICAL | Phase 1 | Integration tests |
| Zero-duration deduplication race | CRITICAL | v4.5 | Rapid-fire command tests |
| Handler state loss | CRITICAL | v4.5 | Multi-command state tests |
| heapless::Deque overflow | MODERATE | v4.5 | Push-past-capacity tests |
| Handler Send+Sync break | CRITICAL | v4.5 | cargo check errors |
| Test migration failure | MODERATE | v4.5 | Feature flag tests |
| Mock API drift | MODERATE | Phase 2 | Integration tests |
| No error path tests | MODERATE | Phase 2 | Coverage tools |
| no_std test failure | MODERATE | Phase 2 | CI failure |
| Test state pollution | MODERATE | Phase 2 | Flaky tests |
| Heater trait gap | MODERATE | Phase 2 | Missing coverage |
| Mock breakage | MODERATE | Phase 1+2 | Compilation |
| Non-portable mocks | LOW | Phase 2 | Design review |

---

## "Looks Done But Isn't" Checklist

- [ ] **SSR deduplication:** Verified with rapid-fire commands (<1ms apart) — not just single command tests
- [ ] **heapless::Deque:** Tested with overflow scenario (pushing past capacity N) — not just happy path
- [ ] **Handler state:** Verified state persists across multiple command invocations — not just initialization
- [ ] **Test migration:** Tests run with both `std` and `no_std` configurations — not just one feature flag
- [ ] **Send+Sync:** Code compiles with `cargo check --features "std"` — not just `cargo build`

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Zero-duration deduplication race | MEDIUM | Add temporal guard with `Instant::elapsed()`, re-run integration test |
| Handler state loss | HIGH | Re-architect to centralize deduplication state, may need API change |
| heapless::Deque overflow | LOW | Add `.try_push_back().ok()` handling, increase N if needed |
| Handler Send+Sync break | HIGH | Refactor inter-task communication, may need channel-based redesign |
| Test migration failures | LOW-MEDIUM | Add feature gates, simplify mock hierarchy, keep mocks in `tests/common` |

---

## Sources

- LibreRoaster codebase analysis — HIGH confidence
- embedded-hal-mock crate documentation — HIGH confidence
- Rust trait bounds and Send+Sync requirements — HIGH confidence
- Common embedded testing patterns from Ferrous Systems blog — MEDIUM confidence
- Code deduplication best practices (Manning Idiomatic Rust) — MEDIUM confidence
- Embassy async patterns documentation — HIGH confidence
- heapless crate v0.9.x API documentation — HIGH confidence
- Embedded Rust community discussions on handler patterns — MEDIUM confidence

---

## Performance Traps for Embedded

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Stack growth in async tasks | Watchdog resets, random crashes | Verify stack size with `embassy-executor` config | At >2KB stack per task |
| Allocating in ISR context | System hangs | Never heap-allocate in interrupt handlers | During hardware interrupts |
| Blocking on channel full | Command processing stalls | Use bounded channels with backpressure | Under high command rate |
| Excessive debug logging | Timing violations | Disable logs in release or use `defmt` | Real-time temperature control |
| Vec heap allocation | Memory fragmentation | Use heapless collections | On memory-constrained devices |

---

## Pitfall-to-Phase Mapping: v4.5

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Zero-duration SSR deduplication | SSR deduplication implementation | Rapid-fire command integration test |
| Handler state loss | Handler pattern refactor | Multi-command state persistence test |
| heapless::Deque overflow | Deque migration | Push-past-capacity unit test |
| Handler Send+Sync break | Handler pattern design | `cargo check --features std` clean compile |
| Test migration failures | Test infrastructure cleanup | Run tests with `--features std` and without |
| Breaking trait bounds | SSR deduplication | Test downstream dependent code |
| Losing Send+Sync | SSR deduplication | Verify type can be stored in Embassy task |

---

*Research for: SSR refactoring and test infrastructure milestone*
*Updated: 2026-02-28*
