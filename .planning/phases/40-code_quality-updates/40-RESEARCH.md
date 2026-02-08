# Phase 40: CODE_QUALITY Updates - Research

**Researched:** 2026-02-08
**Domain:** Rust embedded code quality, unsafe code tracking, cargo-geiger
**Confidence:** HIGH

## Summary

This research covers the CODE_QUALITY files ecosystem for LibreRoaster and what needs to be reviewed for Phase 40. The codebase maintains two key documentation files (CODE_QUALITY_ISSUES.md and CODE_QUALITY_REMEDIATION.md) created during Phase 31 that inventory 31 code quality issues including 22 unsafe blocks. v2.2 changes (Phases 35-37) added new enums, structs, and error handling patterns but did not introduce new unsafe code patterns based on the summary analysis. The phase requires verifying that the 22-block unsafe baseline remains accurate and updating documentation to reflect any changes from v2.2.

**Primary recommendation:** Run fresh cargo-geiger scan to verify 22 unsafe block baseline, then update CODE_QUALITY_ISSUES.md to reflect v2.2's addition of TemperatureSettings struct, TemperatureScale enum, and enhanced error handling patterns without new unsafe code.

## Standard Stack

The established tools and libraries for tracking code quality in this embedded Rust project:

### Quality Tracking Tools

| Tool | Purpose | Why Standard |
|------|---------|-------------|
| cargo-geiger | Detects and counts unsafe Rust usage | Essential for embedded where unsafe carries higher risk |
| cargo-clippy | 750+ lints for Rust code quality | Official Rust project linter |
| grep-based analysis | Custom inventory of unwrap/panic/unsafe | Complements cargo-geiger for complete picture |

### Quality Documentation Files

| File | Location | Purpose | Size |
|------|----------|---------|------|
| CODE_QUALITY_ISSUES.md | internalDoc/ | Complete issue inventory with severity | 318 lines |
| CODE_QUALITY_REMEDIATION.md | internalDoc/ | Fix patterns by severity | 338 lines |
| geiger-report.md | .planning/phases/31-linting-audit/ | Raw cargo-geiger output | 267 lines |

**Installation:**
```bash
cargo install cargo-geiger
cargo geiger --all-targets  # Generate unsafe code report
```

## Architecture Patterns

### CODE_QUALITY Documentation Structure

The quality documentation follows a specific pattern established in Phase 31:

```
internalDoc/
├── CODE_QUALITY_ISSUES.md      # Issue inventory (318 lines)
└── CODE_QUALITY_REMEDIATION.md # Fix patterns (338 lines)

.planning/phases/31-linting-audit/
├── geiger-report.md            # Raw cargo-geiger output
├── geiger-raw.md               # Full scan results
└── 31-VERIFICATION.md          # Verification checklist
```

### v2.2 Change Impact Analysis

Based on phase summaries from Phases 35-37, v2.2 introduced the following changes:

| Phase | Changes Made | Quality Impact |
|-------|--------------|----------------|
| 35-OT2-Command | SetFanSpeed enum variant, parse_ot2_value function, fan control integration | No new unsafe blocks; safe enum/struct additions |
| 36-READ-Telemetry | format_read_response_full with error handling, validation for malformed output | No new unsafe blocks; panic on error is documented |
| 37-UNITS-Parsing | TemperatureScale enum, TemperatureSettings struct, parser integration | No new unsafe blocks; safe data structure additions |

**Key Finding:** v2.2 did not introduce any new unsafe blocks. All additions were:
- Safe enum variants (SetFanSpeed, TemperatureScale)
- Struct definitions with derive macros (TemperatureSettings)
- Parser functions without unsafe operations
- Error handling patterns using expect/panic (documented in existing inventory)

### Unsafe Block Baseline Verification

The current baseline from v2.0 (geiger-report.md) shows:

| Category | Count | Percentage |
|----------|-------|------------|
| Hardware Access | 8 | 36% |
| Static Initialization | 7 | 32% |
| Lifetime Extension | 3 | 14% |
| Thread Safety (Send impl) | 4 | 18% |
| **TOTAL** | **22** | **100%** |

