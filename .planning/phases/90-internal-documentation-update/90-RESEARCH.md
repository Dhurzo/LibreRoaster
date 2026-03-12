# Phase 90: Internal Documentation Update - Research

**Researched:** 2026-03-11
**Domain:** Technical documentation synchronization for embedded Rust projects
**Confidence:** HIGH (current state), MEDIUM (recommendations)

## Summary

This research investigates how to effectively update internal documentation (ARCHITECTURE.md, PROTOCOL.md, HARDWARE.md, DEVELOPMENT.md, INSTRUMENTATION_README.MD) to match the current codebase. The existing documentation is already recent (updated 2026-03-10) and appears accurate based on spot checks. However, systematic verification is needed to ensure all references reflect recent architectural changes (18-field STATUS command, ManualCommandPolicy pattern, etc.).

The standard approach for Rust embedded projects is to rely on `cargo doc` for API documentation, augment with high‑level markdown files, and use `rustdoc` intra‑doc links to keep documentation and code synchronized. The update process should be manual but guided by automated checks (e.g., `cargo test --doc`).

**Primary recommendation:** Perform a systematic file‑by‑file review, update any outdated sections, rename `hardware.md` → `HARDWARE.md`, create missing `DEVELOPMENT.md` by merging `FLASH_GUIDE.md` with build/test/debug instructions, and verify all code references point to correct locations.

## Standard Stack

The established libraries/tools for Rust project documentation:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `cargo doc` | (bundled) | Generate API documentation from doc comments | Official Rust tool, integrates with `#[doc]` attributes and intra‑doc links |
| `rustdoc` | (bundled) | Render documentation, test code examples | Part of Rust distribution, supports `--test` to validate examples |
| `mdbook` | 0.4.x | Create book‑style documentation sites | Used by Rust‑Embedded project and many Rust libraries for user guides |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `rustdoc-json` | (bundled) | Extract documentation as JSON for tooling | When building custom documentation pipelines |
| `cargo‑test‑docs` | (cargo) | Run documentation tests only | Isolate doc‑test execution |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `cargo doc` | `doxygen` | Doxygen supports multiple languages but lacks Rust‑specific features and intra‑doc links |
| `mdbook` | `sphinx` | Sphinx is more mature but requires Python and has less native Rust integration |
| Manual markdown | Generated API docs | Generated docs ensure API accuracy but lack high‑level narrative; combine both |

**Installation:**
```bash
# mdbook (optional, for future documentation site)
cargo install mdbook
```

## Architecture Patterns

### Recommended Project Structure
```
internalDoc/
├── ARCHITECTURE.md          # High‑level system design, task structure, async model
├── PROTOCOL.md              # Artisan command reference, message formats
├── HARDWARE.md              # Pinout, thermocouple wiring, hardware specs (rename from hardware.md)
├── DEVELOPMENT.md           # Build, flash, test, debugging guides (new)
├── INSTRUMENTATION_README.MD # Watchdog, guard, regression telemetry
└── FLASH_GUIDE.md           # Detailed flashing instructions (may be merged into DEVELOPMENT.md)
```

### Pattern 1: Code‑Proximate Documentation
**What:** Place module‑level documentation inside source files using `//!` or `///` doc comments.
**When to use:** For API‑level documentation that should stay synchronized with code changes.
**Example:**
```rust
//! # Temperature Handler Module
//!
//! This module manages thermocouple readings via MAX31856 sensors.
//! See [ARCHITECTURE.md] for the overall data flow.

use crate::config::SystemStatus;

/// Reads temperature from a MAX31856 sensor.
///
/// Returns `Ok(temperature)` or `Err(HardwareError)`.
/// See also [`hardware::max31856`].
pub fn read_temperature(cs_pin: u8) -> Result<f32, HardwareError> {
    // ...
}
```

### Pattern 2: Intra‑Doc Links
**What:** Use `[` `]` syntax to link to other Rust items, making documentation navigable and verifiable.
**When to use:** Whenever referencing a type, function, or constant defined elsewhere in the codebase.
**Example:**
```rust
/// The [`SystemStatus`] struct contains all telemetry fields.
/// Use [`ArtisanFormatter::format_status_response`] to produce the 18‑field CSV.
```

### Anti-Patterns to Avoid
- **Duplicate information:** Copying function signatures or constants into markdown files—reference the source instead.
- **Untested code examples:** Code blocks in markdown that may become outdated; use `rustdoc --test` to validate them.
- **Hidden assumptions:** Documenting behavior that is not enforced by the code (e.g., “UNITS command converts temperatures” when it does not).

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Checking doc‑code consistency | Custom script that parses Rust and markdown | `cargo test --doc` and manual review | Rustdoc already validates intra‑doc links and runs examples; custom scripts are fragile. |
| Generating pinout tables | Manually updating markdown table | Reference `constants.rs` directly in documentation | Pin assignments are defined in one place; duplication invites errors. |
| Creating a documentation website | Writing HTML templates | `mdbook` or `cargo doc` | Static site generators are battle‑tested and support search, themes, and versioning. |

**Key insight:** The Rust ecosystem provides robust documentation tooling; custom solutions are rarely justified and introduce maintenance burden.

## Common Pitfalls

