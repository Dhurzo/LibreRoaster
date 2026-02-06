# Phase 1: Parser Tests - Research

**Researched:** 2026-02-04
**Domain:** Rust embedded unit testing, boundary value testing, parser validation
**Confidence:** HIGH

## Summary

Research confirms that the existing parser testing approach in `src/input/parser.rs` follows Rust embedded best practices. The parser uses `heapless::Vec<&str, 4>` which has no special testing considerations—it works identically in both `std` and `no_std` environments. Boundary value testing for `u8` values (0-100) requires simple addition of test cases for edge conditions. The current `#[cfg(test)] mod tests` co-location pattern is the standard approach and should be continued. Tests can run with standard Rust `cargo test` using the `test` feature that enables `std`.

**Primary recommendation:** Add 5 new boundary value tests to the existing inline test module in `src/input/parser.rs`. No structural changes needed.

## Standard Stack

The established testing tools for this Rust embedded project:

### Core Testing Tools
| Tool | Version | Purpose | Why Standard |
|------|---------|---------|-------------|
| Rust `#[test]` attribute | Built-in | Test function definition | Standard Rust testing primitive |
| `assert!` macro | Built-in | Basic assertion | Standard Rust assertion |
| `matches!` macro | Built-in | Pattern matching assertions | Rust 2021 edition stable macro |
| `heapless` | 0.8.0 | Fixed-capacity collections | Works identically in `std`/`no_std` |

### Test Configuration
| Feature | Purpose | How It Enables Testing |
|---------|---------|----------------------|
| `test = ["std"]` | Enable standard library for tests | Parser uses `std::str::parse()` which works |
| `std` feature | Enable full standard library | Required for `cargo test` |

**Installation:**
No new dependencies needed—standard Rust testing is already configured.

## Architecture Patterns

### Recommended Test Organization

**Current Pattern (Continue Using):**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function() {
        // Test implementation
    }
}
```

**Why This Pattern Works:**
- Tests are co-located with code they test
- `#[cfg(test)]` ensures tests compile only in test mode
- `use super::*` brings parent module items into test scope
- No separate test files needed for unit tests
- Works identically in `std` and `no_std` contexts

### Alternative Patterns Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Inline `#[cfg(test)]` module | Separate `tests/` directory | Less visibility, extra directory |
| Inline tests | Integration tests in `tests/` | More overhead for unit-level tests |

**Recommendation:** Continue with inline `#[cfg(test)]` module. It's already established and appropriate for parser unit tests.

### Boundary Value Testing Pattern

**Standard Approach for u8 (0-255) Valid Range (0-100):**
```rust
#[test]
fn test_parse_ot1_boundary_zero() {
    let result = parse_artisan_command("OT1 0");
    assert!(matches!(result, Ok(ArtisanCommand::SetHeater(0))));
}

#[test]
fn test_parse_ot1_boundary_max() {
    let result = parse_artisan_command("OT1 100");
    assert!(matches!(result, Ok(ArtisanCommand::SetHeater(100))));
}

#[test]
fn test_parse_ot1_invalid_above_max() {
    let result = parse_artisan_command("OT1 150");
    assert!(matches!(result, Err(ParseError::InvalidValue)));
}
```

**Boundary Testing Principles:**
- Test both edges of valid range (0, 100)
- Test first value outside valid range (101 or any >100)
- Test far outside value to ensure range check works (150 existing test covers this)
- Include whitespace variations: `"OT1 0"`, `"OT1  0"` (handled by `split_whitespace`)

### Parser Test Structure Pattern

