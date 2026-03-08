# Phase 87: Wire Modernization to Quality Policy (P81 <-> P83) - Research

**Researched:** 2026-03-08
**Domain:** Rust Code Quality, CI/CD Automation
**Confidence:** MEDIUM

## Summary

This research focuses on integrating Rust's core quality tools (`cargo fmt`, `clippy`, `cargo fix`) into automated modernization scripts to ensure that any code changes adhere to a defined quality baseline and do not bypass policy. The primary goal is to establish an explicit pass/fail policy for formatting, linting, and tests. This involves configuring these tools for CI/CD pipelines and defining how automated fixes (`cargo fix`) can be applied without regressing code quality.

**Primary recommendation:** Integrate `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` as mandatory gates in CI, and define a controlled workflow for applying `cargo fix` that re-runs these checks.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `rustfmt` | Latest (via `rustup`) | Code formatting | Official Rust formatter, highly configurable. Ensures consistent code style. |
| `clippy` | Latest (via `rustup`) | Linting and static analysis | Official Rust linter. Catches common mistakes, suggests idiomatic Rust. |
| `cargo-fix` | Latest (via `cargo`) | Automated code modifications | Applies suggestions from `rustc` and `clippy` automatically. Essential for large-scale modernization. |
| `cargo test` | Latest (via `cargo`) | Running unit/integration tests | Standard way to execute tests in Rust projects, critical for verifying functionality. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `just` (or similar task runner) | N/A | Scripting complex commands | To encapsulate common development workflows (e.g., `just check`, `just fix`). |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `rustfmt` | Custom linting (e.g. `regex` based) | `rustfmt` is canonical and widely accepted; custom solutions are high maintenance and error-prone. |
| `clippy` | Custom static analysis | `clippy` covers a vast range of common issues; custom analysis requires significant effort and knowledge. |

