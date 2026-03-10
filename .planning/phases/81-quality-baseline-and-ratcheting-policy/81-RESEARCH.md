# Phase 81: Quality Baseline and Ratcheting Policy - Research

**Researched:** 2026-03-07
**Domain:** Rust quality-gate orchestration and module-criticality lint policy
**Confidence:** HIGH

## Summary

This research focused on how to plan a deterministic Rust quality baseline for LibreRoaster using the locked phase scope: one orchestrator command, compact output, terminal-only determinism evidence, all failures listed, module+rule actionable failures, and explicit ratcheting by module criticality. I verified Cargo/Clippy/rustc behavior from official docs and validated repository-specific behavior by running gates in this codebase.

The standard approach for this phase is: run `fmt --check`, curated `clippy`, and tests under a pinned toolchain and lockfile policy; collect diagnostics in machine-readable form (`--message-format=json`) for accurate module+rule reporting; and apply pass/fail by policy tier instead of raw global `-D warnings` for all modules. This matches QG-01/QG-02 and avoids blocking lower-risk modules early.

Repository evidence shows two major planning constraints: (1) `cargo clippy -- -D warnings` currently fails on many style/perf lints and on renamed/unknown lint IDs in `Cargo.toml`; (2) `cargo test` passes unit/integration tests but fails doctests, and `--all-features` can pull in embedded-only paths that fail on host. The baseline must therefore define a curated test scope and lint scope explicitly, not rely on blanket defaults.

**Primary recommendation:** Implement a single baseline runner that executes deterministic gates with JSON diagnostics, then enforces policy by module tier (block Tier 1 now, report-only Tier 2/3), with policy version + changelog-based ratchet updates.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust toolchain via `rust-toolchain.toml` | repo currently `stable` (local: `rustc 1.92.0`) | Pins compiler/tooling behavior for reproducible verdicts | rustup toolchain files are the standard project-level pinning mechanism; Cargo behavior depends on active toolchain |
| Cargo lockfile policy (`--locked`, optional `--frozen`) | Cargo 1.92 docs | Deterministic dependency resolution during quality runs | Cargo explicitly documents `--locked`/`--frozen` for deterministic CI-like builds |
| rustfmt via `cargo fmt --all -- --check` | `rustfmt 1.8.0-stable` (local) | Formatting gate with deterministic exit status | rustfmt `--check` has clear pass/fail exit semantics for CI gating |
| Clippy via `cargo clippy` + curated lint levels | `clippy 0.1.92` (local) | Lint gate with rule-level diagnostics | Clippy is the standard Rust lint tool; supports command-line lint levels and source-level policy |
| Cargo test (`cargo test`) with explicit target scope | Cargo 1.92 docs | Test gate with full failure listing behavior | Cargo supports `--no-fail-fast` and explicit target selection; required for predictable all-failures reporting |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| Cargo JSON messages (`--message-format=json`) | Cargo 1.92 docs | Structured diagnostics for module+rule extraction | Use in baseline runner when building compact actionable summaries |
| `[lints.rust]` and `[lints.clippy]` in `Cargo.toml` | stable, MSRV respected since 1.74 | Project-scoped lint policy declaration | Use for domain policy defaults and stable lint ownership |
| rustc lint attributes (`#[allow/warn/deny/forbid]`) | stable reference | Module-level strictness overrides for tiering | Use to ratchet stricter modules without blocking all modules at once |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Parsing human stderr with ad-hoc regex | Cargo JSON stream (`--message-format=json`) | JSON is robust and includes package/target/spans/rule IDs; regex parsing is fragile |
| Global `-D warnings` for all modules immediately | Tiered pass/fail (block only high-criticality modules initially) | Immediate global deny blocks lower-risk work and violates phase direction |
| `cargo test --all-features --all-targets` as baseline default | Curated host-safe test scope | All-features/all-targets currently drags in embedded-only binary paths and fails on host |

**Installation:**
```bash
rustup component add rustfmt clippy
cargo fetch --locked
```

## Architecture Patterns

### Recommended Project Structure
```
.planning/quality/
├── baseline-policy.toml      # policy_id, version, tiers, module mapping, gate settings
├── ratchet-changelog.md      # human-readable per-version policy deltas
└── README.md                 # operator-facing compact run contract

scripts/
└── quality-baseline.sh       # single orchestrator command entrypoint
```

### Pattern 1: Deterministic Baseline Orchestrator
**What:** One command runs `fmt`, `clippy`, and `test` in fixed order with fixed flags, fixed toolchain, and lockfile enforcement.
**When to use:** Every baseline run, local and CI.
**Example:**
```bash
# Source: Cargo command docs + rustup override docs
cargo +stable fmt --all -- --check
cargo +stable clippy --locked --message-format=json --all-targets -- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic
cargo +stable test --locked --tests --lib --no-fail-fast
```