**Existing Test Names to Follow Convention:**
```rust
// Current pattern: test_parse_<command>_<modifier>
// New tests should follow the same pattern:
test_parse_ot1_zero           // OT1 0
test_parse_ot1_max            // OT1 100
test_parse_io3_zero           // IO3 0
test_parse_io3_max            // IO3 100
test_parse_io3_invalid_above  // IO3 150
```

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Parsing u8 from string | Custom parsing logic | `str::parse::<u8>()` | Handles overflow, leading zeros, whitespace correctly |
| Testing framework | Custom test runner | Built-in `#[test]` | Integrates with `cargo test`, standard library |
| Pattern matching tests | Multiple `assert_eq!` | `matches!` macro | More readable, handles nested patterns |
| Test organization | Custom module system | `#[cfg(test)] mod tests` | Idiomatic Rust, automatic test discovery |

**Key insight:** The Rust standard library and cargo already provide complete testing infrastructure. No additional crates needed for basic unit testing.

## Common Pitfalls

### Pitfall 1: Not Testing Whitespace Variations
**What goes wrong:** Parser may fail on commands like `"OT1  0"` (double space) or `" OT1 0 "` (leading/trailing space).
**Why it happens:** `split_whitespace()` handles multiple spaces correctly, but this behavior isn't verified.
**How to avoid:** Add test cases with extra whitespace:
```rust
#[test]
fn test_parse_ot1_extra_whitespace() {
    let result = parse_artisan_command("OT1  0");
    assert!(matches!(result, Ok(ArtisanCommand::SetHeater(0))));
}
```
**Warning signs:** If `split_whitespace()` behavior changes, parser breaks silently.

### Pitfall 2: Confusing `ParseError` Types
**What goes wrong:** `parse_percentage` returns `ParseError::InvalidValue` for both non-numeric input (`"OT1 abc"`) and out-of-range values (`"OT1 150"`).
**Why it happens:** Single error type used for different failure modes.
**How to avoid:** Tests verify the single `InvalidValue` error is correct, but consider if semantic distinction matters for error reporting to users.
**Warning signs:** User can't distinguish "not a number" from "value too high" in error responses.

### Pitfall 3: Testing Without `std` Feature
**What goes wrong:** Running `cargo test` without enabling `test` feature may fail to compile.
**Why it happens:** Parser code uses heapless which works in `no_std`, but test code may need std for certain operations.
**How to avoid:** Always run tests with `cargo test --features test` or ensure `default` includes test feature.
**Warning signs:** Compilation errors mentioning `std` when running tests.

### Pitfall 4: Integer Overflow in Boundary Testing
**What goes wrong:** Testing with values like `u8::MAX` (255) could cause unexpected behavior if parsing logic changes.
**Why it happens:** Parser accepts any u8 value, then checks range.
**How to avoid:** Current tests with 150 and boundary values (0, 100) are sufficient. No need to test u8::MAX.
**Warning signs:** If parse logic changes to accept all u8 values, tests would need updating.

## Code Examples

### Complete Boundary Value Test Suite

```rust
// Add these tests to #[cfg(test)] mod tests in src/input/parser.rs

#[test]
fn test_parse_ot1_zero() {
    let result = parse_artisan_command("OT1 0");
    assert!(matches!(result, Ok(ArtisanCommand::SetHeater(0))));
}

#[test]
fn test_parse_ot1_max() {
    let result = parse_artisan_command("OT1 100");
    assert!(matches!(result, Ok(ArtisanCommand::SetHeater(100))));
}

#[test]
fn test_parse_io3_zero() {
    let result = parse_artisan_command("IO3 0");
    assert!(matches!(result, Ok(ArtisanCommand::SetFan(0))));
}

#[test]
fn test_parse_io3_max() {
    let result = parse_artisan_command("IO3 100");
    assert!(matches!(result, Ok(ArtisanCommand::SetFan(100))));
}

#[test]
fn test_parse_io3_invalid_above_max() {
    let result = parse_artisan_command("IO3 150");
    assert!(matches!(result, Err(ParseError::InvalidValue)));
}
```

### Whitespace Variation Tests (Optional Enhancement)

```rust
#[test]
fn test_parse_ot1_with_whitespace() {
    let result = parse_artisan_command("  OT1 0  ");
    assert!(matches!(result, Ok(ArtisanCommand::SetHeater(0))));
}

#[test]
fn test_parse_io3_with_whitespace() {
    let result = parse_artisan_command("  IO3 100  ");
    assert!(matches!(result, Ok(ArtisanCommand::SetFan(100))));
}
```

