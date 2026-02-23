# Phase 62: Documentation Cleanup - Research

**Researched:** 2026-02-20
**Domain:** Rust Documentation, Markdown linting, Link checking
**Confidence:** HIGH

## Summary

This phase focuses on removing outdated information and aligning documentation with the current Rust codebase state. Since the project relies heavily on `Cargo` and Rust toolchains, standard Rust ecosystem tools and global documentation linters should be used to enforce code-to-doc synchrony.

**Primary recommendation:** Use `cargo doc --document-private-items` to catch inline documentation errors, and apply standard CI-friendly markdown linters (`lychee`, `markdownlint-cli`) to ensure all external `.md` files are current and unbroken.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `cargo doc` | N/A | Rust inline documentation generation and checking. | Built-in to Rust ecosystem. Generates current codebase docs. |
| `lychee` | v0.14+ | Fast dead link checking for Markdown files. | Rust-based, highly parallel, standard in modern CI/CD. |
| `markdownlint-cli` | v0.39+ | Standardizing markdown style and structure. | Catch malformed or unmaintained docs quickly. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `cargo-deadlinks` | v0.8+ | Checking dead links in generated `cargo doc` HTML. | Validating intra-doc `[link]` references in Rust. |
| `cspell` | v8.0+ | Spell checking codebase and documentation. | Avoiding typos and out-of-date terminology. |

## Architecture Patterns

### Recommended Project Structure
For LibreRoaster documentation updates:
```
/
├── README.md        # Update with top-level architecture changes
├── src/             # Update inline `///` Rustdoc comments
└── internalDoc/     # Clean up architectural decision records or internal notes
```

### Pattern 1: CI Documentation Verification
**What:** Enforcing documentation integrity via automated scripts.
**When to use:** On every PR to prevent docs from diverging.
**Example:**
```yaml
# Source: GitHub Actions standard patterns
- name: Run lychee link checker
  uses: lycheeverse/lychee-action@v1.9
  with:
    args: "**/*.md **/*.rs"
```

### Anti-Patterns to Avoid
- **Duplicating Logic in Markdown:** Describing exact function parameters in `.md` files (which easily go out of sync).
  *Instead:* Use `cargo doc` and link to the generated HTML from markdown, or use `rustdoc` includes.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Dead link checking | Custom Python/Bash regex scripts | `lychee` | Regex cannot parse markdown properly; `lychee` handles rate limits, caching, and intra-document anchors. |
| Markdown formatting | Manual inspection rules | `markdownlint` | Standard rulesets prevent endless styling debates and catch syntax issues. |

## Common Pitfalls

### Pitfall 1: Intra-doc Links Breaking
**What goes wrong:** Renaming a Rust struct or module breaks `/// [struct@MyStruct]` references in doc comments.
**Why it happens:** Standard `cargo test` doesn't verify doc links by default unless explicitly run.
**How to avoid:** Run `cargo rustdoc -- -D warnings` in CI.

### Pitfall 2: Outdated README examples
**What goes wrong:** Code snippets in `README.md` no longer compile due to API changes.
**Why it happens:** Markdown snippets aren't type-checked.
**How to avoid:** Use `rustdoc` to test markdown snippets or standard tools like `docmatic` to compile markdown code blocks.

## Code Examples

### Enforcing rustdoc links
```bash
# Fail on any missing doc links or malformed markdown inside Rust files
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual review of `README.md` | Automated link checking (`lychee`) | ~2021 | CI catches broken docs immediately. |
| Unchecked markdown code blocks | Checked doc-tests (`cargo test --doc`) | ~2018 | Rust codebase ensures examples always compile. |

## Open Questions

1. **Specific Requirements (CLN-01, CLN-02, CLN-03)**
   - What we know: The phase defines these as requirements.
   - What's unclear: The exact text of these requirements is not in the provided context.
   - Recommendation: The planner should map standard clean-up steps (markdown linting, Rustdoc updates, README syncing) to these specific ID codes based on project management specs.

## Sources

### Primary (HIGH confidence)
- `rustdoc` Official Book - Enforcing documentation links and warnings in Rust.
- `cargo doc` Official Documentation.

### Secondary (MEDIUM confidence)
- `lychee` documentation for dead-link checking in Rust repositories.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Rust standard tooling is strictly defined and widely adopted.
- Architecture: HIGH - Applies universally to `cargo` projects.
- Pitfalls: HIGH - Outdated READMEs and broken intra-doc links are the most common Rust doc issues.

**Research date:** 2026-02-20
**Valid until:** 2026-08-20 (6 months for standard Rust tooling stability)
