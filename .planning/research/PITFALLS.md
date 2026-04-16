# Domain Pitfalls

**Domain:** Brownfield whole-repo defect audit for embedded firmware + diagnostics tooling
**Project:** LibreRoaster
**Researched:** 2026-04-16
**Confidence:** MEDIUM-HIGH

## Recommended Audit Phases

1. **Phase 0 - Audit Charter, Bug Bar, and Evidence Rules**
   - Freeze what counts as a bug, how severity works, what evidence is required, and what is out of scope.
2. **Phase 1 - Repo Inventory and Observability Baseline**
   - Enumerate firmware, scripts, HIL assets, diagnostics artifacts, host/target feature gates, and known validation paths.
3. **Phase 2 - Subsystem Defect Hunt and Reproduction**
   - Investigate firmware, scripts, tooling, and planning-visible behavior separately, with reproducible cases.
4. **Phase 3 - Cross-Boundary Correlation and Integration Review**
   - Trace failures across Artisan protocol, firmware control loop, TRACE/STATUS outputs, replay scripts, and HIL artifacts.
5. **Phase 4 - Severity Ranking and Report Normalization**
   - Rank criticity consistently and turn findings into implementation-ready defect records.
6. **Phase 5 - Review, Deduplication, and Remediation Scoping**
   - Remove duplicates, challenge weak findings, and package follow-up work for a fix milestone.

## Critical Pitfalls

### Pitfall 1: Audit scope collapses into “firmware only” and misses host/tooling defects
**What goes wrong:**
The team audits `src/` deeply but barely inspects Python tooling, replay scripts, manifests, report generation, or planning-visible behavior. The final report looks thorough but misses defects that distort diagnostics or hide runtime failures.

**Why it happens:**
Firmware feels “more real” than scripts, so brownfield audits drift toward code that runs on the MCU and away from code that shapes evidence.

**Consequences:**
- False confidence in validation tooling.
- Bugs in `validation_runner.py`, replay packaging, or report generation survive into the remediation milestone.
- Follow-up fixes target symptoms in firmware instead of broken evidence pipelines.

**Prevention:**
- In **Phase 0**, define audit surfaces explicitly: firmware, host scripts, HIL tooling, artifact generation, docs/planning-visible operational behavior.
- In **Phase 1**, produce a coverage matrix mapping each repo area to an audit owner and required evidence type.
- Require the final report to tag every defect with a subsystem and integration surface.

**Detection:**
- Findings list is dominated by Rust files and ignores `scripts/`, `tests/hardware/`, and planning artifacts.
- Many defects cite source inspection only, with no artifact or tool-path validation.

### Pitfall 2: Teams treat existing diagnostics as ground truth instead of auditing the diagnostics themselves
**What goes wrong:**
TRACE, STATUS, replay bundles, and HIL reports are used as unquestioned evidence even when those systems may themselves be wrong, incomplete, or stale.

**Why it happens:**
Brownfield projects often mistake observability for truth. In LibreRoaster, recent milestones added TRACE instrumentation, manifest-aware HIL validation, and replay artifacts; that increases audit leverage, but also increases the chance of “auditing through a distorted lens.”

**Consequences:**
- Defects are ranked using faulty telemetry.
- Tool bugs get misreported as firmware bugs.
- The milestone ships a polished report with weak factual grounding.

**Prevention:**
- In **Phase 1**, validate the evidence pipeline itself: manifest -> run metadata -> telemetry CSV -> report -> replay artifact.
- Require at least one independent corroboration for critical findings: code path + test/replay evidence, or telemetry + source-level explanation.
- Mark every finding with evidence class: `source-only`, `test-backed`, `artifact-backed`, or `cross-validated`.

**Detection:**
- Critical bugs rely on a single TRACE line or a single generated report.
- The team cannot explain how a STATUS/TRACE field is produced end-to-end.

### Pitfall 3: Host-only reproduction is mistaken for target truth
**What goes wrong:**
The audit treats host tests, mock drivers, or replay scripts as enough proof for firmware-adjacent failures that may depend on timing, peripheral contention, or target-specific cfg/feature combinations.

**Why it happens:**
Host tests are faster and easier to automate. Embedded brownfield audits often over-credit them, especially when HIL runs are slower.

**Consequences:**
- Timing, watchdog, guard, and transport bugs are mis-ranked or missed.
- “Cannot reproduce” is recorded when the defect only exists on-target.
- Fix milestone inherits incorrect root causes.

**Prevention:**
- In **Phase 1**, freeze the host-vs-target matrix for every audit area.
- In **Phase 2**, require target-aware evidence for any defect touching watchdogs, LEDC guard behavior, UART/USB transport, sensor reads, or control-loop timing.
- In **Phase 4**, downgrade confidence when a finding is host-only for target-sensitive paths.

