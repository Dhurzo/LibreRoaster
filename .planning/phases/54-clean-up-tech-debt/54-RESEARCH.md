# Phase 54: Clean Up Tech Debt - Research

**Researched:** 2026-02-18
**Domain:** Rust embedded project (riscv32) dead code cleanup and warning fixes
**Confidence:** HIGH

## Summary

This phase involves removing dead code and fixing compilation warnings in a Rust embedded project (ESP32-C3) that targets `riscv32imc-unknown-none-elf`. The specific tasks are: (1) remove unused `fan_timer` and `ssr_timer` fields in ledc_bus.rs, (2) fix 12+ compilation warnings, and (3) fix integration tests to compile with the `std` feature on the host target.

The standard approach for Rust dead code cleanup is to use `cargo clippy` to identify issues, then either remove unused code or add `#[allow(dead_code)]` attributes with documentation explaining why kept code is retained. For warnings, the approach is to fix them directly rather than suppress them, except for `async_fn_in_trait` which requires a specific lint attribute if the trait is internal-only.

**Primary recommendation:** Remove the dead fields/functions directly (they are truly unused), prefix unused variables with underscore, add `#[allow(async_fn_in_trait)]` to the internal AsyncThermometer trait, and create a host-compatible test configuration for integration tests.

## Standard Stack

This is an embedded Rust project using the following key components:

### Core
| Component | Why Standard |
 | Version | Purpose|-----------|---------|---------|--------------|
| Rust | 1.88+ | Language | Required by project |
| cargo | latest | Build tool | Standard Rust toolchain |
| clippy | bundled | Linting | Standard Rust linter |

### Supporting
| Component | Version | Purpose | When to Use |
|-----------|---------|---------|-------------|
| esp-hal | ~1.0 | ESP32C3 hardware abstraction | Embedded target only |
| embedded-hal | 1.0 | Portable hardware traits | Cross-platform hardware |
| embassy-executor | 0.9.1 | Async executor | Embedded async runtime |

**Installation:**
```bash
# Standard Rust toolchain already includes cargo and clippy
rustup update
cargo update
```

## Architecture Patterns

### Dead Code Handling Pattern

The codebase already uses `#[allow(dead_code)]` for potentially useful kept code:

```rust
// Source: src/input/multiplexer.rs (lines 29-33)
// NOTE: Handshake (CHAN → UNITS → FILT) is DISABLED for Artisan Scope compatibility
// Placeholder types kept for potential future re-enabling
#[allow(dead_code)]
pub struct InitState;

#[allow(dead_code)]
pub struct InitEvent;
```

**When to use:** Keep code that's not currently used but may be needed later. Document why it's retained.

**When to remove:** Code that is genuinely unused with no potential future use case.

### Warning Fix Patterns

#### Unused Variable
```rust
// Fix: prefix with underscore OR use the value
let _update_result = ServiceContainer::with_roaster(|...| { ... });
// OR simply remove if result is truly not needed
```

#### Unused Fields/Struct Members
```rust
// Remove completely if truly unused
struct LedcBus {
    fan: ChannelEntry,
    ssr: ChannelEntry,
    // REMOVE: fan_timer: u8,
    // REMOVE: ssr_timer: u8,
}
```

#### async_fn_in_trait Warning
```rust
// Source: src/control/traits.rs
// Add #[allow] attribute since trait is internal-only
#[allow(async_fn_in_trait)]
pub trait AsyncThermometer: Send {
    async fn read_temperature_async(&mut self) -> Result<f32, RoasterError>;
}
```

### Anti-Patterns to Avoid

- **Don't suppress warnings globally:** Use specific `#[allow(...)]` attributes per item
- **Don't delete potentially useful code:** Use `#[allow(dead_code)]` with documentation
- **Don't set deny level for future:** Per context, this is a one-time cleanup only
- **Don't use mutable statics in new code:** Existing patterns are marked as "intentional" per prior phases

## Don't Hand-Roll

This project already has patterns in place. Key points:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Dead code detection | Custom tooling | `cargo clippy` or `cargo build` | Standard Rust linting |
| Unused variables | Ignore | Prefix with `_` or remove | Compiler built-in |
| Warning suppression | Global deny | Per-item `#[allow()]` | Standard Rust practice |

**Key insight:** Rust's built-in lint system handles all these cases. No need for custom scripts.

## Common Pitfalls

### Pitfall 1: Removing Struct Fields That Affect Drop Behavior
**What goes wrong:** Removing fields that have side-effects on drop can change program behavior.
**Why it happens:** The dead_code lint warns about this but doesn't prevent removal.
**How to avoid:** Verify the field type doesn't implement Drop with side-effects or contain types like `Mutex`, `File`, etc.
**Warning signs:** Struct has complex drop logic, field type is `Drop` or contains `Drop` types.