### Pattern 2: Policy-Driven Tiered Enforcement
**What:** Evaluate diagnostics against `module -> tier` map; fail only for blocking tiers, emit informational findings for lower tiers.
**When to use:** Initial ratchet rollout where lower-risk modules must not block.
**Example:**
```toml
# Source: Cargo manifest lints section + Rust lint attributes docs
policy_id = "QG-POLICY"
policy_version = "1.0.0"

[tiers.t1_critical]
blocking = true
modules = ["src/safety/**", "src/control/**", "src/input/parser.rs", "src/output/artisan.rs", "src/config/**"]

[tiers.t2_core]
blocking = false
modules = ["src/hardware/**", "src/application/**", "src/input/multiplexer.rs", "src/output/traits.rs"]

[tiers.t3_support]
blocking = false
modules = ["src/logging/**", "src/common/**", "tests/**"]
```

### Pattern 3: Compact Failure Summary with Policy Context
**What:** Print failures grouped by gate, each line includes rule, module/file, severity tier, and policy ID/version reference.
**When to use:** Default run output mode.
**Example:**
```text
FAIL clippy  [QG-POLICY@1.0.0]
- T1 BLOCK  src/control/roaster_refactored.rs  clippy::manual_range_contains  (policy: tier=t1_critical)
- T2 INFO   src/application/service_container.rs  clippy::type_complexity      (policy: tier=t2_core)
```

### Anti-Patterns to Avoid
- **Global deny from day one:** `-D warnings` across all modules violates non-blocking lower-risk requirement.
- **Feature-expansive test gate by default:** `--all-features` currently introduces host-incompatible embedded paths.
- **Stop-on-first-gate failure:** phase requires listing all failures; run all gates and aggregate verdict.
- **Policy without versioned delta:** ratchet updates must include both version bump and human-readable changelog.

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Diagnostic extraction | Regex parser over ANSI text | Cargo JSON messages (`--message-format=json`) | Official schema includes `reason`, `package_id`, target, spans, and diagnostics; far less brittle |
| Lint policy plumbing | Custom global lint registry format | `Cargo.toml` `[lints.*]` + rust lint attributes | Native Rust/Cargo lint system already supports levels and scoped overrides |
| Determinism controls | Homegrown lock semantics | `--locked` / `--frozen` + toolchain file | Officially supported deterministic dependency and toolchain behavior |
| Full-failure test behavior | Custom test harness scheduler | `cargo test --no-fail-fast` | Cargo already supports running all test executables before failing |

**Key insight:** This phase should orchestrate and express policy around existing Cargo/rustc/Clippy primitives, not invent a parallel quality engine.

## Common Pitfalls

### Pitfall 1: Unknown or renamed Clippy lints in policy
**What goes wrong:** Gate emits `unknown lint` / `renamed lint` warnings, undermining policy trust.
**Why it happens:** Clippy lint names evolve; stale lint IDs remain in `Cargo.toml` or CLI flags.
**How to avoid:** Validate curated lint list against current Clippy docs before locking policy version.
**Warning signs:** Output includes `unknown lint: clippy::...` or `lint ... has been renamed`.

### Pitfall 2: Baseline test scope accidentally includes embedded-only targets
**What goes wrong:** Host baseline fails compiling embedded binary/main path.
**Why it happens:** Using `--all-features` and/or broad target selection for host baseline.
**How to avoid:** Define host baseline test target set explicitly (`--lib --tests`), and keep embedded checks as separately declared gate.
**Warning signs:** Errors from `src/main.rs` requiring `esp_hal`/`panic_handler` during host run.

### Pitfall 3: Doctests silently become blocker noise
**What goes wrong:** `cargo test` fails on doctest examples unrelated to intended baseline scope.
**Why it happens:** `cargo test` runs doctests by default for lib targets.
**How to avoid:** Decide and document doctest policy explicitly (include and fix, or exclude initially and ratchet later).
**Warning signs:** Failures under `Doc-tests ...` despite green unit/integration tests.

### Pitfall 4: Non-deterministic verdict due to environment drift
**What goes wrong:** Same code gets different verdicts across runs/machines.
**Why it happens:** Unpinned toolchain, unlocked dependency resolution, or differing feature/target sets.
**How to avoid:** Force toolchain + lock policy + fixed gate flags, and print them in run header.
**Warning signs:** Different lint inventory after toolchain updates without policy version change.

### Pitfall 5: Ratchet policy updates are opaque
**What goes wrong:** Teams cannot explain why gate behavior changed.
**Why it happens:** Tier changes occur without explicit policy version/delta publication.
**How to avoid:** Require policy semantic version bump + human-readable changelog per ratchet.
**Warning signs:** New block failures appear with no policy artifact diff.

