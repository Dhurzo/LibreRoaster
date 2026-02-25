# Phase 44: Protocol Framing Contract - Research

**Researched:** 2026-02-17
**Domain:** Embedded Rust serial protocol framing (Artisan READ response)
**Confidence:** HIGH

## Summary

This research reviewed the current LibreRoaster code paths that format and transmit READ responses, focusing on CSV framing and terminator handling across USB CDC and UART. The codebase already provides a formatter (`ArtisanFormatter::format_read_response_full`) that emits a 4-value CSV without a terminator, while multiple output tasks append CRLF at different boundaries. The application currently spawns `usb_writer_task`, `uart_writer_task`, and `dual_output_task` simultaneously, which can result in duplicate terminators and inconsistent output routing.

The standard approach in this codebase is to keep formatters pure (payload only) and apply transport framing (CRLF) at a single output boundary. To meet the phase contract, the plan should centralize CRLF appending in one output path (preferably the multiplexer-aware boundary) and ensure all READ responses are formatted as a strict 4-field CSV with one-decimal precision and placeholder handling (0.0 for invalid/missing values).

**Primary recommendation:** Keep READ formatting terminator-free and enforce a single CRLF append at one output boundary shared by USB CDC and UART.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust `format!` (core/alloc) | Rust 1.88 | CSV string formatting | Used throughout formatter for numeric precision and CSV layout (`format!`) |
| `heapless` | 0.8.0 | Fixed-capacity strings/buffers for channel output | Used for `String<128>` payloads in output channel and UART/USB tasks |
| `embassy-sync` | 0.6.1 | Channel/pipe synchronization | Output channel and UART pipe are Embassy primitives |
| `embassy-time` | 0.5.0 | Task scheduling and timing | Output tasks and control loop use timers |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `log` | 0.4.27 | Protocol logging and diagnostics | Log RX/TX without mutating payload |
| `critical-section` | 1.2.0 | Shared access to multiplexer/output state | Required for output routing and channel selection |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Manual CSV concatenation in call sites | `ArtisanFormatter::format_read_response_full` | Formatter keeps strict CSV layout and numeric precision centralized |

**Installation:**
```bash
cargo build
```

## Architecture Patterns

### Recommended Project Structure
```
src/
├── output/          # CSV formatters (payload only, no terminators)
├── application/     # Control loop and output routing tasks
├── hardware/        # USB CDC / UART tasks and drivers
└── input/           # Command parsing and multiplexer
```

### Pattern 1: Payload-Only Formatter + Central Terminator Boundary
**What:** Format READ responses as a single CSV line with no CRLF; append CRLF only once at the transport boundary.
**When to use:** Any READ/ERR/ACK responses that will be written to USB CDC or UART.
**Example:**
```rust
// Source: src/output/artisan.rs
pub fn format_read_response_full(status: &SystemStatus) -> String {
    format!(
        "{:.1},{:.1},{:.1},{:.1}",
        status.env_temp,
        status.bean_temp,
        status.ssr_output,
        status.fan_output
    )
}
```

### Pattern 2: Single Output Boundary Adds CRLF
**What:** Only one output task should append the CRLF terminator before writing bytes to the transport.
**When to use:** The centralized output boundary should be the only place that adds `\r\n` for all response lines.
**Example:**
```rust
// Source: src/application/tasks.rs
let mut bytes = data.as_bytes().to_vec();
bytes.extend_from_slice(b"\r\n");
```

### Anti-Patterns to Avoid
- **Embedded terminator in formatter output:** causes double terminators when transport also appends CRLF.
- **Multiple writers appending CRLF:** `usb_writer_task`, `uart_writer_task`, and `dual_output_task` all append CRLF today, which can duplicate terminators or split output routing.

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| READ CSV formatting | Manual concatenation at call sites | `ArtisanFormatter::format_read_response_full` | Centralizes CSV order and numeric precision |
| Output routing + terminator | Ad-hoc CRLF appends in multiple tasks | Single boundary task (multiplexer-aware) | Prevents double terminators and inconsistent routing |

**Key insight:** The formatter should never know about transport framing; transport should never recompute CSV layout.

## Common Pitfalls

### Pitfall 1: Double terminator on READ responses
**What goes wrong:** Responses end with `\r\n\r\n` or get two writes because multiple tasks append CRLF.
**Why it happens:** USB/UART writers and `dual_output_task` all append CRLF today, while sharing the same output channel.
**How to avoid:** Choose a single output boundary to append CRLF and remove/disable other terminator appends.
**Warning signs:** Artisan sees blank lines between responses or logs show duplicated CRLF bytes.

### Pitfall 2: Wrong CSV shape or spacing
**What goes wrong:** Extra fields, spaces after commas, or mismatched ordering break the READ contract.
**Why it happens:** Mixing old 7-field output or manually constructed CSV strings.
**How to avoid:** Use `format_read_response_full` and keep formatting strict (`{:.1}`), no trailing spaces.
**Warning signs:** Tests assert parts length != 4 or Artisan rejects responses.

### Pitfall 3: Invalid sensor values leak to output
**What goes wrong:** NaN/inf or unavailable sensor values propagate into CSV fields.
**Why it happens:** SystemStatus fields are raw f32 without validation.
**How to avoid:** Clamp invalid values to `0.0` before formatting; fill placeholders per decisions.
**Warning signs:** CSV contains `nan`, `inf`, or parsing errors downstream.

## Code Examples

Verified patterns from official sources:

### READ formatting without terminator
```rust
// Source: src/output/artisan.rs
pub fn format_read_response_full(status: &SystemStatus) -> String {
    format!(
        "{:.1},{:.1},{:.1},{:.1}",
        status.env_temp,
        status.bean_temp,
        status.ssr_output,
        status.fan_output
    )
}
```

### Output boundary appends CRLF
```rust
// Source: src/application/tasks.rs
let mut bytes = data.as_bytes().to_vec();
bytes.extend_from_slice(b"\r\n");
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| CRLF appended in multiple writers | Single CRLF append at one output boundary | v2.5 (planned) | Prevents double terminators and keeps CSV payload clean |

**Deprecated/outdated:**
- Embedding CRLF in formatter output: duplicates terminators when transport also appends CRLF.

## Open Questions

1. **Which output task is the canonical boundary?**
   - What we know: `usb_writer_task`, `uart_writer_task`, and `dual_output_task` are all spawned and each appends CRLF.
   - What's unclear: Which task is intended to be the single output boundary moving forward.
   - Recommendation: Use the multiplexer-aware boundary (`dual_output_task`) as the single terminator append point and disable other CRLF appends.

## Sources

### Primary (HIGH confidence)
- `src/output/artisan.rs` - READ response formatting and CSV precision
- `src/application/tasks.rs` - output channel handling and CRLF append
- `src/hardware/usb_cdc/tasks.rs` - USB writer CRLF append
- `src/hardware/uart/tasks.rs` - UART writer/send_response CRLF append
- `src/application/app_builder.rs` - task spawn wiring
- `Cargo.toml` - dependency versions

### Secondary (MEDIUM confidence)
- None

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - versions from `Cargo.toml` and usage in code
- Architecture: HIGH - output/formatter/tasks are defined in repo
- Pitfalls: HIGH - inferred from current output wiring and formatter behavior

**Research date:** 2026-02-17
**Valid until:** 2026-03-19
