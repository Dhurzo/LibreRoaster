# Phase 49: Safety Static Fixes - Research

**Researched:** 2026-02-18
**Domain:** Embedded Rust memory safety / static initialization
**Confidence:** HIGH

## Summary

This phase requires replacing unsafe static/mutable patterns with the `StaticCell` pattern from the `static_cell` crate (version 2.1.1). The project has four specific unsafe patterns that need refactoring:

1. **SAFE-01**: The `make_static` function in `main.rs` creates a use-after-free by returning a reference to stack-allocated memory
2. **SAFE-02**: `get_usb_cdc_driver()` uses unsafe `static_mut_refs` pattern
3. **SAFE-03**: `get_uart_driver()` uses unsafe `static_mut_refs` pattern
4. **SAFE-04**: `ServiceContainer::get_instance()` uses unsafe `static mut`

The `StaticCell` crate provides a safe, no-std-compatible way to reserve memory at compile time and initialize it at runtime with a `'static` reference.

**Primary recommendation:** Use `StaticCell::init()` for straightforward cases and `StaticCell::init_with()` for large values to avoid stack overflow. Add inline SAFETY comments at each usage site explaining why the pattern is safe.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| static_cell | 2.1.1 | Safe static initialization for embedded Rust | Embassy ecosystem standard, no-std compatible, compile-time memory allocation |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| critical_section | (already used) | Thread-safe access to statics | Already in project, used for mutex access |
| embassy_sync | (already used) | Channel and blocking mutex | Already in project |

**Installation:**
```bash
# Already in Cargo.toml - no changes needed
static_cell = "2.1.1"
```

## Architecture Patterns

### Recommended StaticCell Usage Pattern

```rust
use static_cell::StaticCell;

// 1. Declare static at compile time
static STATIC_NAME: StaticCell<T> = StaticCell::new();

// 2. Initialize at runtime, getting &'static mut
let reference: &'static mut T = STATIC_NAME.init(value);

// Or for large values to avoid stack overflow:
let reference: &'static mut T = STATIC_NAME.init_with(|| complex_value);
```

### Pattern 1: Replacing unsafe make_static

**What:** Replace the unsafe `make_static` function that returns reference to stack memory

**When to use:** When you need to convert a local variable to static lifetime

**Current (unsafe) code in main.rs:**
```rust
/// SAFETY: The caller must ensure that the returned reference is only used
/// for the lifetime of the program, and that `value` is not dropped while the reference is in use.
#[cfg(target_arch = "riscv32")]
unsafe fn make_static<T>(mut value: T) -> &'static mut T {
    let ptr = &mut value as *mut T;
    &mut *ptr  // BUG: value is dropped at end of function, but reference is returned!
}
```

**Fixed approach:**
```rust
use static_cell::StaticCell;

// Instead of calling unsafe make_static(value):
// static STATIC_VAR: StaticCell<T> = StaticCell::new();
// let static_ref = STATIC_VAR.init(value);
```

### Pattern 2: Replacing mutable static with StaticCell

**What:** Replace `static mut INSTANCE: Option<T> = None` with `static STATIC_CELL: StaticCell<Option<T>> = StaticCell::new()`

**When to use:** When you need global mutable state that is initialized once at runtime

**Current (unsafe) code in usb_cdc/driver.rs:**
```rust
static mut USB_CDC_INSTANCE: Option<UsbCdcDriver> = None;

pub fn get_usb_cdc_driver() -> Option<&'static mut UsbCdcDriver> {
    #[allow(static_mut_refs)]
    unsafe {
        USB_CDC_INSTANCE.as_mut()
    }
}
```

**Fixed approach:**
```rust
use static_cell::StaticCell;

static USB_CDC_INSTANCE: StaticCell<Option<UsbCdcDriver>> = StaticCell::new();

pub fn get_usb_cdc_driver() -> Option<&'static mut UsbCdcDriver> {
    USB_CDC_INSTANCE.init(Option::None); // Initialize once
    // Or better: initialize in init_usb_cdc and use get()
    USB_CDC_INSTANCE.get().as_mut()
}
```

### Pattern 3: ServiceContainer Singleton

**What:** Replace unsafe `static mut INSTANCE` singleton pattern

**Current (unsafe) code in service_container.rs:**
```rust
pub fn get_instance() -> &'static mut Self {
    static mut INSTANCE: ServiceContainer = ServiceContainer::new();
    unsafe { &mut *core::ptr::addr_of_mut!(INSTANCE) }
}
```

**Fixed approach using ConstStaticCell:**
```rust
use static_cell::ConstStaticCell;

static INSTANCE: ConstStaticCell<ServiceContainer> = ConstStaticCell::new();

pub fn get_instance() -> &'static mut Self {
    INSTANCE.take()
}
```

Note: `ConstStaticCell` is ideal for singletons that can be taken once and used forever.

### Anti-Patterns to Avoid

