# Quality Baseline - Operator Guide

This document describes the baseline quality gate contract for LibreRoaster.

## Core Principle

**"Same input, same verdict"** — Running the baseline on the same code will always produce the same result, regardless of environment or tool version.

## Baseline Command

Run the complete quality baseline with a single command:

```bash
# From repository root
cargo fmt --all -- --check && cargo clippy --locked --all-targets -- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic && cargo test --locked --target x86_64-unknown-linux-gnu --features test --lib --tests --no-fail-fast
```

Alternatively, run each gate individually in order:

```bash
# Gate 1: Formatting
cargo fmt --all -- --check

# Gate 2: Linting  
cargo clippy --locked --all-targets -- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic

# Gate 3: Tests
cargo test --locked --target x86_64-unknown-linux-gnu --features test --lib --tests --no-fail-fast

# Gate 4: Regression numeric suite (H-9, audit 2026-08-11)
# The strongest numeric tests (tests/sensor_conversion.rs — 19-bit
# two's-complement LSB math, fixture→fault mapping) are gated behind
# `--features regression` and are NOT part of Gate 3. Run them explicitly:
cargo test --locked --target x86_64-unknown-linux-gnu --features test --features regression --test sensor_conversion --no-fail-fast
```

## Output Mode

Default output is **compact** — concise pass/fail visibility for each gate. When failures occur, all findings are reported with policy context.

## Reproducibility

The baseline enforces reproducibility through:

1. **Locked dependencies** (`--locked` flag)
2. **Fixed gate order** (fmt → clippy → test)
3. **Explicit host scope** (`--target x86_64-unknown-linux-gnu --features test`): host tests require the `test` feature to link (Embassy time driver)
4. **Deterministic toolchain** (uses current stable toolchain)

## Post-Fix Workflow

After fixing failures:

1. **Run the full baseline** — not partial gates
2. All three gates must pass before code is baseline-compliant
3. Reruns are deterministic: same input → same verdict

## Policy Reference

- **Policy ID:** QG-POLICY
- **Policy Version:** See `baseline-policy.toml`
- **Authority:** `.planning/quality/baseline-policy.toml`

## Output Formats

`quality-baseline.sh` runs the three cargo gates directly and prints their plain output (fmt/clippy/test). The tiered evaluator (`[GATE]`, `T1 BLOCK`, `VERDICT`) is a separate tool — `scripts/quality_baseline.py` — invoked by `scripts/quality-baseline-selfcheck.sh` against the fixtures in `tests/quality/fixtures/`:

```
[GATE] [TIER] [BLOCKING/INFO] path/to/file.rs:LINE:COL - rule_id (policy: tier=TIER_NAME)
```

Example:
```
clippy T1 BLOCK src/control/roaster.rs:42:10 - clippy::unwrap_used (policy: tier=t1_critical)
```

## Ratchet Updates

Policy ratchets are governed by:
- Version bump required (semver)
- Human-readable changelog entry required

See `ratchet-changelog.md` for update history.