### Pitfall 2: Breaking Integration Tests with Target-Specific Code
**What goes wrong:** Integration tests fail when compiled for embedded target due to missing ESP-specific dependencies.
**Why it happens:** ESP-hal and related crates are only available for riscv32 target.
**How to avoid:** Use conditional compilation (`#[cfg(target_arch = "riscv32")]`) and provide host-compatible alternatives. Run integration tests on host target (`x86_64-unknown-linux-gnu`) with `--features std`.
**Warning signs:** Compilation errors mentioning `esp_hal`, `esp32c3` when building tests.

### Pitfall 3: Forgetting to Document Kept Dead Code
**What goes why:** Future developers won't understand why dead code was kept.
**Why it happens:** Rush to fix warnings without documentation.
**How to avoid:** Always add `/// Reason: ...` or `// NOTE: ...` comments when using `#[allow(dead_code)]`.
**Warning signs:** `#[allow(dead_code)]` without explanatory comment.

## Code Examples

### Fixing Unused Variable
```rust
// BEFORE (src/application/tasks.rs:67)
let update_result = ServiceContainer::with_roaster(...);

// AFTER - either prefix with underscore
let _update_result = ServiceContainer::with_roaster(...);

// OR remove completely if not needed
ServiceContainer::with_roaster(...);
```

### Fixing Dead Fields
```rust
// BEFORE (src/hardware/ledc_bus.rs:67-74)
pub struct LedcBus<'a> {
    guard: LedcGuard,
    fan: ChannelEntry<'a>,
    ssr: ChannelEntry<'a>,
    /// Timer number used by Fan channel (Timer1 = 25kHz)
    fan_timer: u8,      // REMOVE
    /// Timer number used by SSR channel (Timer0 = 1Hz)
    ssr_timer: u8,      // REMOVE
}

// AFTER
pub struct LedcBus<'a> {
    guard: LedcGuard,
    fan: ChannelEntry<'a>,
    ssr: ChannelEntry<'a>,
    // Note: Timer configuration handled internally by Channel implementation
}
```

### Fixing Unused Functions
```rust
// BEFORE (src/hardware/uart/tasks.rs)
// These functions are defined but never called - remove them
fn handle_complete_command(command: &[u8]) { ... }
fn send_parse_error(error: ParseError) { ... }

// AFTER - remove both functions
```

### Suppressing async_fn_in_trait
```rust
// BEFORE (src/control/traits.rs:11-13)
pub trait AsyncThermometer: Send {
    async fn read_temperature_async(&mut self) -> Result<f32, RoasterError>;
}

// AFTER
#[allow(async_fn_in_trait)]
pub trait AsyncThermometer: Send {
    async fn read_temperature_async(&mut self) -> Result<f32, RoasterError>;
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Ignore warnings | Fix all warnings | Now | Clean build |
| Delete all unused code | Use `#[allow(dead_code)]` for useful code | Prior phases | Preserves potentially useful code |
| Tests on embedded target | Run on host target with mocks | Now | Integration tests work |

**Deprecated/outdated:**
- `static_mut_refs` patterns: These are marked as "existing patterns from prior phases" in the context - they should be left as-is per the phase boundary.

## Open Questions

1. **Integration tests on host target**
   - What we know: The code uses ESP-specific modules (esp-hal, max31856, ledc_bus) that don't exist on host.
   - What's unclear: Whether existing mocks are sufficient, or new mocks need to be created.
   - Recommendation: First attempt to run tests on host target to identify missing mocks.

2. **static_mut_refs warnings**
   - What we know: These exist in uart/driver.rs, uart/tasks.rs, usb_cdc/driver.rs, usb_cdc/tasks.rs.
   - What's unclear: Whether these should be fixed or left as-is.
   - Recommendation: Leave as-is per "existing patterns from prior phases" in phase context.

## Sources

### Primary (HIGH confidence)
- Rust Compiler Lints Documentation - https://doc.rust-lang.org/rustc/lints/listing/warn-by-default.html
- Rust dead_code lint - https://doc.rust-lang.org/stable/rustc_lint/builtin/static.DEAD_CODE.html
- Rust async_fn_in_trait lint - https://doc.rust-lang.org/nightly/nightly-rustc/rustc_lint/async_fn_in_trait/static.ASYNC_FN_IN_TRAIT.html

### Secondary (MEDIUM confidence)
- Code examples from existing codebase patterns (multiplexer.rs)
- Cargo.toml feature configuration analysis

### Tertiary (LOW confidence)
- Web search results for best practices (verified against official docs)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Uses standard Rust tooling (cargo, clippy)
- Architecture: HIGH - Based on existing codebase patterns and Rust idioms
- Pitfalls: HIGH - Known Rust behaviors verified with official documentation

**Research date:** 2026-02-18
**Valid until:** 30 days (stable Rust tooling)
