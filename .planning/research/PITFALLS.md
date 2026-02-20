# Embedded Safety Fix Pitfalls: ESP32-C3 v3.0

**Domain:** Embedded Rust / ESP32-C3 Firmware Safety Fixes  
**Project:** LibreRoaster v3.0 Critical Safety Fixes  
**Researched:** 2026-02-18  
**Confidence:** HIGH

## Overview

This document catalogs common mistakes when fixing critical safety issues (Use-After-Free, unsafe statics, test failures, documentation mismatches, blocking I/O) in ESP32-C3 firmware using embassy-rs and esp-hal. Each pitfall includes prevention strategies specific to the bug types in this milestone.

---

## Critical Pitfalls

### Pitfall 1: StaticCell Double Initialization

**What goes wrong:** Calling `.init()` on a StaticCell that has already been initialized causes a panic at runtime.

**Why it happens:** StaticCell can only be initialized once. Code paths that trigger initialization multiple times (e.g., re-initialization on error, conditional initialization) will panic.

**Consequences:** Firmware crashes on startup or after error recovery attempts.

**Prevention:**
```rust
// WRONG - will panic on second call
static SSR: StaticCell<SsrControl> = StaticCell::new();
SSR.init(ssr1);  // First call - OK
SSR.init(ssr2);  // PANIC!

// CORRECT - check before init or use different pattern
static SSR: StaticCell<SsrControl> = StaticCell::new();
pub fn get_ssr() -> &'static mut SsrControl {
    // Only initializes once; subsequent calls panic
    // Use critical_section::Mutex if re-initialization needed
    SSR.init(SsrControl::new())
}
```

**Detection:** Runtime panic with message about "already initialized"

**Bug context:** Bug A (make_static), Bug D (mutable statics), Bug E (ServiceContainer)

---

### Pitfall 2: StaticCell Inside Function Scope

**What goes wrong:** Defining a StaticCell inside a function instead of as a static variable defeats the purpose of StaticCell.

**Why it happens:** Developers confuse `static_cell::make_static!` macro (which works inside functions) with `StaticCell::new()` (which requires static storage).

```rust
// WRONG
fn init_driver() {
    static DRIVER: StaticCell<Driver> = StaticCell::new();  // Won't compile in const context
    // ...
}

// CORRECT - must be at module scope
static DRIVER: StaticCell<Driver> = StaticCell::new();

fn init_driver() {
    let drv = Driver::new();
    DRIVER.init(drv);
}
```

**Consequences:** Compilation errors, or if using `make_static!` macro incorrectly, the data isn't actually static.

**Prevention:** Use the `make_static!` macro when inside function scope, or define StaticCell at module scope.

**Bug context:** Bug A, Bug D

---

### Pitfall 3: Converting Blocking to Async Incorrectly

**What goes wrong:** Converting blocking delay to async but not actually awaiting it, or using blocking delay in async context.

**Why it happens:** Simply renaming `delay_ms()` to `Timer::after_millis()` doesn't make it async.

```rust
// WRONG - still blocks!
async fn read_temp() {
    Timer::after_millis(160);  // Missing .await!
    // ...
}

// WRONG - blocks the executor
async fn read_temp() {
    embassy_time::block_for(embassy_time::Duration::from_millis(160));  // Blocks entire executor
    // ...
}

// CORRECT
async fn read_temp() {
    Timer::after_millis(160).await;  // Yields to other tasks
    // ...
}
```

**Consequences:** Other async tasks starve; entire async executor blocks.

**Prevention:** Always `.await` async delays; use `Timer::after_millis()` not blocking delays in async contexts.

**Bug context:** Bug G (blocking MAX31856 read)

---

### Pitfall 4: Fixing Test Assertions Instead of Implementation

**What goes wrong:** Changing test expectations to match broken behavior instead of fixing the actual bug.

**Why it happens:** Test failure seems easier to fix by changing the assertion than understanding the root cause.

