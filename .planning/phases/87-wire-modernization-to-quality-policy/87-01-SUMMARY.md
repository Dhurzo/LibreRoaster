---
phase: 87-wire-modernization-to-quality-policy
plan: 01
subsystem: testing
tags: [bash, clippy, rustfmt, cargo, quality-gate, shell-script]

# Dependency graph
requires:
  - phase: 86-fix-integration-regression-p84-p85
    provides: clean integration test baseline
  - phase: 81-quality-baseline-and-ratcheting-policy
    provides: quality policy framework (baseline-policy.toml, tier mappings)
provides:
  - simple quality-baseline.sh script invoking fmt/clippy/test in sequence
  - global Clippy deny-warnings policy in .cargo/config.toml
affects:
  - 87-02 (integrate quality-baseline.sh into modernization/regression scripts)
  - 88-architecture-alignment (must pass quality gate after refactoring)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Quality gate script pattern: set -euxo pipefail with ordered cargo commands"
    - "Global Clippy policy: deny=[warnings] in .cargo/config.toml"

key-files:
  created: []
  modified:
    - scripts/quality-baseline.sh
    - .cargo/config.toml

key-decisions:
  - "Replace complex policy-aware script with simple direct quality gate for 87-01 requirement"
  - "Clippy deny policy placed in .cargo/config.toml as specified; -D warnings flag in script provides runtime enforcement"

patterns-established:
  - "Quality gate script: set -euxo pipefail ensures any failing command exits immediately"

# Metrics
duration: 3min
completed: 2026-03-08
---

# Phase 87 Plan 01: Wire Modernization to Quality Policy Summary

**Simple quality-baseline.sh (set -euxo pipefail, cargo fmt/clippy/test) and global Clippy deny=[warnings] policy in .cargo/config.toml**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-08T15:38:27Z
- **Completed:** 2026-03-08T15:41:03Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Replaced the complex 200-line policy-aware quality-baseline.sh with a clean, simple 13-line script that directly runs `cargo fmt -- --check`, `cargo clippy --workspace --all-features -- -D warnings`, and `cargo test --workspace --all-features`
- Added `[lints.clippy]\ndeny = ["warnings"]` section to `.cargo/config.toml` establishing a global policy declaration
- Script uses `set -euxo pipefail` for robust execution - any failed check immediately exits with non-zero status

## Task Commits

Each task was committed atomically:

1. **Task 1: Create quality-baseline.sh script** - `8d4f57f` (feat)
2. **Task 2: Configure global Clippy policy in .cargo/config.toml** - `aeed9b4` (chore)

**Plan metadata:** `(pending)` (docs: complete plan)

## Files Created/Modified

- `scripts/quality-baseline.sh` - Simple quality gate script: fmt check → clippy with -D warnings → test, with set -euxo pipefail
- `.cargo/config.toml` - Added [lints.clippy] deny=["warnings"] global Clippy policy section

## Decisions Made

- **Replace complex script**: The existing quality-baseline.sh from Phase 81 was a 202-line complex policy-aware script with Python evaluator integration, tier policies, JSON output, etc. The plan specified a simple 13-line script. Replaced as specified to meet the plan's must_haves.
- **Script enforcement via -D warnings flag**: The `-D warnings` flag in `cargo clippy --workspace --all-features -- -D warnings` provides the runtime enforcement that treats all warnings as errors. The `[lints.clippy]` in `.cargo/config.toml` serves as a policy declaration (`.cargo/config.toml` [lints] sections are not standard Cargo config but required by plan spec).

## Deviations from Plan

### Auto-fixed Issues

None - plan executed exactly as written.

---

**Total deviations:** 0
**Impact on plan:** Plan executed as specified.

## Issues Encountered

- The existing `scripts/quality-baseline.sh` (from Phase 81) was a 202-line complex policy script, not the simple script the plan specifies. Replaced it as the plan requires, preserving the plan's exact content specification.
- The `[lints.clippy]` section in `.cargo/config.toml` is not standard Cargo configuration (lints belong in `Cargo.toml` workspace manifest). However, the plan explicitly specifies this path and content, so it was added as required. The `-D warnings` flag in the script provides actual runtime enforcement.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Ready for 87-02-PLAN.md: Integrate quality-baseline.sh into modernization and regression scripts
- The simple script is now a stable integration point for run-modernization.sh and run-regression-checks.sh to call
- No blockers

---
*Phase: 87-wire-modernization-to-quality-policy*
*Completed: 2026-03-08*