### Running Tests

```bash
# Run all parser tests
cargo test --features test parse

# Run only boundary value tests
cargo test --features test test_parse_ot1_zero
cargo test --features test test_parse_ot1_max
cargo test --features test test_parse_io3_zero
cargo test --features test test_parse_io3_max
cargo test --features test test_parse_io3_invalid_above

# Run all tests including parser
cargo test --features test
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Custom test frameworks | Built-in `#[test]` | Rust 1.0 (2015) | Simpler, standard tooling |
| External test runners | `cargo test` integration | Rust 1.0 (2015) | Unified workflow |
| Third-party assertions | `matches!` macro | Rust 2021 edition | Idiomatic pattern matching |

**Deprecated/outdated:**
- `panic!` based tests - Use `assert!`, `assert_eq!`, `matches!`
- External test crates (rusttest, quicktest) - Built-in testing is sufficient for unit tests
- Integration tests in `tests/` for unit-level functionality - Inline tests are better for unit tests

## Open Questions

### Question 1: Should IO3 > 100 error test be added?
**What we know:** OT1 > 100 is tested (existing `test_invalid_value`). IO3 > 100 follows identical logic but isn't tested.
**What's unclear:** Is this truly missing, or was it intentionally omitted as "obviously same behavior"?
**Recommendation:** Add `test_parse_io3_invalid_above` for completeness. Tests should explicitly verify all command paths.

### Question 2: Should whitespace variations be tested?
**What we know:** `trim()` and `split_whitespace()` handle whitespace. Current tests don't verify whitespace behavior.
**What's unclear:** Is whitespace handling considered implementation detail or contract?
**Recommendation:** Add at least one whitespace variation test (`"OT1 0"` with extra spaces) to verify contract.

### Question 3: Should error types be differentiated?
**What we know:** Both numeric parse errors and range errors return `ParseError::InvalidValue`.
**What's unclear:** Does the Artisan+ protocol specification require distinguishing these error types?
**Recommendation:** Out of scope for this phase. Add test comment noting this for future consideration if error handling needs improvement.

## Sources

### Primary (HIGH confidence)
- **Rust Standard Library Documentation** - `#[test]` attribute, `assert!`, `matches!` macro behavior
- **The Rust Programming Language** - Chapter on testing patterns
- **Rust Embedded Book** - `no_std` testing approaches
- **heapless crate documentation** - `Vec` behavior in `std`/`no_std` contexts

### Secondary (MEDIUM confidence)
- **Rust by Example - Testing** - Standard testing patterns
- **cargo test documentation** - Test execution configuration
- **embedded-hal testing patterns** - How other embedded projects structure tests

### Tertiary (LOW confidence)
- **Community patterns from rust-embedded projects** - Observed in multiple ESP32-C3 projects
- **GitHub - Popular embedded Rust projects** - Common testing organization patterns

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Built-in Rust testing is stable and well-documented
- Architecture: HIGH - Established patterns confirmed by existing codebase
- Pitfalls: HIGH - Common Rust testing issues are well-known
- Boundary testing: HIGH - Standard u8 range validation patterns

**Research date:** 2026-02-04
**Valid until:** Indefinite - Rust testing patterns are stable

**Tests to add:**
1. `test_parse_ot1_zero` - OT1 0 boundary (heater off)
2. `test_parse_ot1_max` - OT1 100 boundary (heater max)
3. `test_parse_io3_zero` - IO3 0 boundary (fan off)
4. `test_parse_io3_max` - IO3 100 boundary (fan max)
5. `test_parse_io3_invalid_above` - IO3 > 100 error

**Optional enhancements:**
- `test_parse_ot1_with_whitespace` - Verify whitespace handling
- `test_parse_io3_with_whitespace` - Verify whitespace handling