```rust
// WRONG - test now passes but bug remains
#[test]
fn test_partial_ot2() {
    let result = parse_command("OT2");
    // Changed from: assert!(matches!(result, Err(ParseError::InvalidValue)));
    assert!(matches!(result, Ok(SetFanSpeed(0))));  // WRONG!
}

// CORRECT - fix the parser implementation
// The test expectation is correct; fix the parser instead
["OT2" | "ot2"] => Err(ParseError::InvalidValue),  // Require value
["OT2" | "ot2", value_str] => { /* parse value */ }
```

**Consequences:** Bug remains unfixed; downstream code receives incorrect behavior.

**Prevention:** Treat test failures as bug indicators; always fix implementation, not tests (unless test is wrong about expected behavior).

**Bug context:** Bug C (test_parse_ot2_partial_command)

---

### Pitfall 5: Documentation Updates Without Code Verification

**What goes wrong:** Updating documentation to match code without verifying code behavior.

**Why it happens:** Documentation drift goes both ways - either docs are wrong or code changed without docs.

```rust
// WRONG - just copy from code without verification
// README.md updated to say: "READ returns ET, BT, HEATER, FAN"
// But actual implementation returns 7 values

// CORRECT - verify actual behavior first
// 1. Run actual READ command, observe output
// 2. Check formatter code for exact output format
// 3. THEN update docs to match reality
```

**Consequences:** Users rely on incorrect documentation; integration failures.

**Prevention:** Always verify actual behavior before updating docs; run commands and inspect outputs.

**Bug context:** Bug F (README vs PROTOCOL.md mismatch)

---

### Pitfall 6: LEDC Timer Not Actually Separated

**What goes wrong:** Creating separate timer variables but they still reference the same hardware timer.

**Why it happens:** ESP32-C3 has 4 LEDC timers; channels 0-5 share them. Creating a variable doesn't create a new hardware timer.

```rust
// WRONG - variables are separate but hardware timer is shared
let timer0 = ledc.timer(Number::Timer0);
let timer1 = ledc.timer(Number::Timer0);  // Same timer!

// WRONG - both channels end up on same timer
channel0.configure(ChannelConfig { timer: &mut timer0, ... })?;
channel1.configure(ChannelConfig { timer: &mut timer0, ... })?;  // Same timer!

// CORRECT - use different timer numbers
let mut fan_timer = ledc.timer(Number::Timer0);
let mut ssr_timer = ledc.timer(Number::Timer1);  // Different timer!

fan_channel.configure(ChannelConfig { timer: &mut fan_timer, ... })?;
ssr_channel.configure(ChannelConfig { timer: &mut ssr_timer, ... })?;
```

**Consequences:** SSR (~1Hz) and Fan (25kHz) interfere; PWM behaves unexpectedly.

**Prevention:** Explicitly use different timer numbers (Timer0, Timer1, etc.); verify hardware timer allocation.

**Bug context:** Bug H (SSR and Fan share same LEDC timer)

---

### Pitfall 7: Unsafe Static Mut Replacement Incomplete

**What goes wrong:** Replacing `static mut` with StaticCell but keeping other unsafe patterns.

**Why it happens:** The original code may have multiple unsafe issues; fixing one doesn't fix all.

```rust
// WRONG - StaticCell used but still returns &mut and has aliasing issues
static INSTANCE: StaticCell<ServiceContainer> = StaticCell::new();
pub fn get_instance() -> &'static mut Self {
    unsafe { &mut *INSTANCE.as_ptr() }  // Still unsafe!
}

// CORRECT - StaticCell init returns &static mut; no unsafe needed
static INSTANCE: StaticCell<ServiceContainer> = StaticCell::new();
pub fn get_instance() -> &'static mut Self {
    // .init() can only be called once; returns &'static mut
    INSTANCE.init(ServiceContainer::new())
}

// ALTERNATIVE - if shared access needed, use interior mutability
static INSTANCE: StaticCell<ServiceContainer> = StaticCell::new();
pub fn get_instance() -> &'static ServiceContainer {
    // Return shared reference to avoid aliasing
    unsafe { &*INSTANCE.as_ptr() }
}
```

