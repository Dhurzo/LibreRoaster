# Phase 69: Regression Helper API Alignment - Research

**Researched:** 2026-02-23
**Domain:** Safety regression API hygiene
**Confidence:** HIGH

## Summary

The milestone audit flagged `safety::regression::run_overtemp_regression` as an unused public helper. The helper is currently exported alongside `regression_task` and `request_regression`, but the only call site inside the repository lives in `regression.rs` itself. Keeping the helper on the public surface leaves an unused entry point, so this phase must collapse the helper into the module’s implementation while keeping the regression API that safety instrumentation and the boot path depend on intact.

## Findings

- `run_overtemp_regression` is defined and exported from `src/safety/regression.rs` but used only by `regression_task` in the same file (`src/safety/regression.rs:24, 28`).
- `regression_task` is spawned from `src/application/app_builder.rs` and gates the regression runner on the instrumentation channel, so it must stay public.
- `request_regression` is invoked from `src/application/tasks.rs` when automation triggers a regression, which means it also must stay public.
- `rg -n run_overtemp_regression src` currently returns matches only from the helper definition and the helper invocation inside `regression.rs`; no other modules import the helper today.

## Standard Stack

| Library | Purpose | Why Use |
|---------|---------|---------|
| Rust 1.88 | `src/safety/regression.rs` | Manages the regression helper, task, and request API in one file | Aligning the exports here keeps the change localized and avoids touching other modules.
| `embassy_executor` | Task spawning and async helpers | `regression_task` depends on `Spawner`, so the helper signature should stay compatible.
| `rg` / `ripgrep` | Search for unused exports and references | Confirm that no other module depends on `run_overtemp_regression` before making it private.

## Alternatives Considered

- **Keep the helper exported but mark it deprecated:** still leaves the unused symbol, so the audit gap remains. Deprecation also forces downstream consumers to keep referencing something the audit already flagged as redundant.
- **Expose a new helper that wraps `regression_task`:** unnecessary because `regression_task` already covers the runnable workflow, and adding another surface area risks repeating the same tech-debt finding.

## Recommendation

Privatize `run_overtemp_regression` so the regression task retains exclusive access, then trim the `pub use target_impl` list to the two real consumers. After the change, rerun `rg -n run_overtemp_regression src` to prove the helper has no additional callers.

## Next Steps

1. Update `src/safety/regression.rs` to change `pub async fn run_overtemp_regression` into a private `async fn` while keeping the existing `Spawner` parameter and call site intact.
2. Limit `pub use target_impl::{...}` to `regression_task` and `request_regression` only.
3. Run `rg -n run_overtemp_regression src` to confirm no other references remain.

## Sources

- `src/safety/regression.rs` (helper definition and exports)
- `src/application/app_builder.rs` (spawns the regression task)
- `src/application/tasks.rs` (calls `request_regression`)
- `.planning/v4.1-MILESTONE-AUDIT.md` (tech-debt gap description)
