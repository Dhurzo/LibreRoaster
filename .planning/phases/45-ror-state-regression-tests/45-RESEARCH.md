# Phase 45: ROR State + Regression Tests - Research

**Researched:** 2026-02-17
**Domain:** Embedded Rust firmware output/state tracking + protocol regression tests
**Confidence:** HIGH

## Summary

This phase focuses on correcting and freezing Rate-of-Rise (ROR) state behavior during roast sessions and adding regression tests that lock in READ terminator framing plus ROR update timing. The codebase already has stateful ROR support in the `MutableArtisanFormatter`, continuous output controlled by `OutputController`, and CRLF framing centralized in `dual_output_task`. The gap is aligning ROR state lifecycle with roast session start/stop decisions and covering the framing + ROR expectations in tests.

The standard approach is to keep ROR state in the formatter used for continuous output, reset it when roast sessions start/end, and emit CSV/READ responses without embedding terminators (CRLF framing is applied once by the output task). Tests should simulate ROR sequences and output framing behavior using existing integration test patterns under `tests/` with `--features test`.

**Primary recommendation:** Keep ROR state in the continuous-output formatter, reset on START/STOP, and add targeted tests for CRLF framing and ROR timing using existing test harness patterns.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust toolchain | 1.88 | Compiler/runtime for firmware and tests | Project toolchain pin in `Cargo.toml` |
| `embassy-time` | 0.5.0 | Time/Instant for output timing | Used for elapsed time and control loop timing |
| `heapless` | 0.8.0 | Fixed-capacity collections and strings | Used for output buffers and parsing |
| `embassy-sync` | 0.6.1 | Channels for inter-task output | Output channel wiring for UART/USB |
| `critical-section` | 1.2.0 | Safe shared-state access | Used in multiplexer/channel routing |
| `log` | 0.4.27 | Logging in firmware/tests | Standard logging façade in repo |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `embedded-io` | 0.7.1 | I/O traits for drivers | UART/USB driver write/read abstractions |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `MutableArtisanFormatter` for ROR state | `ArtisanFormatter` (stateless) | Stateless formatter cannot track ROR across samples or reset on session boundaries |

**Installation:**
```bash
cargo test --features test
```

## Architecture Patterns

### Recommended Project Structure
```
src/
├── application/        # Task orchestration (control loop + output framing)
├── control/            # Roast session state + START/STOP logic
├── output/             # Artisan CSV/READ formatting and ROR tracking
└── hardware/           # UART/USB parsing with CR terminator
tests/                  # Integration/regression tests
```

### Pattern 1: Session-Bound ROR State
**What:** Keep ROR tracking state inside the formatter that emits continuous Artisan CSV, and reset that state when the roast session starts or stops. ROR should remain 0 until two samples have been collected, then become non-zero on the first BT change after the second sample.
**When to use:** Continuous output during roast sessions (`START` / `STOP` flow).
**Example:**
```rust
// Source: src/output/artisan.rs
pub struct MutableArtisanFormatter {
    start_time: Instant,
    last_bt: f32,
    bt_history: Vec<f32>,
}

impl MutableArtisanFormatter {
    pub fn reset(&mut self) {
        self.start_time = Instant::now();
        self.last_bt = 0.0;
        self.bt_history.clear();
    }
}
```

### Pattern 2: Single-Point CRLF Framing
**What:** Keep all protocol responses as raw CSV/ERR lines without terminators and append `\r\n` only once in the output task that writes to UART/USB.
**When to use:** Any outbound line written to the physical serial interfaces.
**Example:**
```rust
// Source: src/application/tasks.rs
let mut bytes = data.as_bytes().to_vec();
bytes.extend_from_slice(b"\r\n");
```

### Pattern 3: READ Response Format Is 4-Value CSV
**What:** READ replies are 4 values (ET,BT,HEATER,FAN) with one-decimal precision.
**When to use:** On `READ` command processing.
**Example:**
```rust
// Source: src/output/artisan.rs
format!("{:.1},{:.1},{:.1},{:.1}", et, bt, heater, fan)
```