- **Using `make_static` or similar stack-to-static functions:** These create use-after-free bugs - the stack value is dropped but references to it are returned
- **`static mut` without proper synchronization:** Mutable statics require `unsafe` and can cause data races
- **Double initialization of StaticCell:** Calling `init()` twice on the same cell will panic - ensure single initialization
- **Large values with `init()`:** For large types, use `init_with(|| ...)` to construct in-place rather than moving through stack

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Converting stack value to static | `unsafe fn make_static<T>(value: T) -> &'static mut T` | `StaticCell::init()` | The naive approach creates use-after-free; StaticCell reserves compile-time memory |
| Global mutable state | `static mut VAR: T = ...` | `StaticCell<T>` with interior mutability | StaticCell provides safe initialization with compile-time memory reservation |
| Singleton pattern | `static mut INSTANCE` with `addr_of_mut!` | `ConstStaticCell::take()` | ConstStaticCell is designed for this exact use case |

**Key insight:** Embedded Rust requires all memory to be statically allocated. The `StaticCell` crate provides compile-time memory reservation with runtime initialization - the standard solution in the Embassy ecosystem.

## Common Pitfalls

### Pitfall 1: Double Initialization Panic
**What goes wrong:** Calling `StaticCell::init()` twice panics
**Why it happens:** StaticCell is designed for one-time initialization
**How to avoid:** Initialize once in an `init_*` function, or use pattern that checks if already initialized
**Warning signs:** Panic with message "init called twice on StaticCell"

### Pitfall 2: Stack Overflow with Large Values
**What goes wrong:** Stack overflow when using `init(large_value)` for big structs
**Why it happens:** `init()` moves value through the stack before writing to StaticCell
**How to avoid:** Use `init_with(|| large_value)` which constructs in-place
**Warning signs:** Hard-to-debug crashes on embedded systems; may appear as random memory corruption

### Pitfall 3: Forgetting Interior Mutability
**What goes wrong:** Cannot mutate data even with `&'static mut`
**Why it happens:** In async context with multiple tasks, exclusive mutable access isn't enough
**How to avoid:** Use `critical_section::Mutex<RefCell<T>>` or similar for safe shared mutation
**Warning signs:** Borrow checker errors when trying to use the static from multiple tasks

### Pitfall 4: Unsafe Comments Without Real Safety Justification
**What goes wrong:** Adding "SAFETY:" comments that don't actually explain why the code is safe
**Why it happens:** Meeting a requirement without understanding the safety properties
**How to avoid:** Actually document: (1) who initializes, (2) when, (3) what prevents data races, (4) what prevents use-after-free
**Warning signs:** Vague comments like "SAFETY: trusted code" or "SAFETY: single-threaded"

## Code Examples

### Example 1: Basic StaticCell Initialization
```rust
// Source: https://docs.rs/static_cell/latest/static_cell/
use static_cell::StaticCell;

static SOME_INT: StaticCell<u32> = StaticCell::new();

fn example() {
    // Initialize once, get &'static mut
    let x: &'static mut u32 = SOME_INT.init(42);
    assert_eq!(*x, 42);
}
```

### Example 2: ConstStaticCell for Singleton
```rust
// Source: https://docs.rs/static_cell/latest/static_cell/struct.ConstStaticCell.html
use static_cell::ConstStaticCell;

static INSTANCE: ConstStaticCell<MyService> = ConstStaticCell::new();

fn get_instance() -> &'static mut MyService {
    // take() can only be called once - panics if called again
    INSTANCE.take()
}
```

### Example 3: init_with for Large Values
```rust
// Source: https://docs.rs/static_cell/latest/static_cell/struct.StaticCell.html
use static_cell::StaticCell;

static LARGE_BUFFER: StaticCell<LargeStruct> = StaticCell::new();

fn init() -> &'static mut LargeStruct {
    // Construct in-place to avoid stack overflow
    LARGE_BUFFER.init_with(|| LargeStruct::new())
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `static mut` with raw pointers | StaticCell pattern | Pre-2020 → Now | Compile-time memory safety, one-time init guarantee |
| `lazy_static` crate | `static_cell` crate | 2020+ | No-std compatible, no macros required |
| Custom `make_static` functions | `StaticCell::init()` or `make_static!` macro | Various | Eliminates use-after-free bugs |

**Deprecated/outdated:**
- `static_mut_refs` lint (denied in Rust 2024 edition): Use StaticCell instead
- Manual `mem::transmute` for lifetime extension: Use StaticCell
- ` Box::leak()` for static initialization: StaticCell is zero-cost and doesn't require alloc

## Open Questions

None - the approach is well-defined from the project context.

## Sources

### Primary (HIGH confidence)
- https://docs.rs/static_cell/latest/static_cell/ - Official crate documentation
- https://github.com/embassy-rs/static-cell - Official GitHub repository
- Code examination of project files: main.rs, usb_cdc/driver.rs, uart/driver.rs, service_container.rs

### Secondary (MEDIUM confidence)
- https://esp32.implrust.com/wifi/embassy/connecting-wifi.html - impl Rust for ESP32 tutorial showing StaticCell usage patterns

### Tertiary (LOW confidence)
- N/A - no WebSearch-only sources needed for this well-documented crate

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH - static_cell 2.1.1 is in project, well-documented
- Architecture: HIGH - clear patterns from docs and existing usage in project
- Pitfalls: HIGH - documented in official docs and well-understood

**Research date:** 2026-02-18
**Valid until:** 30 days (static_cell API is stable)
