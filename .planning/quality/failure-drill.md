# Quality Baseline - Failure Drill and Reproducibility Guide

This document provides documented workflows for intentionally failing the baseline gate and verifying deterministic rerun behavior.

## Core Principle

**"Same input, same verdict"** — Running the baseline on the same code will always produce the same result. This is guaranteed by:
- Locked dependencies (`--locked` flag)
- Fixed gate order (fmt → clippy → test)
- Explicit policy version in output

## Intentional Failure Drill

Use the selfcheck script to verify intentional failure behavior without modifying source code:

```bash
# Run the deterministic failure drills
./scripts/quality-baseline-selfcheck.sh
```

### Expected Output

The selfcheck runs three verification tests:

1. **Tier 1 Blocking Drill** — Verifies that:
   - A finding in `src/control/` (Tier 1 module) triggers blocking
   - Output includes policy reference (`QG-POLICY@version`)
   - Output includes module path
   - Output includes tier marker (`T1 BLOCK`)
   - Output includes lint rule ID

2. **Mixed Tier Findings Drill** — Verifies that:
   - All findings are listed (not just first failure)
   - Tier 2/3 findings are marked informational
   - Final verdict depends only on Tier 1 blocking findings

3. **Reproducibility Drill** — Verifies that:
   - Same fixture run twice produces identical verdict
   - Exit codes match
   - Output text is deterministic

### Pass/Fail Outcomes

| Drill | Expected Outcome |
|-------|------------------|
| Tier 1 Blocking | Exit code 1 (FAIL) — action required |
| Mixed Tiers | Exit code 0 (PASS) — T2/T3 are informational |
| Reproducibility | Both runs return same exit code and verdict text |

## Normal Baseline Run

Run the full baseline against actual source code:

```bash
./scripts/quality-baseline.sh
```

### Expected Output Format

```
==============================================
  LibreRoaster Quality Baseline Runner
==============================================
Policy:     QG-POLICY
Version:    1.0.0
...

[GATE] Format Check (cargo fmt --check)
----------------------------------------------
[PASS] fmt

[GATE] Clippy Lint Check
----------------------------------------------
[PASS] clippy

[GATE] Test Execution (cargo test)
----------------------------------------------
[PASS] test

============================================================
 QUALITY BASELINE SUMMARY
============================================================
Policy:     QG-POLICY v1.0.0
Tier 1 (Blocking):     0
Tier 2 (Core):         0
Tier 3 (Support):     0
============================================================
[QG-POLICY@1.0.0] VERDICT: PASS
All Tier 1 (blocking) requirements satisfied.
==============================================
  FINAL VERDICT: PASS
==============================================
same input, same verdict - QG-POLICY v1.0.0
```

### Failure Output Format

When failures occur, the output includes all findings before the final verdict:

```
============================================================
 FINDINGS
============================================================

[QG-POLICY@1.0.0] CLIPPY Findings:
--------------------------------------------------
  - T1 BLOCK   src/control/roaster.rs:42                clippy::unwrap_used
             unwrapping a Result value, which may be an Err...

[QG-POLICY@1.0.0] VERDICT: FAIL
Tier 1 (blocking) issues must be resolved.

==============================================
  FINAL VERDICT: FAIL
==============================================
same input, same verdict - QG-POLICY v1.0.0
```

**Key elements in failure output:**
- `[QG-POLICY@v1.0.0]` — Policy ID and version for traceability
- `T1 BLOCK` or `T2 CORE` / `T3 SUPPORT` — Tier classification
- Module path with line number — Precise location
- Lint rule ID — Actionable fix guidance

## Full Rerun-After-Fix Workflow

After fixing failures identified by the baseline:

### Step 1: Verify Fixes

```bash
# Run baseline to confirm fixes
./scripts/quality-baseline.sh
```

### Step 2: Confirm Deterministic Behavior

The baseline is deterministic — rerunning with the same code produces the same result:

```bash
# Run twice to confirm reproducibility
./scripts/quality-baseline.sh
./scripts/quality-baseline.sh
```

Both runs should produce identical:
- Exit codes
- Finding counts by tier
- Verdict text

### Step 3: Policy Version Awareness

When policy ratchets (tightens), the version changes:

```bash
# Check current policy version
cat .planning/quality/baseline-policy.toml | grep policy_version
```

**Ratchet rules:**
- Version bump required (semver): v1.0.0 → v1.1.0
- Human-readable changelog entry required
- See `.planning/quality/ratchet-changelog.md` for update history

## Updating Policy (Ratcheting)

When you need to tighten quality requirements:

1. **Update version** in `.planning/quality/baseline-policy.toml`:
   ```toml
   [policy]
   policy_version = "1.1.0"  # Increment semver
   ```

2. **Add changelog entry** in `.planning/quality/ratchet-changelog.md`:
   ```markdown
   ## v1.1.0 - YYYY-MM-DD
   
   ### Changes
   - Added new Tier 1 module: src/application/* (previously T2)
   - Tightened clippy rules: added clippy::todo
   
   ### Rationale
   - Application layer is now mature enough to warrant blocking enforcement
   ```

3. **Verify baseline still passes** with new policy:
   ```bash
   ./scripts/quality-baseline.sh
   ```

4. **Commit** with version in message:
   ```bash
   git commit -m "chore: ratchet QG-POLICY to v1.1.0
   
   - Add src/application to Tier 1 blocking
   - Add clippy::todo to enforced lints
   "
   ```

## Troubleshooting

### Selfcheck Fails

If `./scripts/quality-baseline-selfcheck.sh` fails:

1. **Verify Python dependencies:**
   ```bash
   python3 -c "import json, argparse, re"
   ```

2. **Check fixture files exist:**
   ```bash
   ls -la tests/quality/fixtures/
   ```

3. **Run evaluator directly:**
   ```bash
   python3 scripts/quality_baseline.py --from-json tests/quality/fixtures/clippy-tier1-fail.jsonl
   ```

### Baseline Produces Different Results

If `./scripts/quality-baseline.sh` produces different results on rerun:

1. **Check for uncommitted changes:**
   ```bash
   git status
   ```

2. **Verify lockfile unchanged:**
   ```bash
   git diff Cargo.lock
   ```

3. **Check toolchain version:**
   ```bash
   rustc --version
   ```

4. **Verify policy file unchanged:**
   ```bash
   git diff .planning/quality/baseline-policy.toml
   ```

If all are unchanged and results differ, this is a bug — report it.

---

**Policy Reference:** QG-POLICY v1.0.0  
**Authority:** `.planning/quality/baseline-policy.toml`  
**Changelog:** `.planning/quality/ratchet-changelog.md`
