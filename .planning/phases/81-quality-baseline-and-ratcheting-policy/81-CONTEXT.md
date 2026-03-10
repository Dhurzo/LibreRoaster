# Phase 81: Quality Baseline and Ratcheting Policy - Context

**Gathered:** 2026-03-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Define a reproducible quality-gate baseline workflow and an explicit module-criticality ratcheting policy so users can run deterministic format/lint/test gates, see stricter treatment for higher-risk modules, and receive actionable module+rule failure signals. This phase clarifies behavior and policy expression only; it does not add new product capabilities.

</domain>

<decisions>
## Implementation Decisions

### Baseline run flow
- Provide a single orchestrator command as the primary baseline execution path.
- Baseline run order strictness is at Claude's discretion during planning.
- Default run output is compact summary style (concise pass/fail visibility).
- Determinism evidence is terminal output only; no required persisted run artifact in this phase.

### Criticality tiers
- Tier count is at Claude's discretion (choose explicit tier structure during planning).
- Exact top-tier module set is at Claude's discretion (derive concrete module mapping during planning).
- Lower-risk modules start as informational-only (issues reported without initial blocking behavior).
- Policy presentation must include both domain-level policy and explicit module-to-tier mapping.

### Failure output style
- Failure headline focus (rule-first/module-first/gate-first) is at Claude's discretion.
- Remediation guidance depth is at Claude's discretion.
- A failed run should list all failures rather than stopping at the first failure.
- Failure output must include both inline policy context and a policy identifier reference.

### Rerun and ratchet behavior
- After fixes, expected workflow is full baseline rerun.
- Reproducibility messaging should explicitly emphasize "same input, same verdict."
- Ratchet cadence is at Claude's discretion (define update cadence during planning).
- Ratchet changes must be visible via both policy version bump and human-readable delta/changelog.

### Claude's Discretion
- Baseline gate ordering policy details.
- Number of strictness tiers and exact top-tier module membership.
- Failure headline structure and remediation guidance verbosity.
- Ratchet application cadence over time.

</decisions>

<specifics>
## Specific Ideas

- Keep operator feedback concise during normal baseline runs.
- Keep deterministic verification lightweight (terminal-first, no mandatory artifact file).

</specifics>

<deferred>
## Deferred Ideas

None - discussion stayed within phase scope.

</deferred>

---

*Phase: 81-quality-baseline-and-ratcheting-policy*
*Context gathered: 2026-03-07*