**Installation:**
All core tools are typically installed as part of `rustup` or `cargo`.
\`\`\`bash
# Ensure rustfmt and clippy components are installed
rustup component add rustfmt clippy
\`\`\`

## Architecture Patterns

### Recommended Project Structure
\`\`\`
.
├── .cargo/                 # Cargo-related configuration
│   └── config.toml         # Global cargo settings, can include clippy defaults
├── src/                    # Source code
├── Cargo.toml              # Project dependencies and metadata
├── rustfmt.toml            # rustfmt configuration
├── clippy.toml             # Custom clippy configuration (less common, usually via attributes)
└── justfile                # (Optional) Task runner definitions for common commands
\`\`\`

### Pattern 1: CI Pipeline Quality Gates
**What:** Enforcing code quality checks as mandatory steps in the Continuous Integration (CI) pipeline. This ensures that no code is merged that violates the defined quality baseline.
**When to use:** On every pull request and merge to the main branch.
**Example:** (Conceptual GitHub Actions workflow)
\`\`\`yaml
# Source: Internal Knowledge (Common CI/CD patterns)
name: Rust Quality Gates

on: [push, pull_request]

jobs:
  check-quality:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v4
    - name: Install Rust
      uses: dtolnay/rust-toolchain@stable
      with:
        toolchain: stable
        components: rustfmt, clippy
    - name: Run cargo fmt --check
      run: cargo fmt -- --check
    - name: Run cargo clippy -- -D warnings
      run: cargo clippy --workspace --all-features -- -D warnings
    - name: Run cargo test
      run: cargo test --workspace --all-features
\`\`\`

### Pattern 2: Automated Modernization with Quality Re-check
**What:** Applying automated code fixes (`cargo fix`) and immediately re-validating against the quality gates to ensure no policy bypass. This is crucial for mass code changes.
**When to use:** During dedicated modernization tasks, e.g., in a temporary branch, to update code to new idioms or fix widespread lints.
**Example:** (Shell script for modernization workflow)
\`\`\`bash
# Source: Internal Knowledge (Common automation patterns)
#!/bin/bash
set -euxo pipefail

# Ensure we're on a clean branch for fixes
git checkout -b feature/automated-fixes-"$(date +%Y%m%d%H%M%S)"

# 1. Apply cargo fix
# --workspace: apply to all crates
# --all-features: ensure all features are considered for fixes
# --allow-dirty --allow-staged: necessary if working in a dirty repo, but generally prefer clean slate
echo "Applying cargo fix..."
cargo fix --workspace --all-features --allow-dirty --allow-staged

# 2. Re-format code after fixes (fixes might introduce formatting changes)
echo "Running cargo fmt..."
cargo fmt --all

# 3. Check if formatting is now clean
echo "Checking formatting after fix..."
cargo fmt -- --check

# 4. Re-run clippy with strict warnings
echo "Re-running clippy after fix..."
cargo clippy --workspace --all-features -- -D warnings

# 5. Re-run tests to ensure no regressions
echo "Running tests after fix..."
cargo test --workspace --all-features

echo "Automated fixes applied and re-validated. Review changes and create PR."
# git add .
# git commit -m "feat: Automated code modernization with cargo fix"
# git push origin HEAD
\`\`\`

### Anti-Patterns to Avoid
-   **Ignoring Warnings (`-A warnings`):** Setting `clippy` to allow all warnings in CI, effectively bypassing the policy. Always prefer `-D warnings` to treat warnings as errors.
-   **Running `cargo fmt` without `--check` in CI:** This would auto-format code, but not fail the CI if the code was initially unformatted, leading to inconsistent style in the main branch.
-   **Applying `cargo fix` without subsequent checks:** Applying fixes blindly without immediately verifying that `fmt`, `clippy`, and `test` still pass can introduce new issues or fail to meet the quality baseline.

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Code Formatting | Custom scripts with `sed`/`awk` | `rustfmt` | Handles complex Rust syntax, idempotent, widely adopted. Custom scripts are fragile and hard to maintain. |
| Code Linting | Custom regex-based linters | `clippy` | Provides deep semantic analysis, covers numerous edge cases and best practices, constantly updated by the community. |
| Automated Refactorings | Manual search-and-replace | `cargo fix` | Leverages `rustc` and `clippy` suggestions, understands the Rust AST for safe, accurate changes. Manual refactoring is prone to errors. |

**Key insight:** The Rust ecosystem provides robust, well-maintained, and semantically aware tools for code quality and modernization. Custom solutions will almost always be inferior and introduce maintenance burden.

## Common Pitfalls

### Pitfall 1: Version Drift
**What goes wrong:** `rustfmt` or `clippy` updates introduce new formatting rules or lints, causing previously passing code to fail CI.
**Why it happens:** Tools are under active development; new Rust versions often come with new tool versions.
**How to avoid:**
1.  **Pin Toolchain:** Use `rust-toolchain.toml` to pin the exact Rust toolchain version (e.g., `stable-2026-02-01`). This provides stability.
2.  **Regular Updates (Controlled):** Periodically update the pinned toolchain and address new warnings/formatting changes in a dedicated "toolchain upgrade" PR.
**Warning signs:** CI failures after `rustup update` or toolchain bumps without corresponding code changes.

### Pitfall 2: Configuration Overwhelm / False Positives
**What goes wrong:** Enabling too many `clippy` lints, especially strict ones, can lead to excessive warnings or lints that are not relevant to the project's context, causing "noise" and developer fatigue.
**Why it happens:** Desire for high quality, but without careful curation.
**How to avoid:**
1.  **Curated Clippy:** Start with a sensible subset of `clippy` lints (e.g., `cargo clippy -- -A clippy::pedantic` or by explicitly allowing/denying specific lints).
2.  **Targeted Lint Control:** Use `#[allow(clippy::lint_name)]` on specific items or modules where a lint is genuinely a false positive or not applicable. This should be a deliberate decision.
3.  **Gradual Ratcheting (QG-02 related):** Gradually enable stricter lints as the codebase matures and issues are addressed.
**Warning signs:** Many `clippy` warnings that developers manually `#[allow]` or ignore, or a backlog of `clippy` issues.

