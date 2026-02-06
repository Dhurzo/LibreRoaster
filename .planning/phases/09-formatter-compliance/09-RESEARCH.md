# Phase 09: Formatter Compliance - Research

**Researched:** 2026-02-04
**Domain:** Embedded UART response formatting and error schema (Artisan protocol)
**Confidence:** MEDIUM

## Summary

This phase targets deterministic output formatting and a stable, parseable error schema for Artisan command responses. The current code already uses explicit precision formatting and appends CRLF at the UART boundary, but it also includes silent clamping in `format_read_response` and inconsistent error payload structure between parse errors and handler failures.

The standard approach in this codebase is to format data using explicit format strings (`{:.1}`, `{:.2}`), to keep column order fixed, and to append CRLF in UART output tasks rather than inside formatter logic. Errors are emitted with the `ERR` prefix plus reason codes at parse time, but handler errors add a debug payload that breaks a strict code/message schema.

**Primary recommendation:** Centralize formatting rules (precision, separators, order, CRLF), standardize `ERR <code> <message>` output for all error paths, and eliminate silent clamping by returning explicit errors for out-of-range values.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust `core::fmt` / `alloc::format` | Rust 1.88 | Deterministic numeric formatting | Explicit precision avoids locale/default variance |
| `heapless` | 0.8.0 | Fixed-capacity strings for UART messages | Matches embedded memory constraints |
| `embassy-time` | 0.5.0 | Timekeeping for elapsed seconds | Stable time source for CSV time column |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `embassy-sync` | 0.6.1 | Channels for output delivery | Queueing formatted lines before UART output |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `format!` with explicit precision | Custom float-to-string (e.g., ryu) | Adds complexity and no clear benefit for fixed precision CSV |

**Installation:**
```bash
# already in Cargo.toml
```

## Architecture Patterns

### Recommended Project Structure
```
src/
├── output/           # Formatting logic (CSV, read responses)
├── hardware/uart/    # UART IO and CRLF termination
└── input/            # Command parsing and error classification
```

### Pattern 1: Deterministic CSV Formatting
**What:** Format all numeric fields with explicit precision and fixed column order.
**When to use:** All READ responses and streaming CSV lines.
**Example:**
```rust
// Source: src/output/artisan.rs
format!("{},{:.1},{:.1},{:.2},{:.1}", time_str, et, bt, ror, gas)
```

### Pattern 2: CRLF at UART Boundary
**What:** Append `\r\n` only in UART output tasks, not in formatter functions.
**When to use:** Any data sent over UART (normal responses and errors).
**Example:**
```rust
// Source: src/hardware/uart/tasks.rs
let mut bytes = response.as_bytes().to_vec();
bytes.extend_from_slice(b"\r\n");
```

### Pattern 3: Parse → Validate → Enqueue
**What:** Reject invalid commands early and only enqueue validated commands.
**When to use:** UART command parsing.
**Example:**
```rust
// Source: src/hardware/uart/tasks.rs
core::str::from_utf8(command)
    .map_err(|_| ParseError::InvalidValue)
    .and_then(crate::input::parse_artisan_command)
```

### Anti-Patterns to Avoid
- **Silent clamping:** `clamp()` in formatter hides invalid inputs and violates FMT-02.
- **Mixed error payloads:** adding debug text to some `ERR` lines breaks schema.
- **Implicit float formatting:** default formatting without precision can change output.

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Numeric formatting | Custom float serializer | `format!` with explicit precision | Stable rounding and dot separators |
| Line termination | Manual caller-side `\r\n` logic | UART task append | Keeps termination consistent |
| Error codes | Ad-hoc error strings | Centralized `ERR <code> <message>` | Stable parsing for host tools |

**Key insight:** Determinism requires a single authority for precision, ordering, and termination. Distributing formatting rules across handlers introduces drift.

## Common Pitfalls

### Pitfall 1: Silent clamping in READ
**What goes wrong:** `format_read_response` clamps power/fan, masking invalid inputs.
**Why it happens:** Convenience clamp is used to keep values in range.
**How to avoid:** Validate at command parse or handler; return `ERR out_of_range` instead.
**Warning signs:** Output always within 0–100 even when inputs were invalid.

### Pitfall 2: Inconsistent error schema
**What goes wrong:** Parse errors emit `ERR <code>`, handler errors emit `ERR handler_failed: <debug>`.
**Why it happens:** Error formatting is spread across different modules.
**How to avoid:** Centralize error formatting and keep a stable `code/message` structure.
**Warning signs:** Error lines vary in delimiter or include debug payloads.

### Pitfall 3: Unstable time formatting
**What goes wrong:** Time column changes precision or rounding across runs.
**Why it happens:** Formatting uses milliseconds without fixed precision.
**How to avoid:** Always use `format!("{}.{:02}", secs, ms/10)` and avoid locale/defaults.
**Warning signs:** Time column length varies, or decimals show more than two digits.

## Code Examples

### READ response formatting
```rust
// Source: src/output/artisan.rs
format!("{:.1},{:.1},{:.1},{:.1}", status.env_temp, status.bean_temp, power, fan)
```

### Parse error emission
```rust
// Source: src/hardware/uart/tasks.rs
let _ = message.push_str("ERR ");
let _ = match error {
    ParseError::UnknownCommand => message.push_str("unknown_command"),
    ParseError::InvalidValue => message.push_str("invalid_value"),
    ParseError::OutOfRange => message.push_str("out_of_range"),
    ParseError::EmptyCommand => message.push_str("empty_command"),
};
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Ad-hoc formatting per handler | Central formatter functions | Present in repo | Enables deterministic CSV output |
| Mixed error payloads | Target: `ERR <code> <message>` schema | Planned | Parseable and stable error output |

**Deprecated/outdated:**
- Silent clamping in formatters: conflicts with FMT-02 and should be removed.

## Open Questions

1. **Should `empty_command` be part of the official reason codes?**
   - What we know: Parse errors currently emit `empty_command`, but prior decision lists only `unknown_command`, `invalid_value`, `out_of_range`, `handler_failed`.
   - What's unclear: Whether to keep `empty_command` or map it to `invalid_value`.
   - Recommendation: Decide and document a canonical set of reason codes before refactoring.

2. **Should handler errors include a message field?**
   - What we know: `ERR handler_failed: <debug>` currently violates a strict schema.
   - What's unclear: Whether message content should be fixed tokens or human-readable detail.
   - Recommendation: Use `ERR handler_failed <message>` with a bounded, parseable token set.

## Sources

### Primary (HIGH confidence)
- `src/output/artisan.rs` - CSV formatting, READ response, precision handling
- `src/hardware/uart/tasks.rs` - CRLF termination, parse error emission
- `src/input/parser.rs` - Parse errors and validation rules
- `tests/command_errors.rs` - Expected error output behavior in tests
- `Cargo.toml` - Library versions

### Secondary (MEDIUM confidence)
- None (no external docs consulted)

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Versions and usage verified in repo
- Architecture: HIGH - Patterns documented in current code
- Pitfalls: MEDIUM - Derived from code review and phase requirements

**Research date:** 2026-02-04
**Valid until:** 2026-03-06