### Anti-Patterns to Avoid
- **Embedding CRLF in formatters:** Causes double-terminators because output task already appends `\r\n`.
- **Resetting ROR on every sample:** Breaks the requirement that ROR becomes non-zero only after the second sample and first BT change.
- **Using stateless formatter for ROR:** Loses historical BT context and cannot honor session boundaries.

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Serial line framing | Per-caller `\r\n` concatenation | Central framing in `dual_output_task` | Prevents double terminators and keeps protocol consistent |
| READ CSV formatting | Custom per-test string building | `ArtisanFormatter::format_read_response_full` | Enforces 4-value, one-decimal precision consistently |

**Key insight:** The protocol already centralizes framing and formatting; duplicating these in callers or tests causes subtle regressions.

## Common Pitfalls

### Pitfall 1: ROR Carryover Across Roasts
**What goes wrong:** ROR starts non-zero at the beginning of a new roast because prior BT history remains.
**Why it happens:** `MutableArtisanFormatter` state is not reset when sessions stop or start.
**How to avoid:** Reset formatter state on roast session START and STOP events.
**Warning signs:** First ROR value in a new session is non-zero without two fresh samples.

### Pitfall 2: ROR Updates Too Early
**What goes wrong:** ROR is non-zero on the first or second sample.
**Why it happens:** ROR calculation uses `last_bt` immediately without enforcing the two-sample warmup.
**How to avoid:** Track sample count; keep ROR at 0 until after the second sample and a BT change.
**Warning signs:** ROR non-zero after only one or two readings.

### Pitfall 3: READ Terminator Regression
**What goes wrong:** Output is missing `\r\n` or includes it twice.
**Why it happens:** Formatting or tests add terminators directly instead of relying on output framing.
**How to avoid:** Test framing at the output task boundary; keep formatters terminator-free.
**Warning signs:** Artisan shows merged or split lines for READ responses.

## Code Examples

Verified patterns from project sources:

### ROR History Calculation
```rust
// Source: src/output/artisan.rs
fn compute_ror_from_history(history: &[f32]) -> f32 {
    if history.len() < 2 {
        0.0
    } else {
        let samples = history.len();
        let first_bt = history[0];
        let last_bt = history[samples - 1];
        (last_bt - first_bt) / (samples as f32 - 1.0)
    }
}
```

### CRLF Output Framing
```rust
// Source: src/application/tasks.rs
let mut bytes = data.as_bytes().to_vec();
bytes.extend_from_slice(b"\r\n");
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Stateless delta-only ROR in `ArtisanFormatter::format` | Stateful `MutableArtisanFormatter` used in `control_loop_task` | Unknown (present in repo) | Enables multi-sample ROR tracking but still needs session-bound resets |

**Deprecated/outdated:**
- None explicitly marked in repo for this phase.

## Open Questions

1. **None identified for this phase**
   - What we know: ROR timing/reset rules are locked by context decisions.
   - What's unclear: No additional open decisions surfaced in current sources.
   - Recommendation: Proceed with the defined timing/reset behavior and add regression tests.

## Sources

### Primary (HIGH confidence)
- `Cargo.toml` - dependency versions and toolchain pinning
- `src/output/artisan.rs` - ROR tracking, READ formatting
- `src/application/tasks.rs` - output framing with CRLF
- `src/control/handlers.rs` - START/STOP enabling/disabling continuous output
- `src/control/roaster_refactored.rs` - command handling for START/STOP
- `src/hardware/uart/tasks.rs` - CR-terminated command parsing
- `src/hardware/usb_cdc/tasks.rs` - CR-terminated command parsing
- `tests/artisan_integration_test.rs` - existing ROR and READ flow test patterns
- `tests/usb_cdc_tests.rs` - terminator parsing and command framing tests
- `tests/mock_uart_integration.rs` - READ response output flow in tests

### Secondary (MEDIUM confidence)
- None

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - versions and usage pinned in `Cargo.toml`
- Architecture: HIGH - patterns derived from current task wiring and formatter usage
- Pitfalls: HIGH - inferred from existing stateful formatter and output framing

**Research date:** 2026-02-17
**Valid until:** 2026-03-19
