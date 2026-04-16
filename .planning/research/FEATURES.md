# Feature Landscape

**Domain:** Brownfield embedded/firmware defect analysis and implementation-ready defect reporting
**Researched:** 2026-04-16
**Project:** LibreRoaster v5.3 Deep Bug Analysis & Defect Report
**Confidence:** MEDIUM-HIGH

## Table Stakes

Features users expect from a serious firmware-adjacent defect-audit milestone. Missing these usually turns the output into a vague bug brainstorm instead of a usable remediation backlog.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **Whole-repo investigation coverage map** | A brownfield audit is only credible if it states what was inspected: firmware runtime paths, host scripts, validation tooling, and planning-visible behavior. | MEDIUM | Required behavior: declare in-scope surfaces, depth of review per surface, and known blind spots. Dependency: current repo structure and milestone scope in `.planning/PROJECT.md`. |
| **Evidence-backed finding standard** | Rigorous audits do not report "likely bugs" without proof. Each finding needs code pointer(s), failure mode, trigger conditions, and why the behavior is wrong. | MEDIUM | Required minimum evidence: file/line or artifact reference, observed or reasoned failure chain, affected subsystem, and confidence level. Prefer replay/HIL/TRACE artifacts when available. |
| **Criticity model tuned for firmware operations** | Embedded defects are not just "severity" in the web-app sense; they need safety/control impact, operator impact, and recovery implications. | MEDIUM | Recommend 4 levels: Critical, High, Medium, Low. Score using impact to safety/control, likelihood/reachability, and detectability/containment. Do **not** use pure CVSS as the primary model for non-security defects. |
| **Implementation-ready defect record** | The output milestone succeeds only if a follow-up fix milestone can pick up a bug and act without redoing discovery work. | MEDIUM | Each record should include: problem statement, why it is a bug, evidence, suspected root cause, fix direction, affected files/components, validation needed after fix, and dependencies/risks. |
| **Structured report with decision-friendly sections** | Teams need to triage quickly: exec summary first, defect inventory second, detailed findings third. | LOW-MEDIUM | Required sections: scope/method, coverage summary, criticity rubric, finding inventory table, detailed findings, false-positive/needs-validation list, deferred items, and recommended remediation slicing. |
| **Validation expectation per finding** | Firmware bugs often look fixed on host-side reasoning but fail on target timing or hardware paths. The report must say how to confirm the eventual fix. | MEDIUM | Required behavior: every finding names the cheapest valid proof after remediation: unit test, integration test, TRACE replay, diagnostics replay, HIL rerun, or manual hardware check. |
| **Explicit milestone boundaries** | Brownfield audit milestones fail when they drift into opportunistic refactors and half-fixes. | LOW | Required behavior: separate confirmed bugs from code smells, future enhancements, and speculative redesigns. This milestone produces defect reports, not merged fixes. |

## Differentiators

Useful but not strictly required for milestone closeout. These materially improve trust, triage speed, and follow-up planning quality.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Cross-artifact correlation (code -> TRACE -> replay/HIL -> docs)** | Stronger than a code-only audit because it can prove that defects cross firmware, tooling, and operator-facing evidence chains. | HIGH | Best fit for LibreRoaster because TRACE instrumentation, replay artifacts, and HIL playbooks already exist. |
| **Finding confidence label** | Distinguishes confirmed defects from likely defects and review-only hypotheses, reducing false urgency. | LOW | Recommend labels: Confirmed / Strongly Suspected / Needs Runtime Confirmation. |
| **Root-cause clustering across findings** | Prevents follow-up milestones from fixing symptoms one by one when several bugs share the same queue, state, parsing, or error-taxonomy cause. | MEDIUM | Useful output: "shared failure family" tags such as control-loop timing, protocol parsing, artifact drift, validation mismatch, unsafe defaults. |
| **Fix sequencing hints** | Makes the follow-up remediation milestone easier to plan by grouping defects into low-risk-first slices. | MEDIUM | Example groups: report-only/tooling, host-side scripts, deterministic firmware logic, hardware/timing-sensitive defects. |
| **False-positive quarantine section** | Preserves useful suspicions without polluting the confirmed bug backlog. | LOW | Important in embedded audits where not every suspicious path can be reproduced immediately on target hardware. |
| **Machine-readable defect inventory** | Lets later phases sort/filter by subsystem, criticity, and validation type. | MEDIUM | Optional enhancement: emit JSON/CSV alongside Markdown, but Markdown remains the authoritative narrative artifact. |