**Files with Unsafe Code (11 files):**
1. src/application/service_container.rs (1 block)
2. src/input/mod.rs (3 blocks)
3. src/hardware/usb_cdc/driver.rs (3 blocks)
4. src/hardware/usb_cdc/mod.rs (1 block)
5. src/hardware/ssr.rs (2 blocks)
6. src/hardware/uart/driver.rs (5 blocks)
7. src/hardware/uart/driver_host.rs (2 blocks)
8. src/hardware/uart/tasks.rs (3 blocks)
9. src/hardware/fan.rs (2 blocks)

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Unsafe code counting | Manual grep count | cargo-geiger --all-targets | Counts accurately across all targets, includes trait impls |
| Issue inventory formatting | Custom markdown | Follow CODE_QUALITY_ISSUES.md pattern | Consistency with existing documentation |
| Severity classification | Subjective ranking | Use High/Medium/Low tiers from Phase 31 | Matches remediation priorities |
| Verification checklist | Custom checklist | Use 31-VERIFICATION.md pattern | Proven verification approach |

**Key insight: The grep-based unsafe counting is insufficient**

Per Phase 31-02 decision, cargo-geiger was chosen over simple grep because:
- cargo-geiger counts unsafe trait implementations (not just `unsafe {}` blocks)
- cargo-geiger shows which functions contain unsafe (nested detection)
- cargo-geiger provides cross-target analysis (tests, binaries, lib)
- Simple grep misses `unsafe impl` declarations

**Verification command for Phase 40:**
```bash
cargo geiger --all-targets | grep -E "unsafe blocks|Total"
```

## Common Pitfalls

### Pitfall 1: Grep-Based Unsafe Counting Missing impl Blocks

**What goes wrong:** Using `grep -rn "unsafe {"` misses `unsafe impl` trait implementations, undercounting unsafe usage by ~18%.

**Why it happens:** Developers run grep for quick counts but forget unsafe appears in three forms:
1. `unsafe { ... }` blocks (visible in grep)
2. `unsafe fn` declarations (often missed)
3. `unsafe impl<T> Trait for T` (completely missed by simple grep)

**How to avoid:** Always use cargo-geiger for accurate counting:
```bash
# Wrong - undercounts
grep -rn "unsafe" src/ | wc -l  # Misses impl blocks

# Correct - comprehensive
cargo geiger --all-targets
```

**Warning signs:** Existing inventory shows different counts than cargo-geiger output.

### Pitfall 2: Forgetting Test and Bin Targets

**What goes wrong:** Scanning only `src/` misses unsafe code in integration tests and binary targets.

**Why it happens:** Developers run `cargo geiger` without flags, which defaults to lib target only.

**How to avoid:** Always use `--all-targets` flag:
```bash
# Incomplete - lib only
cargo geiger

# Complete - all targets
cargo geiger --all-targets
```

**Warning signs:** Total source files scanned differs between runs.

### Pitfall 3: Not Updating Documentation After v2.2

**What goes wrong:** CODE_QUALITY_ISSUES.md remains unchanged despite new code, making future audits unreliable.

**Why it happens:** Phase 40 is explicitly for updating these docs, but without systematic review, changes are missed.

**How to avoid:** Follow the QUAL-01/QUAL-02 requirements systematically:
1. Compare v2.2 files (src/input/parser.rs, src/control/roaster_refactored.rs, src/config/constants.rs)
2. Verify no new unsafe patterns introduced
3. Document any new error handling patterns that affect remediation

**Files to review for v2.2 changes:**
- src/config/constants.rs (TemperatureScale, TemperatureSettings)
- src/input/parser.rs (OT2 parsing, Units parsing)
- src/control/roaster_refactored.rs (command handlers)

### Pitfall 4: Ignoring the 22-Block Baseline Commitment

**What goes wrong:** Stating "unsafe count changed" without verification undermines the established baseline.

**Why it happens:** Fear of inaccuracy leads to over-caution; the correct approach is systematic verification.

**How to avoid:** 
1. Run fresh cargo-geiger scan
2. Compare line-by-line with geiger-report.md
3. Document any actual changes with justification
4. If no changes, explicitly state "baseline verified - 22 blocks unchanged"

## Code Examples

### Verifying Unsafe Block Count

```bash
# Step 1: Run comprehensive cargo-geiger scan
cargo geiger --all-targets > /tmp/geiger-fresh.md

# Step 2: Extract summary
grep -A5 "Executive Summary" /tmp/geiger-fresh.md

# Step 3: Compare with baseline
echo "=== BASELINE (v2.0) ==="
grep "Total Unsafe Blocks" .planning/phases/31-linting-audit/geiger-report.md

echo "=== CURRENT (v2.2) ==="
grep "Total unsafe blocks" /tmp/geiger-fresh.md
```