**Detection:**
- Report uses mock/test-only evidence for control-loop or hardware integration bugs.
- No distinction between `cfg(test)` behavior and device behavior.

### Pitfall 4: Integration defects are reported as isolated component bugs
**What goes wrong:**
The audit records separate bugs in parser, handler, telemetry, or scripts without identifying the cross-boundary failure chain.

**Why it happens:**
Repos are organized by component, but many brownfield defects live at handoffs: Artisan command -> parser -> control path -> actuator state -> TRACE/STATUS -> host analysis.

**Consequences:**
- Duplicate or conflicting tickets.
- Remediation milestone fixes only one side of the boundary.
- High-friction bugs return because the true contract mismatch survives.

**Prevention:**
- In **Phase 3**, require integration traces for defects that cross subsystems.
- Add a report field: `Boundary/Contract Broken`.
- Deduplicate by failure chain, not by file location.

**Detection:**
- Multiple findings share the same trigger and user-visible symptom.
- Bugs are phrased as local code smells instead of broken end-to-end behavior.

### Pitfall 5: Severity ranking is inconsistent because there is no embedded-specific bug bar
**What goes wrong:**
Teams rank defects by annoyance or implementation difficulty instead of roast-session risk, safety impact, auditability impact, and reproducibility.

**Why it happens:**
Generic bug triage scales poorly in firmware-adjacent systems where a report-generation bug, a false-safe telemetry field, and a control bug have very different consequences.

**Consequences:**
- Dangerous defects are buried under cosmetic/tooling issues.
- Evidence-quality problems are underrated even when they invalidate audit conclusions.
- Follow-up roadmap prioritizes easy fixes over risk reduction.

**Prevention:**
- In **Phase 0**, define a bug bar with explicit dimensions: safety, control correctness, data/evidence integrity, field detectability, reproducibility, and blast radius.
- In **Phase 4**, rank severity with a fixed scoring rubric and require a short rationale per finding.
- Separate `severity` from `fix effort` and `confidence`.

**Detection:**
- Report mixes “critical because hard to fix” with “critical because unsafe.”
- Similar defects receive materially different severity without explanation.

### Pitfall 6: Evidence quality is too weak for an implementation-ready report
**What goes wrong:**
Findings are real concerns but lack a reliable trigger, impacted surface, proof, or fix direction. The next milestone has to re-investigate instead of remediate.

**Why it happens:**
Brownfield audits often optimize for defect count instead of defect quality.

**Consequences:**
- Report churn during planning review.
- Engineering disputes consume the next milestone.
- The audit milestone fails its stated goal of producing remediation-ready scope.

**Prevention:**
- In **Phase 2**, use a mandatory defect template: trigger, expected behavior, observed behavior, subsystem, evidence, confidence, likely root cause, proposed fix direction.
- In **Phase 4**, reject findings that cannot pass a minimum evidence threshold.
- Keep a separate appendix for hypotheses so they do not pollute the confirmed defect list.

**Detection:**
- Findings say “likely bug” without a repro path or bounded symptom.
- Proposed fixes are generic (“refactor”, “improve handling”).

### Pitfall 7: Duplicate findings inflate the bug inventory and distort severity
**What goes wrong:**
The same underlying defect appears multiple times under different surfaces: failing test, TRACE anomaly, replay mismatch, and user-visible symptom all become separate bugs.

**Why it happens:**
Whole-repo audits naturally generate evidence from multiple layers. Without deduping by root cause, counts become meaningless.

**Consequences:**
- The report overstates defect volume.
- Prioritization becomes noisy.
- The remediation milestone burns time merging tickets.

**Prevention:**
- In **Phase 5**, run a dedup pass keyed by trigger, contract, and likely root cause.
- Track `evidence items` separately from `defects`.
- Link supporting observations under one canonical defect entry.

**Detection:**
- Several defects share the same reproduction steps and proposed fix area.
- Bug counts change dramatically after review without new evidence.

## Moderate Pitfalls

### Pitfall 8: Scope explodes into architecture review, refactor planning, or feature ideation
**What goes wrong:**
The bug-audit milestone turns into “while we are here” redesign work, cleanup planning, or feature discussion.

**Prevention:**
- In **Phase 0**, define out-of-scope categories: refactors without a defect, net-new features, speculative architecture improvements.
- Require every work item to point to a defect hypothesis or evidence gap.

### Pitfall 9: Known fragile areas are not front-loaded
**What goes wrong:**
The team spends early effort on low-risk surfaces and reaches watchdog, guard, serial transport, or replay integrity too late.

**Prevention:**
- In **Phase 1**, rank repo areas by risk: safety/control loop, transport, diagnostics pipeline, then lower-risk utilities.
- Use existing project clues (`watchdog`, `guard`, `TRACE`, HIL manifest/report flow) to seed audit order.

### Pitfall 10: Confidence is not separated from severity
**What goes wrong:**
Highly plausible bugs with thin evidence are reported beside proven defects with equal weight.

