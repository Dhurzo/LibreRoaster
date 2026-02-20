# Feature Research: embassy_sync::Mutex for Async-Safe Embedded Access

**Project:** LibreRoaster  
**Domain:** Embedded Rust async synchronization  
**Researched:** 2026-02-19  
**Confidence:** HIGH

## Executive Summary

This research addresses the race condition in `roaster_async_sensor_read()` by replacing the unsafe `take/replace` pattern with `embassy_sync::Mutex`. The current implementation uses `critical_section::Mutex + RefCell + take()` which creates a vulnerability window during async operations where the `RoasterControl` is `None`.

**Key Finding:** `embassy_sync::Mutex` solves this by holding the lock across `.await` points safely, using a two-phase locking mechanism where the raw mutex is only held briefly during lock/unlock operations, not for the entire duration.

## How embassy_sync::Mutex Works

### Architecture

The `embassy_sync::Mutex` provides async-safe mutual exclusion with this critical behavior:

```
┌─────────────────────────────────────────────────────────────────┐
│                  embassy_sync::Mutex<M, T>                      │
├─────────────────────────────────────────────────────────────────┤
│  1. lock().await →waits for internal "is locked" flag          │
│  2. Acquires raw mutex (M) briefly to set flag                  │
│  3. Releases raw mutex - flag indicates "locked"                │
│  4. Task holds "logical" lock across .await points              │
│  5. unlock() → acquires raw mutex, clears flag, wakes waiters  │
└─────────────────────────────────────────────────────────────────┘
```

**Key insight:** The raw mutex is held for only microseconds (flag toggle), not for the entire critical section. This allows the executor to yield between lock acquisition and release.

### API Surface

```rust
use embassy_sync::mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

// Static initialization
static ROASTER_MUTEX: Mutex<CriticalSectionRawMutex, RoasterControl> = 
    Mutex::new(RoasterControl::new());

// In async context
async fn read_sensors() {
    let mut roaster = ROASTER_MUTEX.lock().await;  // Yields if locked
    roaster.read_sensors().await;                  // Safe: lock held across await
    // roaster dropped here → lock released
}
```

### Available Raw Mutex Types

| Type | Use Case | Interrupt-Safe |
|------|----------|----------------|
| `CriticalSectionRawMutex` | Shared between tasks AND ISRs | Yes |
| `ThreadModeRawMutex` | Tasks only, singleton pattern | No |
| `NoopRawMutex` | Tasks only, no overhead | No |

**For ESP32-C3 (RISC-V):** Use `CriticalSectionRawMutex` since LibreRoaster uses interrupt-driven hardware.

## Comparison: critical_section::Mutex vs embassy_sync::Mutex

### The Race Condition Problem

Current code in `service_container.rs` (lines 110-132):

```rust
// DANGEROUS PATTERN - race condition window
pub async fn roaster_async_sensor_read() -> Result<(), ContainerError> {
    // GAP 1: Take leaves None - other tasks see None here!
    let mut roaster: RoasterControl = critical_section::with(|cs| {
        let container = Self::get_instance();
        container.roaster.borrow(cs).borrow_mut().take()  // Returns None
    });
    
    // GAP 2: Async operation - OTHER TASKS CAN RUN AND PANIC!
    // Any concurrent call to with_roaster() sees None
    roaster.read_sensors().await;  // Task yields here - window of vulnerability
    
    // Also do the control update (sync)
    let _ = roaster.update_control(embassy_time::Instant::now());
    
    // GAP 3: Replace puts Some back - only now is it safe
    critical_section::with(|cs| {
        let container = Self::get_instance();
        container.roaster.borrow(cs).borrow_mut().replace(roaster);
    });
    
    Ok(())
}
```

**Race window:** Between `take()` (line 114) and `replace()` (line 128), any concurrent access sees `None` and panics with "already borrowed" or "None".

### Comparison Matrix

| Aspect | critical_section::Mutex | embassy_sync::Mutex |
|--------|------------------------|---------------------|
| **Lock across await** | No - releases between awaits | Yes - holds logically |
| **Blocking during lock** | Yes - blocks executor | Yes - yields to executor |
| **Interrupt safe** | Yes (with CriticalSection) | Yes (with CriticalSectionRawMutex) |
| **Borrow checker** | Manual (RefCell) | Automatic (MutexGuard) |
| **Take/replace pattern** | Required for async ops | Not needed |
| **API style** | Closure-based | Future-based |
| **Memory overhead** | RefCell needed | None extra |

### Why embassy_sync::Mutex Fixes This

With `embassy_sync::Mutex`, the lock is held *logically* across the entire scope:

```rust
// FIXED PATTERN - no race condition
pub async fn roaster_async_sensor_read() -> Result<(), ContainerError> {
    // Lock is acquired here - other tasks wait
    let mut roaster = ROASTER_MUTEX.lock().await;
    
    // Safe: lock still held, other tasks blocked
    roaster.read_sensors().await;
    
    let _ = roaster.update_control(embassy_time::Instant::now());
    
    // Dropping roaster releases lock, wakes waiters
    Ok(())
}
```

**No take/replace needed:** The mutex holds the lock while the task yields, preventing any race window.

## Embedded Considerations

### 1. Memory Footprint

`embassy_sync::Mutex` has minimal overhead:
- `Mutex<M, T>` stores: flag + raw mutex
- No heap allocation required
- Size: ~8 bytes overhead on top of T

### 2. Executor Integration

The mutex integrates with embassy executor:
- Uses `Waker` registration to notify waiting tasks
- When lock released, executor wakes waiting task
- Zero busy-waiting (CPU sleeps until woken)