### Pitfall 3: Performance Degradation in CI
**What goes wrong:** Running all quality checks (`fmt`, `clippy`, `test`) on large codebases can lead to long CI times, slowing down development feedback loops.
**Why it happens:** Compiling and checking large amounts of code is resource-intensive.
**How to avoid:**
1.  **Caching:** Ensure CI pipelines leverage caching for `cargo` build artifacts.
2.  **Parallelization:** Run independent checks in parallel jobs where possible (e.g., `fmt` in one job, `clippy` in another, tests in a third).
3.  **Incremental Checks:** For large projects, consider tools or CI configurations that only run checks on changed files/modules (more advanced, often requires custom scripting or specialized CI features).
**Warning signs:** CI runs taking consistently longer than acceptable (e.g., >10-15 minutes for minor changes).

## Code Examples

Verified patterns from official sources:

### `rustfmt.toml` example (at project root)
\`\`\`toml
// Source: https://rust-lang.github.io/rustfmt/
max_width = 100
tab_spaces = 4
newline_style = "Unix"
imports_granularity = "Module"
group_imports = "StdExternalCrate"
format_code_in_doc_comments = true
\`\`\`

### Basic CI Integration with `clippy` attribute
\`\`\`rust
// Source: Internal Knowledge (Common Rust patterns)
// Example of how to enable specific clippy lints for a module or crate
#![deny(clippy::all)] // Deny all lints not explicitly allowed
#![deny(clippy::pedantic)] // Deny pedantic lints, often requires more allows
#![allow(clippy::module_name_repetitions)] // Allow if common in project

fn calculate_thing(input_data: &str) -> usize {
    // clippy might suggest a more idiomatic way, but for now we deny pedantic and allow specific
    input_data.len()
}
\`\`\`

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual code review for style/idioms | `rustfmt` & `clippy` as CI gates | Ongoing, significant uptake in last 5 years | Automates consistency, catches more errors, frees up human reviewers for logic. |
| Ad-hoc fixing of warnings | `cargo fix` automated workflows | `cargo fix` stabilized with Rust 1.29 (2018) | Enables large-scale, reliable code modernization and maintenance. |

**Deprecated/outdated:**
-   **Pre-`rustfmt` custom style guides:** Largely replaced by automated formatting tools for consistency and less Bikeshedding.

## Open Questions

Things that couldn't be fully resolved:

1.  **Fine-grained Clippy Curation by Module:**
    -   What we know: `clippy` lints can be controlled via attributes (`#[allow]`, `#[deny]`) at different granularities (crate, module, function). `.cargo/config.toml` can set default lint levels.
    -   What's unclear: QG-02 mentions "ratcheting quality policy by module criticality." While this phase focuses on *enforcement*, the *definition* of such a fine-grained policy and its implementation details (e.g., how to express different lint sets for different modules automatically) are not fully explored here. This might involve custom build scripts or advanced `clippy` configurations.
    -   Recommendation: The planner should assume a project-wide `clippy` policy for Phase 87. If module-specific policies are critical for QG-02, further research or design will be needed in that phase.

## Sources

### Primary (HIGH confidence)
-   Internal Knowledge: Rust tooling (`rustfmt`, `clippy`, `cargo fix`) behavior and common CI/CD patterns for Rust projects are well-established.

### Secondary (MEDIUM confidence)
-   Internal Knowledge: Best practices for configuring these tools and integrating them into automated workflows.

### Tertiary (LOW confidence)
-   N/A (Google Search failed, so no specific external web sources to cite).

## Metadata

**Confidence breakdown:**
-   Standard stack: HIGH - Core Rust tools are extremely stable and well-documented.
-   Architecture: MEDIUM - Common patterns are established, but specific implementation details (e.g., CI provider syntax) can vary.
-   Pitfalls: MEDIUM - Based on general experience with large Rust projects.

**Research date:** 2026-03-08
**Valid until:** 2026-09-08 (Rust tooling evolves, but core principles are stable; 6 months allows for minor updates)