## Code Examples

Verified patterns from official sources:

### Deterministic Cargo invocations
```bash
# Source: https://doc.rust-lang.org/cargo/commands/cargo-check.html
# Source: https://doc.rust-lang.org/cargo/commands/cargo-test.html
cargo +stable clippy --locked --message-format=json --all-targets -- -W clippy::unwrap_used
cargo +stable test --locked --tests --lib --no-fail-fast
```

### Module-level lint ratcheting
```rust
// Source: https://doc.rust-lang.org/reference/attributes/diagnostics.html
#![warn(clippy::unwrap_used)]

mod safety {
    #![deny(clippy::unwrap_used)]
    // stricter tier module
}
```

### Cargo lint policy declaration
```toml
# Source: https://doc.rust-lang.org/cargo/reference/manifest.html#the-lints-section
[lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Project-specific, ad-hoc lint conventions in code/comments | First-class Cargo `[lints]` section with priority and tool scoping | Respected on stable since Rust/Cargo 1.74 | Policy can live in manifest and be ratcheted systematically |
| Human-only gate output parsing | Cargo JSON messages for external tools | Mature in Cargo reference; `package_id` format stabilized by 1.77 | Reliable module+rule extraction for actionable summaries |
| Immediate global strict linting in brownfield code | Tiered enforcement (block high-criticality first) | Widely adopted brownfield hardening practice | Reduces adoption friction while preserving safety-first blocking |

**Deprecated/outdated:**
- Blanket `-D warnings` for all modules at phase start (conflicts with non-blocking lower-risk requirement).
- Regex/grep-only parsing of compiler text as the primary failure signal transport.

## Open Questions

1. **Should doctests be in the initial blocking baseline?**
   - What we know: `cargo test` currently fails on multiple doctests while unit/integration tests pass.
   - What's unclear: Whether phase acceptance expects doctests in QG-01 baseline immediately.
   - Recommendation: Start baseline blocking on `--lib --tests`; track doctests as explicit ratchet item for next policy version.

2. **Exact protocol-module boundary for Tier 1 mapping**
   - What we know: Requirement says safety/control/protocol first; protocol logic spans `src/input/parser.rs`, `src/output/artisan.rs`, and protocol constants/config.
   - What's unclear: Whether `src/input/multiplexer.rs` should be Tier 1 now or Tier 2 initially.
   - Recommendation: Put parser/formatter/constants in Tier 1 now, multiplexer in Tier 2 for initial non-blocking rollout.

3. **Google ecosystem trend verification unavailable in this environment**
   - What we know: `google_search` tool returned `403 PERMISSION_DENIED` during this research run.
   - What's unclear: Broad 2026 community trend deltas beyond official docs.
   - Recommendation: Treat ecosystem-trend claims as constrained; rely on official docs + repository evidence for planning.

## Sources

### Primary (HIGH confidence)
- https://doc.rust-lang.org/cargo/commands/cargo-check.html - `--locked`, `--frozen`, `--keep-going`, `--message-format`
- https://doc.rust-lang.org/cargo/commands/cargo-test.html - `--no-fail-fast`, target selection, doctest defaults
- https://doc.rust-lang.org/cargo/reference/external-tools.html#json-messages - Cargo JSON diagnostics schema
- https://doc.rust-lang.org/stable/clippy/usage.html - Clippy CLI usage, lint group behavior, command-line levels
- https://doc.rust-lang.org/stable/clippy/continuous_integration/index.html - CI guidance (`-D warnings`, toolchain alignment)
- https://doc.rust-lang.org/rustc/lints/levels.html - lint levels, CLI precedence, `--cap-lints`
- https://doc.rust-lang.org/reference/attributes/diagnostics.html - module/item lint attributes and override behavior
- https://doc.rust-lang.org/cargo/reference/manifest.html#the-lints-section - `[lints]` manifest policy, MSRV note
- https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file - toolchain file behavior and precedence
- Repository validation runs (2026-03-07):
  - `cargo fmt --all -- --check` (passes)
  - `cargo clippy --all-targets --all-features -- -D warnings` (fails; unknown/renamed lint IDs + many diagnostics)
  - `cargo test --no-fail-fast` (unit/integration pass, doctests fail)

### Secondary (MEDIUM confidence)
- https://github.com/rust-lang/rustfmt/blob/master/README.md - rustfmt `--check` exit semantics and CI usage

### Tertiary (LOW confidence)
- N/A (web ecosystem search unavailable due tool permission error)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - grounded in official Cargo/rustc/Clippy docs and local repo evidence.
- Architecture: HIGH - directly mapped to locked phase decisions and verified tool capabilities.
- Pitfalls: HIGH - based on observed failures in this repository plus official command semantics.

**Research date:** 2026-03-07
**Valid until:** 2026-04-06