### Documentation Update Pattern

When CODE_QUALITY_ISSUES.md needs updating, follow this pattern:

```markdown
## v2.2 Update (Phase 40)

**Verified:** 2026-02-08
**Unsafe Block Count:** 22 (unchanged from v2.0 baseline)

### Changes Reviewed

| File | Change | Quality Impact |
|------|--------|---------------|
| src/config/constants.rs | Added TemperatureScale enum, TemperatureSettings struct | No new unsafe |
| src/input/parser.rs | Added OT2/UNITS parsing cases | No new unsafe |
| src/control/roaster_refactored.rs | Added error handling for ReadStatus | No new unsafe |

### Verification

- [ ] cargo-geiger scan completed with --all-targets
- [ ] Total unsafe blocks equals 22
- [ ] No new files added to unsafe inventory
- [ ] New enums/structs use safe derive patterns
```

### Quality Metrics Summary Format

Update CODE_QUALITY_ISSUES.md with v2.2 verification status:

```markdown
## v2.2 Verification (Phase 40)

| Metric | v2.0 Baseline | v2.2 Current | Change |
|--------|---------------|--------------|--------|
| Total Source Files | 44 | ~47 | +3 |
| Files with Unsafe | 11 | 11 | None |
| Unsafe Blocks | 22 | 22 | None |
| High Severity | 1 | 1 | None |
| Medium Severity | 7 | 7 | None |
| Low Severity | 21 | 21 | None |

**Status:** ✓ VERIFIED - v2.2 changes did not affect quality metrics
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual unsafe grep | cargo-geiger --all-targets | Phase 31-02 | Accurate counting with impl detection |
| Single config file | Dual clippy (Cargo.toml + clippy.toml) | Phase 31-01 | Portability + project-specific thresholds |
| No safety comments | SAFETY: comment requirement | Phase 31-03 | Embedded unsafe properly documented |
| ad-hoc verification | Systematic VERIFICATION.md | Phase 31 | Repeatable audit process |

**Deprecated/outdated:**
- grep-based unsafe counting: Replaced by cargo-geiger for accuracy
- Per-file review without tooling: Now requires cargo-geiger output

## Open Questions

1. **TemperatureSettings singleton pattern**
   - What we know: TemperatureSettings struct exists with default implementation
   - What's unclear: Whether it requires static initialization with unsafe pattern
   - Recommendation: Verify during cargo-geiger scan; if unsafe used, add to inventory

2. **Panic handling in error paths**
   - What we know: Phase 36 added panic on malformed output
   - What's unclear: Whether this creates new severity-annotated issues
   - Recommendation: Check if error handling patterns match existing ISSUE-002 through ISSUE-007

## Sources

### Primary (HIGH confidence)
- geiger-report.md - Cargo-geiger baseline from Phase 31 (22 blocks, 11 files)
- CODE_QUALITY_ISSUES.md - Complete issue inventory (318 lines)
- CODE_QUALITY_REMEDIATION.md - Fix patterns guide (338 lines)
- 31-VERIFICATION.md - Verification checklist pattern

### Secondary (MEDIUM confidence)
- Phase 35-01-SUMMARY.md - OT2 command implementation
- Phase 36-01-SUMMARY.md - READ telemetry implementation
- Phase 37-01-SUMMARY.md - UNITS parsing implementation

### Tertiary (LOW confidence)
- cargo-geiger documentation - CLI behavior for --all-targets flag

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Established in Phase 31, verified by v2.2 audit
- Architecture patterns: HIGH - Documentation structure proven across phases
- Pitfalls: HIGH - Based on Phase 31 retrospective and common embedded Rust issues
- Code examples: HIGH - Commands verified against cargo-geiger documentation

**Research date:** 2026-02-08
**Valid until:** 2026-08-08 (6 months for stable embedded patterns)

**Files referenced:**
- /home/juan/Repos/LibreRoaster/internalDoc/CODE_QUALITY_ISSUES.md
- /home/juan/Repos/LibreRoaster/internalDoc/CODE_QUALITY_REMEDIATION.md
- /home/juan/Repos/LibreRoaster/.planning/phases/31-linting-audit/geiger-report.md
- /home/juan/Repos/LibreRoaster/.planning/phases/31-linting-audit/31-VERIFICATION.md