**Consequences:** Compiler warnings persist; potential undefined behavior remains.

**Prevention:** Verify all unsafe is eliminated; use `&'static` not `&'static mut` where possible.

**Bug context:** Bug E (ServiceContainer::get_instance)

---

### Pitfall 8: Lifetime Transmute Without Proper Ownership

**What goes wrong:** Using `mem::transmute` to extend lifetimes creates use-after-free if original data is dropped.

**Why it happens:** Transmute changes types but doesn't prevent the original data from being dropped.

```rust
// WRONG - lifetime extended but data dropped
fn make_static<T>(value: T) -> &'static mut T {
    unsafe {
        let ptr = Box::into_raw(Box::new(value));
        // value is dropped here!
        &mut *ptr  // DANGLING POINTER!
    }
}

// CORRECT - use StaticCell
static CELL: StaticCell<T> = StaticCell::new();
fn init(value: T) -> &'static mut T {
    CELL.init(value)  // StaticCell owns the data
}
```

**Consequences:** Memory corruption; undefined behavior; intermittent crashes.

**Prevention:** Use StaticCell instead of manual transmute; verify with Miri.

**Bug context:** Bug A (make_static), Bug H (LEDC timer lifetime extension)

---

## Phase-Specific Warning Matrix

| Bug | Phase | Common Pitfall | Mitigation |
|-----|-------|----------------|------------|
| Bug A | 1 | StaticCell double init or not static | Define at module scope; check init |
| Bug C | 1 | Fix test instead of implementation | Fix parser, not test |
| Bug D | 2 | Unsafe replacement incomplete | Verify all unsafe eliminated |
| Bug E | 2 | Aliasing through &mut return | Use &'static or interior mutability |
| Bug F | 3 | Docs updated without verification | Run code, verify behavior first |
| Bug G | 3 | Blocking→async without await | Add .await; use Timer |
| Bug H | 3 | Timer variables but same hardware | Use Timer0 vs Timer1 explicitly |

---

## Detection Strategies

| Bug Type | Tool | How |
|----------|------|-----|
| Use-After-Free | Miri | `cargo +nightly miri test` |
| Unsafe statics | Clippy | `clippy --warn unsafe` |
| Blocking in async | Visual inspection | Search for `.await` after Timer |
| LEDC timer conflict | ESP-IDF logs | Check for "timer conflict" errors |
| Documentation mismatch | Integration test | Run actual commands, compare output |

---

## Recovery Strategies

| Pitfall | Recovery | Cost |
|---------|----------|------|
| StaticCell panic | Refactor to check-before-init pattern | MEDIUM |
| Blocking in async | Add `.await`; use embassy_time::Timer | LOW |
| Test assertion changed | Revert test; fix implementation | LOW |
| LEDC timer conflict | Explicit timer numbers in config | MEDIUM |
| Docs out of sync | Audit code, verify behavior, update docs | LOW |

---

## Additional Pitfall Sections

The following sections cover additional domain-specific pitfalls for this project.

### Pitfall 9: embassy_sync::Mutex Migration — RefCell and Blocking Issues

**What goes wrong:** When migrating from `critical_section::Mutex<RefCell<Option<T>>>` with take/replace pattern to `embassy_sync::Mutex`, several issues can occur:

1. **Keeping RefCell unnecessarily** — The async Mutex already provides exclusive access via MutexGuard; RefCell adds runtime overhead and can cause panics from double borrowing.

2. **Using blocking mutex across await** — The `embassy_sync::blocking_mutex::Mutex` is NOT designed to be held across `.await` points. This causes deadlock.

3. **Wrong RawMutex selection** — Using `ThreadModeRawMutex` when you need interrupt + task sharing, or vice versa.

4. **Lazy initialization pattern mismatch** — Trying to replicate the `Option<T>` lazy init pattern without understanding the new semantics.