### 3. Critical Section Choice

For ESP32-C3 with interrupts:

```rust
// Recommended for LibreRoaster
use embassy_sync::mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

// Use CriticalSectionRawMutex because:
// - ISRs may need to interact with shared hardware
// - Provides true mutual exclusion across all contexts
static ROASTER: Mutex<CriticalSectionRawMutex, RoasterControl> = 
    Mutex::new(RoasterControl::new());
```

### 4. No RefCell Needed

The async mutex provides interior mutability without RefCell:

```rust
// Before: Manual borrow tracking
critical_section::with(|cs| {
    let mut roaster = container.roaster.borrow(cs).borrow_mut();  // RefCell
});

// After: Automatic borrow tracking  
let mut roaster = ROASTER.lock().await;  // MutexGuard
```

### 5. Deadlock Prevention

Best practices for embedded:

1. **Keep locks short** - don't hold across long operations
2. **Never await while holding two locks** - lock ordering
3. **Use try_lock when appropriate** - for non-blocking paths

```rust
// Non-blocking variant
if let Some(mut roaster) = ROASTER.try_lock() {
    roaster.read_sensors().await;
} else {
    // Skip this cycle, try again later
}
```

## Feature Recommendations for v4.0

### Required Features

| Feature | Description | Complexity |
|---------|-------------|------------|
| Replace `critical_section::Mutex<RefCell<Option<T>>>` with `embassy_sync::Mutex` | Core migration | Medium |
| Remove `take()/replace()` pattern | Simplifies code, removes race window | Low |
| Update `roaster_async_sensor_read()` | Use new lock pattern | Low |
| Verify no borrow errors during concurrent access | Integration testing | Medium |

### Migration Path

1. **Phase 1:** Add `embassy_sync::Mutex` wrapper around existing `RoasterControl`
2. **Phase 2:** Update all `with_roaster()` callers to use async lock
3. **Phase 3:** Remove `take/replace` pattern from `roaster_async_sensor_read()`
4. **Phase 4:** Verify with concurrent sensor reads

### What to Keep

- `ServiceContainer` structure for dependency injection
- `CriticalSectionRawMutex` for interrupt safety
- Error handling via `ContainerError`

### What Changes

- `Mutex<RefCell<Option<T>>>` → `Mutex<CriticalSectionRawMutex, T>`
- Synchronous `with_roaster()` → async `roaster.lock().await`
- Explicit `take()/replace()` → implicit lock scope

## Anti-Features to Avoid

### Anti-Feature 1: Using Blocking Mutex

**What:** Using `embassy_sync::blocking_mutex::Mutex` (not async)

```rust
// WRONG - cannot hold across await
use embassy_sync::blocking_mutex::Mutex;
static MUTEX: Mutex<CriticalSectionRawMutex, RoasterControl> = 
    Mutex::new(RoasterControl::new());

async fn broken() {
    // This won't compile - blocking lock cannot await
    let guard = MUTEX.lock();  // Returns guard, not Future
}
```

**Why:** The blocking mutex is designed for short-lived critical sections only. Use async `Mutex` instead.

### Anti-Feature 2: Mixing Mutex Types

**What:** Using different mutex types for same resource

```rust
// WRONG - inconsistent locking
static M1: Mutex<CriticalSectionRawMutex, T> = ...;
static M2: Mutex<ThreadModeRawMutex, T> = ...;  // Different type!
```

**Why:** Different raw mutex types may have different safety guarantees. Pick one and use consistently.

### Anti-Feature 3: Holding Lock Across Long Operations

**What:** Keeping mutex locked during long I/O

```rust
// AVOID - blocks other tasks
async fn bad_pattern() {
    let mut resource = MUTEX.lock().await;
    // Don't do this:
    Timer::after_secs(10).await;  // Other tasks blocked for 10s
}
```

**Why:** While safe, this reduces concurrency. Copy data out, release lock, process.

## Dependencies

Already present in `Cargo.toml`:
```toml
embassy-sync = "0.6.1"  # Already included
```

No new dependencies needed.

## Sources

- **HIGH:** [embassy_sync::Mutex official docs](https://docs.embassy.dev/embassy-sync/git/default/mutex/struct.Mutex.html) - Primary API reference
- **HIGH:** [embassy_sync blocking_mutex docs](https://docs.embassy.dev/embassy-sync/git/default/blocking_mutex/struct.Mutex.html) - Raw mutex types
- **MEDIUM:** [The Embedded Rustacean Blog - Sharing Data Among Tasks](https://blog.theembeddedrustacean.com/sharing-data-among-tasks-in-rust-embassy-synchronization-primitives) - Tutorial with examples
- **MEDIUM:** [crates.io embassy-sync](https://crates.io/crates/embassy-sync) - Version history and stats

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| API behavior | HIGH | Verified via official docs |
| Comparison | HIGH | Direct contrast based on source |
| Embedded considerations | HIGH | ESP32-C3 context verified |
| Migration path | MEDIUM | Pattern recommended, needs implementation verification |
| Race condition fix | HIGH | Architecture prevents take/replace window |

## Conclusion

`embassy_sync::Mutex` is the correct solution for LibreRoaster's race condition. It provides:

1. **Async-safe locking** - holds across `.await` points
2. **Zero runtime overhead** - raw mutex only held microseconds
3. **Interrupt compatible** - with CriticalSectionRawMutex
4. **Simpler code** - no manual take/replace management

The migration from `critical_section::Mutex<RefCell<Option<T>>>` to `embassy_sync::Mutex<CriticalSectionRawMutex, T>` eliminates the race window while maintaining interrupt safety.