## Anti-Features

Features to explicitly NOT build in this milestone.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **Mix bug analysis with direct remediation** | Hides investigation quality, muddies blame for regressions, and breaks the milestone's stated goal. | Stop at implementation-ready fix descriptions and validation guidance. Schedule actual fixes in the next milestone. |
| **Speculative bug lists without evidence** | Creates churn and distrust; maintainers cannot act on hand-wavy findings. | Require evidence + confidence labels; move weaker items to "needs validation." |
| **Architecture rewrite recommendations disguised as bug fixes** | Brownfield audits often overreach into redesign. That expands scope and delays useful output. | Keep recommendations local and minimal unless a repeated root cause clearly proves a boundary problem. |
| **Criticity inflation** | If everything is high severity, nothing is prioritized. | Use a narrow rubric tied to safety/control impact, customer-visible effect, and containment. |
| **Treating style/lint issues as defects by default** | Noise overwhelms real runtime, protocol, or evidence-chain bugs. | Track non-functional cleanup separately unless it directly causes incorrect behavior or masks failures. |
| **Requiring on-hardware reproduction for every finding** | Too expensive for a whole-repo audit and will suppress real script/tooling/reporting defects. | Accept static/reasoned findings when evidence is strong; reserve HIL for hardware-sensitive or timing-sensitive issues. |
| **Generating a giant flat report with no triage structure** | Makes the output unreadable and unusable for milestone planning. | Use summary tables plus detailed per-finding records. |

## Feature Categories for This Milestone

These are the concrete behavior categories downstream roadmap work should preserve.

### 1. Investigation Coverage

**Required practices**
- Cover firmware control/runtime logic, host scripts, validation tooling, diagnostics/replay tooling, and planning-visible behavior/documentation drift.
- Distinguish inspected surfaces from lightly sampled surfaces.
- Record blind spots explicitly: hardware-only paths not exercised, environment-dependent scripts not run, assumptions derived from static review only.

**Optional enhancements**
- Subsystem heatmap by risk and evidence depth.
- Reviewer notes on "high churn / high coupling / low test visibility" zones.

### 2. Evidence Standards

**Required practices**
- Every bug gets direct evidence: code location, artifact reference, or deterministic reasoning chain.
- Evidence should be reproducible where practical: command transcript, TRACE line family, replay artifact mismatch, failing test path, or documented scenario.
- Separate facts from inference.

**Optional enhancements**
- Attach minimal repro steps.
- Include screenshots/log excerpts only when they add signal over structured references.

### 3. Severity / Criticity Model

**Required practices**
- Use a defect criticity model that fits embedded systems, not only security disclosures.
- Recommended rubric:
  - **Critical** — can create unsafe behavior, uncontrolled actuation, loss of shutdown/guard guarantees, or corrupt trusted evidence for safety decisions.
  - **High** — breaks core roast control, telemetry correctness, protocol correctness, or audit/replay workflows in realistic use.
  - **Medium** — degrades diagnostics, creates misleading outputs, weakens validation confidence, or breaks less common but supported flows.
  - **Low** — real defect with bounded impact, low reachability, or easy operator detection/recovery.
- Score using three axes: **impact**, **reachability/likelihood**, **detectability/containment**.

**Optional enhancements**
- Add a separate **confidence** field so high-risk hypotheses do not masquerade as confirmed critical bugs.
- Add a **fix-cost** hint, but keep it secondary to criticity.

### 4. Report Structure

**Required practices**
- Start with an inventory table: ID, title, subsystem, criticity, confidence, validation type, proposed fix owner/area.
- Detailed entries should include:
  1. bug statement
  2. affected components/files
  3. why this is wrong
  4. evidence
  5. trigger/preconditions
  6. impact
  7. suspected root cause
  8. implementation-ready fix direction
  9. validation needed after fix
  10. dependencies / blockers / open questions
- Include a separate section for non-bug observations and deferred concerns.

**Optional enhancements**
- Machine-readable sidecar inventory.
- Root-cause clusters and suggested remediation batches.

### 5. Validation Expectations

**Required practices**
- The report must specify how each eventual fix should be validated.
- Validation should use the cheapest proof that still matches the risk:
  - host/unit/integration tests for deterministic logic
  - TRACE/replay verification for diagnostics and evidence-chain defects
  - HIL/manual hardware validation for timing, actuation, guard, or telemetry-on-target defects
- Findings that cannot be reproduced now should still name the required confirmation step.