**Why it happens:** The original pattern `Mutex<RefCell<Option<T>>>` was necessary for:
- Interior mutability with critical_section (which doesn't provide exclusive access)
- Lazy initialization (None at start, replace with Some later)
- ISR-to-main communication

With embassy_sync's async Mutex, these patterns change fundamentally.

**Consequences:**
- Compilation errors (trait bounds)
- Runtime panics (RefCell borrow violations)
- Deadlocks (blocking across await, wrong RawMutex)
- Data races (wrong RawMutex for context)

**Prevention:**

```rust
// WRONG: Keeping RefCell in async Mutex
use embassy_sync::mutex::Mutex;
use core::cell::RefCell;
static DATA: Mutex<ThreadModeRawMutex, RefCell<Option<RoasterControl>>> = 
    Mutex::new(RefCell::new(None));

// CORRECT: No RefCell needed - MutexGuard provides exclusive access
static DATA: Mutex<ThreadModeRawMutex, RoasterControl> = 
    Mutex::new(RoasterControl::new());

// WRONG: Blocking mutex held across await
static DATA: Mutex<ThreadModeRawMutex, u32> = Mutex::new(0);
async fn bad_example() {
    DATA.lock(|v| {
        *v = 42;
        Timer::after_millis(100).await; // DEADLOCK - holding lock!
    });
}

// CORRECT: Use async Mutex when holding across await
static DATA: embassy_sync::mutex::Mutex<ThreadModeRawMutex, u32> = 
    embassy_sync::mutex::Mutex::new(0);
async fn good_example() {
    let mut guard = DATA.lock().await;
    *guard = 42;
    Timer::after_millis(100).await; // OK - other tasks can run
}

// WRONG: ThreadModeRawMutex in ISR
static DATA: Mutex<ThreadModeRawMutex, u32> = Mutex::new(0);
fn isr_handler() {
    // Not safe! ThreadModeRawMutex doesn't work in interrupt context
}

// CORRECT: Use CriticalSectionRawMutex for ISR + task sharing
static DATA: Mutex<CriticalSectionRawMutex, u32> = Mutex::new(0);
```

**Detection:**
- RefCell in Mutex type → likely unnecessary
- `.await` inside `lock()` closure → blocking across await
- Compilation errors about `Sync` → likely wrong RawMutex
- Excessive `.unwrap()` → wrong initialization pattern

---

### Pitfall 10: embassy_sync Mutex — Trait Bound Issues

**What goes wrong:** "trait `Send` is not implemented" or "trait `Sync` is not implemented" errors.

**Why it happens:** The inner type `T` must be `Send` to be shared between async tasks. The RawMutex must be `Sync`.

**Prevention:**
```rust
// Ensure T is Send:
struct RoasterControl {
    // Use owned types, not borrowed references with non-'static lifetimes
    timer: Timer,           // OK - owned
    // timer: &'a mut Timer, // WRONG - not 'static
}

// Choose correct RawMutex:
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, ThreadModeRawMutex};

// For task-only: ThreadModeRawMutex (or NoopRawMutex)
// For ISR + task: CriticalSectionRawMutex
```

---

## Sources

- embassy-sync official documentation: https://docs.embassy.dev/embassy-sync/git/default/mutex/struct.Mutex.html
- embassy-sync blocking_mutex: https://docs.embassy.dev/embassy-sync/git/default/blocking_mutex/struct.Mutex.html
- Rust forum — Mutex<RefCell<Option<T>>> pattern: https://users.rust-lang.org/t/mutex-refcell-option-t-on-stm32-project/124386
- The Embedded Rustacean — Sharing Data Among Tasks: https://blog.theembeddedrustacean.com/sharing-data-among-tasks-in-rust-embassy-synchronization-primitives

---

*Additional pitfalls research for: LibreRoaster v3.0 embassy_sync::Mutex migration*
*Researched: 2026-02-19*