**Prevention:**
- In **Phase 4**, each defect gets both `severity` and `confidence`.
- Use confidence buckets: `confirmed`, `strong evidence`, `suspected`.
- Suspected issues should shape follow-up investigation scope, not immediate fix commitments.

### Pitfall 11: Repro steps depend on private operator knowledge
**What goes wrong:**
Audit findings make sense only to the person who investigated them. Another engineer cannot reproduce them from the report.

**Prevention:**
- In **Phase 2**, require deterministic repro instructions or explicit “non-deterministic/rare” labeling.
- Reference exact commands, files, artifacts, scenarios, and expected outputs.

### Pitfall 12: Planning-visible behavior is ignored because it is “not code”
**What goes wrong:**
Operational expectations embedded in README, HIL playbooks, manifests, and report templates drift from actual behavior, but the audit ignores that mismatch.

**Prevention:**
- In **Phase 3**, compare planning-visible contracts against implementation-visible behavior.
- Treat contract drift as a reportable defect when it can mislead validation, operation, or remediation planning.

## Minor Pitfalls

### Pitfall 13: Missing ownership for defect report curation
**What goes wrong:**
Everyone investigates, nobody normalizes. The final report has inconsistent wording, incomplete fields, and uneven severity logic.

**Prevention:**
- In **Phase 4**, assign one editor/triage owner for report normalization.

### Pitfall 14: Audit artifacts are not retained or linked
**What goes wrong:**
The report references runs or logs that are not preserved long enough to review.

**Prevention:**
- In **Phase 5**, archive the exact evidence pack used for triage and link it from each defect where possible.

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Audit charter | No shared definition of bug vs hypothesis vs tech debt | Create a bug bar and evidence gate before investigation starts |
| Repo inventory | Firmware dominates scope; scripts/tooling skipped | Build an audit coverage matrix that includes `src/`, `scripts/`, `tests/hardware/`, and planning-visible assets |
| Evidence baseline | TRACE/STATUS treated as authoritative without verification | Validate manifest -> telemetry -> report -> replay chain before using it as critical evidence |
| Firmware investigation | Host tests overused for target-sensitive defects | Require target-aware evidence for control, timing, watchdog, guard, UART/USB, and sensor paths |
| Tooling investigation | Report generation defects dismissed as “just tooling” | Rank evidence-integrity bugs by how much they can falsify triage and signoff |
| Integration review | Same failure logged as multiple component bugs | Group by broken contract and end-to-end symptom, not by file |
| Severity ranking | Criticity based on effort or annoyance | Use a rubric centered on safety, control correctness, auditability, and blast radius |
| Final report | Weak findings mixed with confirmed defects | Split confirmed defects from hypotheses and show confidence separately |
| Remediation scoping | Fix milestone inherits noisy, duplicated backlog | Deduplicate aggressively and convert each retained finding into implementation-ready scope |

## Warning Signs Checklist

- [ ] More than half the findings come from source reading only.
- [ ] Few or no findings reference `scripts/` or `tests/hardware/` despite whole-repo scope.
- [ ] Report does not distinguish severity, confidence, and fix effort.
- [ ] Multiple findings share one trigger or one user-visible symptom.
- [ ] Critical findings cannot be reproduced by another engineer from the report alone.
- [ ] HIL/replay/reporting pipeline was used as evidence but never validated as part of the audit.
- [ ] Planning-visible contracts (README, playbook, manifest, report template) were not compared to actual behavior.

## Sources

### High confidence (project-local)
- `.planning/PROJECT.md` — v5.3 scope, constraints, and goal of implementation-ready defect reporting.
- `README.md` — active integration surfaces, STATUS/TRACE expectations, and HIL workflow references.
- `tests/hardware/HIL-PLAYBOOK.md` — manifest-driven evidence workflow and artifact expectations.
- `tests/hardware/scenario_manifest.json` — scenario/evidence metadata and retention expectations.
- `tests/hardware/report_template.md` — report structure and threshold/evidence expectations.
- `.planning/codebase/INTEGRATIONS.md` — hardware/software integration surfaces relevant to defect boundaries.
- `.planning/codebase/CONCERNS.md` — known fragile areas and test gaps that bias audit risk.

### High confidence (official docs)
- Rust Reference, conditional compilation: https://doc.rust-lang.org/reference/conditional-compilation.html
- Cargo Book, features and feature combinations: https://doc.rust-lang.org/cargo/reference/features.html
- Embedded Rust Book, concurrency/shared-state hazards: https://docs.rust-embedded.org/book/concurrency/

### Confidence notes
- Repo-specific failure modes are HIGH confidence because they align with current project structure and local documentation.
- Milestone-structure recommendations are MEDIUM confidence because they are process guidance derived from embedded brownfield audit practice rather than a single normative standard.