**Optional enhancements**
- Map findings to existing HIL scenarios or replay artifacts that can be reused.
- Recommend new regression assets only where coverage is clearly missing.

### 6. Boundaries

**Required practices**
- Keep the milestone centered on detection, ranking, and remediation planning.
- Defer actual code changes, architecture overhauls, and broad quality-program work.
- Record when a finding depends on hardware access, environment setup, or future repro work.

**Optional enhancements**
- Propose remediation slices for the next milestone.
- Identify candidate bugs that should be bundled into a dedicated reliability or diagnostics follow-up.

## Feature Dependencies

```text
Existing project context (already built):
  TRACE instrumentation
  Diagnostics replay artifacts
  Manifest-aware HIL validation workflow
  Embedded diagnostics + unified error taxonomy
  Existing firmware/script/tooling/planning surfaces

[Whole-repo investigation coverage]
    └──requires──> [Current subsystem map and explicit scope boundaries]

[Evidence-backed findings]
    └──depends on──> [TRACE logs / replay artifacts / tests / code references]
    └──depends on──> [Ability to cite planning-visible expected behavior]

[Criticity model]
    └──depends on──> [Understanding of safety/control importance]
    └──depends on──> [Known operator-facing impact paths]

[Implementation-ready defect record]
    └──depends on──> [Subsystem ownership or at least affected-component clarity]
    └──depends on──> [Validation path selection per finding]

[Validation expectations]
    └──reuse──> [HIL playbook]
    └──reuse──> [TRACE/replay tooling]
    └──reuse──> [Existing tests where sufficient]
```

### Dependency Notes

- **TRACE instrumentation is a major accelerator** for proving cross-boundary bugs; use it as primary evidence where runtime flow matters.
- **Replay artifacts matter for evidence-chain defects**; if a bug is in diagnostics or reporting fidelity, replay/metadata comparison is stronger than prose-only reasoning.
- **HIL should be selective, not universal**; use it for actuation/timing/telemetry correctness, not as a blanket prerequisite for every reported defect.
- **Planning-visible behavior is in scope** because this milestone explicitly includes reportability and auditability across tooling and planning artifacts, not firmware alone.

## MVP Recommendation

Prioritize:
1. **Whole-repo coverage declaration** — prove the audit is systematic.
2. **Evidence-backed per-finding record format** — make the output actionable.
3. **Firmware-appropriate criticity + confidence model** — make prioritization credible.
4. **Validation expectation per finding** — prevent the follow-up fix milestone from guessing how to verify changes.
5. **Strict milestone boundary enforcement** — keep analysis work from becoming a mixed fix/refactor milestone.

Defer:
- **Actual bug remediation** — next milestone.
- **Large new automation frameworks** — only recommend when the report proves a coverage gap.
- **Broad architectural cleanup programs** — only justify later if multiple findings share one structural root cause.
- **Formal external vulnerability disclosure workflow** — out of scope unless the audit uncovers true security vulnerabilities needing coordinated handling.

## Sources

Project context (HIGH confidence):
- `.planning/PROJECT.md` — milestone goal, scope, and out-of-scope definition.
- `tests/hardware/HIL-PLAYBOOK.md` — existing artifact/evidence expectations for reproducible validation.
- `tests/hardware/report_template.md` — current report structure for auditor-facing validation output.
- `replay-report.json` — example of structured replay evidence with explicit metadata matching.

Official / authoritative references (MEDIUM-HIGH confidence):
- NIST SP 800-218 SSDF — emphasizes reducing vulnerabilities, mitigating undetected issues, and addressing root causes: https://csrc.nist.gov/pubs/sp/800/218/final
- FIRST CVSS v4.0 — useful as a reference for transparent scoring dimensions, but not sufficient alone for firmware defect criticity: https://www.first.org/cvss/v4.0/specification-document
- MITRE CWE List v4.19.1 — useful for consistent weakness/root-cause taxonomy across software and hardware-adjacent findings: https://cwe.mitre.org/data/index.html
- SEI CERT C Coding Standard — supports rule/recommendation framing, analyzers, and risk-oriented reasoning for C/C++-adjacent code and embedded tooling: https://wiki.sei.cmu.edu/confluence/display/c/SEI+CERT+C+Coding+Standard

Confidence notes:
- Google Search was unavailable in this environment, so broader ecosystem trend claims are based on project artifacts, official standards, and established embedded review practice rather than a large 2026 community survey.
- The recommendations above are **high confidence for brownfield firmware audit mechanics**, but **medium confidence for ecosystem popularity claims** because broad web survey tooling was not available.