### Pitfall 1: Stale Code References
**What goes wrong:** Documentation points to a file:line that no longer exists or has changed.
**Why it happens:** Code is refactored, but the documentation is not updated.
**How to avoid:** Use intra‑doc links (`[` `]`) instead of hard‑coded line numbers where possible. For markdown, reference function names rather than line numbers.
**Warning signs:** CI fails because `cargo test --doc` reports broken links; manual review finds mismatched line numbers.

### Pitfall 2: Inconsistent Naming
**What goes wrong:** Documentation uses “SSR control pin” while code uses `SSR_CONTROL_PIN`.
**Why it happens:** Different authors, no enforced naming convention.
**How to avoid:** Adopt a single naming style (snake_case for Rust identifiers, natural language for prose) and verify consistency with a simple grep.
**Warning signs:** Multiple terms for the same concept appear in search results.

### Pitfall 3: Missing DEVELOPMENT.md
**What goes wrong:** New contributors cannot build, flash, or test the firmware.
**Why it happens:** Development instructions are scattered across README.md, FLASH_GUIDE.md, and ad‑‑hoc scripts.
**How to avoid:** Create a single DEVELOPMENT.md that consolidates all development workflows.
**Warning signs:** README.md contains build instructions but no flashing or debugging details; FLASH_GUIDE.md exists but is separate.

### Pitfall 4: Language Inconsistency
**What goes wrong:** HARDWARE.md is written in Spanish while other documents are in English.
**Why it happens:** Historical contributions in different languages.
**How to avoid:** Standardize on English for all technical documentation (or decide on a single language). Translate non‑English sections.
**Warning signs:** Mixed‑language documents; non‑English terms in otherwise English prose.

## Code Examples

Verified patterns from official sources:

### Linking to Constants
```rust
// Source: rustdoc documentation
/// The heater is controlled via [`SSR_CONTROL_PIN`] (GPIO10).
/// See [`constants`] module for all pin assignments.
```

### Testing Documentation Examples
```rust
/// ```
/// use libreroaster::config::constants::SSR_CONTROL_PIN;
/// assert_eq!(SSR_CONTROL_PIN, 10);
/// ```
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual line‑number references | Intra‑doc links (`[` `]`) | Rust 1.48 | References are checked by `rustdoc`, remain valid across refactoring. |
| Separate API and user docs | Unified `cargo doc` with module‑level explanations | Rust 2018 | Single source of truth for API documentation. |
| Static HTML generated by hand | `mdbook` for narrative documentation | Rust‑Embedded book 2020 | Easier to maintain, supports versioning, search, and themes. |

**Deprecated/outdated:**
- **Hard‑coded line numbers in markdown:** Use function names or intra‑doc links.
- **Separate `docs/` and `internalDoc/` directories:** Consider consolidating or establishing clear boundaries (internal vs. external).

## Open Questions

Things that couldn't be fully resolved:

1. **Should `FLASH_GUIDE.md` be merged into `DEVELOPMENT.md`?**
   - What we know: `FLASH_GUIDE.md` exists and contains detailed flashing instructions.
   - What's unclear: Whether the requirement “DEVELOPMENT.md has up‑to‑date build, flash, test, and debugging guides” implies a single file.
   - Recommendation: Merge `FLASH_GUIDE.md` into `DEVELOPMENT.md` as a “Flashing” section, keeping the original file as a symlink or redirect for existing links.

2. **Is `hardware.md` (lowercase) acceptable, or must it be `HARDWARE.md`?**
   - What we know: The requirement lists `HARDWARE.md` (uppercase). The current file is `hardware.md`.
   - What's unclear: Whether the case sensitivity matters for the verification step.
   - Recommendation: Rename `hardware.md` → `HARDWARE.md` to match the requirement exactly.

3. **What about other files in `internalDoc/` (e.g., `CODE_QUALITY_ISSUES.md`)?**
   - What we know: The requirement only lists five specific files.
   - What's unclear: Whether additional internal documentation should also be updated.
   - Recommendation: Focus only on the five listed files; other files are out of scope for this phase.

## Sources

### Primary (HIGH confidence)
- `./internalDoc/ARCHITECTURE.md` – reviewed for architecture accuracy
- `./internalDoc/PROTOCOL.md` – verified STATUS command field order matches `artisan.rs`
- `./internalDoc/hardware.md` – compared pinout with `src/config/constants.rs`
- `./internalDoc/INSTRUMENTATION_README.MD` – checked STATUS payload description
- `./src/output/artisan.rs` – confirmed `format_status_response` produces 18 fields

### Secondary (MEDIUM confidence)
- Rust documentation practices (based on Rust embedded book and `cargo doc` official documentation)

### Tertiary (LOW confidence)
- General best practices for technical documentation synchronization (no current web search verification)

## Metadata

**Confidence breakdown:**
- Standard stack: MEDIUM – Rust tooling is well‑established, but specific version recommendations are based on general knowledge.
- Architecture: HIGH – current documentation structure is known and matches the codebase.
- Pitfalls: MEDIUM – identified pitfalls are common in documentation projects, but may not all apply.

**Research date:** 2026-03-11
**Valid until:** 2026-04-11 (30 days – documentation practices evolve slowly)
