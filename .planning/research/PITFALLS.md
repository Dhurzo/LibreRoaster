# Domain Pitfalls: v5.0 Quality Hardening (Embedded Rust Firmware)

**Domain:** Brownfield ESP32-C3 firmware (Embassy + Artisan control path)
**Project:** LibreRoaster
**Researched:** 2026-03-07
**Confidence:** MEDIUM-HIGH

## Phase Model Used for Prevention

1. **Phase 0 - Baseline & Invariants Freeze**
   - Freeze behavioral contracts (Artisan command semantics, safety telemetry, watchdog/guard expectations)
2. **Phase 1 - Dead Code Audit & Controlled Elimination**
   - Evidence-based catalog, tombstone removals, fast rollback
3. **Phase 2 - Rust Modernization (Mechanical First)**
   - Lints/edition/API-boundary improvements without behavior changes
4. **Phase 3 - SOLID Refactor in Hot Paths**
   - Component boundaries + loop-budget and safety gates
5. **Phase 4 - Hardware Validation Readiness (HIL Preflight)**
   - Instrumentation, command/actuator correlation, failure-injection readiness
6. **Phase 5 - Real Hardware Validation & Signoff**
   - Artisan Scope -> firmware -> actuator proof on real roaster path

---

## Critical Pitfalls

### Pitfall 1: "Dead" Code Removal Breaks Linker/Runtime Side-Effects

**What goes wrong:**
Code flagged as unused is removed, but it carried required linker/runtime behavior (`#[used]`, linker sections, drop side-effects, startup registration), causing missing symbols, silent behavior changes, or boot/runtime instability.

**Why it happens:**
In brownfield firmware, "not referenced in Rust call graph" does not mean "safe to remove".

**How to avoid:**
- Split dead-code candidates into: `provably dead`, `externally reachable`, `side-effectful`.
- For every candidate, require one evidence artifact: symbol map diff, protocol trace diff, or test proof.
- Ban direct deletion of items with `unsafe(no_mangle)`, `unsafe(link_section)`, `#[used]` unless explicitly reviewed.

**Warning signs:**
- New linker errors about missing symbols after cleanup.
- Behavior differs only in release/LTO builds.
- Startup/reset path regressions despite "pure cleanup" PR.

**Phase to address:**
**Phase 1 - Dead Code Audit & Controlled Elimination**

---

### Pitfall 2: Removing "Unused" Artisan Paths That Are Required by Real Sessions

**What goes wrong:**
Handshake or parser branches that seem unused in unit tests are removed, and Artisan Scope stops controlling heater/fan reliably in real sessions.

**Why it happens:**
Host tests cover nominal command subsets; real tools often rely on sequence/timing quirks and expected response formats.

**How to avoid:**
- Capture and freeze canonical command transcripts (connect, init, control, stop, error).
- Gate removal on transcript replay tests and exact response-shape assertions.
- Keep parser/formatter compatibility checks for existing command set before and after cleanup.

**Warning signs:**
- "Works in tests, fails in Artisan Scope" reports.
- Increased parser recovery logs or more `ERR` responses under normal operation.
- Session starts but manual commands no longer map predictably to actuator output.

**Phase to address:**
**Phase 0** and **Phase 1**

---

### Pitfall 3: SOLID Refactor Fragments Safety-Critical Authority

**What goes wrong:**
Refactor introduces clean interfaces but duplicates or splits authority over manual heater/fan/safety state, causing divergence between handler state and real actuator output.

**Why it happens:**
SOLID is applied as a structural goal without preserving existing invariants in command ownership.

**How to avoid:**
- Define invariant docs before refactor: who is source-of-truth for manual setpoints and safety flags.
- Enforce single-writer rules per critical field (`manual_heater`, `manual_fan`, emergency/fault flags).
- Add contract tests at handler boundary and post-apply actuator boundary.

**Warning signs:**
- Two components mutate same safety/manual fields.
- Integration tests pass only when execution order is unchanged.
- Manual command handling logic appears in more than one handler path.

**Phase to address:**
**Phase 3 - SOLID Refactor in Hot Paths**

---

### Pitfall 4: Performance Regressions from Over-Abstraction in 100 ms Loop

**What goes wrong:**
Refactor adds extra indirection, allocations, or logging overhead in control path; loop jitter increases and watchdog/safety timing assumptions erode.

**Why it happens:**
Code quality improvements focus on readability while treating timing as secondary.

**How to avoid:**
- Introduce loop budget SLO (per tick) and track before/after timing.
- Keep hot path allocation-free and avoid new trait-object churn in tick path.
- Enforce release-build performance checks (debug results are not enough).

**Warning signs:**
- Watchdog near-miss counters rise after "refactor-only" changes.
- Control cadence drifts under serial traffic.
- Regressions appear only with `--release` + LTO.

**Phase to address:**
**Phase 3**, validated again in **Phase 4**

---

### Pitfall 5: Modernization Changes Error Semantics Needed for Safety Decisions

**What goes wrong:**
Error cleanup unifies/simplifies error types but loses distinctions needed for safe fallback (sensor fault vs transient IO vs actuator guard timeout).

**Why it happens:**
"Cleaner" `Result` hierarchy is treated as purely cosmetic.

**How to avoid:**
- Preserve error classes that map to different operational responses.
- Add explicit conversion policy documenting which errors are recoverable vs fail-safe.
- Include safety action assertions in tests (shutdown, clamp, retry, degrade).

**Warning signs:**
- Multiple distinct failures now map to same generic error.
- Emergency behavior triggers less often after refactor.
- Logs become less specific exactly where previous incidents required granularity.

**Phase to address:**
**Phase 2 - Rust Modernization**

---

### Pitfall 6: Dead Code Elimination Removes Observability Hooks Needed for Validation

**What goes wrong:**
Telemetry/status fields, diagnostics commands, or instrumentation paths are deleted as "non-functional," making hardware validation impossible or non-auditable.

**Why it happens:**
Quality work is scoped to runtime behavior, not validation observability requirements.

**How to avoid:**
- Mark validation-critical telemetry as protected API surface for milestone duration.
- Require observability parity checklist before merge.
- Keep protocol-level status snapshots stable until HIL signoff is complete.

**Warning signs:**
- Fewer safety counters/fields available after cleanup.
- Regression harness can no longer prove guard/watchdog interactions.
- Hardware test plans require manual interpretation instead of machine-readable evidence.

**Phase to address:**
**Phase 0** and **Phase 1**

---

### Pitfall 7: Hardware Validation Readiness Skipped (Only Host/Mock Confidence)

**What goes wrong:**
Roadmap claims "ready for real hardware" based on host tests only; physical actuator behavior (latency, clamping, guard conflicts) is not actually verified.

**Why it happens:**
Host CI is fast and deterministic; HIL setup is slower and often deferred.

**How to avoid:**
- Add preflight gate: no real-roaster testing until command-to-actuator evidence path is instrumented.
- Define minimal HIL matrix: command class x expected actuator/telemetry response.
- Require at least one failure-injection run (sensor fault, emergency stop, comms disturbance).

**Warning signs:**
- "Ready" stated without captured serial + actuator evidence.
- Hardware validation plans lack pass/fail thresholds.
- First real-device run reveals timing/ordering bugs absent in host tests.

**Phase to address:**
**Phase 4 - HIL Preflight**

---

### Pitfall 8: Unsafe Modernization Sweep Expands Risk Surface

**What goes wrong:**
During modernization, unsafe/linker attributes and low-level sections are mass-updated without per-item safety review; subtle ABI/link behavior changes occur.

**Why it happens:**
Edition/lint migration is applied mechanically to embedded-specific constructs.

**How to avoid:**
- Treat unsafe-attribute updates (`unsafe(no_mangle)`, `unsafe(link_section)`, `unsafe(export_name)`) as safety-reviewed changes.
- Require one-line SAFETY rationale at each site.
- Diff symbol table and section placement when touching these attributes.

**Warning signs:**
- Large automated migration touching startup/interrupt/linker-adjacent files.
- New symbols exported/unexported unexpectedly.
- Runtime changes without corresponding logic diffs.

**Phase to address:**
**Phase 2 - Rust Modernization**

---

## Moderate Pitfalls

### Pitfall 9: Clippy/Lint "Fixes" Introduce Embedded-Unfriendly Behavior

**What goes wrong:**
Automatic lint fixes introduce less deterministic patterns (extra formatting, hidden copies, less explicit timing or ownership behavior).

**How to avoid:**
- Use lint tiers: allow/warn/deny by module criticality.
- Ban bulk `cargo fix` across control/safety modules without focused review.

**Warning signs:**
- Large mechanical diffs in control modules with no behavioral tests added.

**Phase to address:**
**Phase 2**

---

### Pitfall 10: Feature-Gate Drift Between Host Tests and Firmware Build

**What goes wrong:**
Code looks dead or safe under host features but is active on target (or vice versa), creating false confidence in cleanup/refactor outcomes.

**How to avoid:**
- Maintain a feature matrix with required test jobs for each gate set.
- Review dead-code candidates per target/feature combination.

**Warning signs:**
- Cleanup PR passes host tests but fails target build or behaves differently on-device.

**Phase to address:**
**Phase 1** and **Phase 4**

---

### Pitfall 11: Command/Actuator Correlation Missing in Logs

**What goes wrong:**
You can see command logs and actuator states separately, but cannot prove causality or latency per command.

**How to avoid:**
- Introduce correlation IDs/timestamps across parser, handler, apply, and telemetry output.
- Store compact event tuples suitable for post-run analysis.

**Warning signs:**
- Team debates "did command X actually produce actuator Y?" with no definitive trace.

**Phase to address:**
**Phase 4**

---

### Pitfall 12: No Rollback Path for Aggressive Cleanup/Refactor Batches

**What goes wrong:**
Multiple risky changes land together; when hardware regression appears, rollback is slow and root cause is unclear.

**How to avoid:**
- Keep high-risk changes in small, reversible slices.
- Tag each slice with explicit rollback plan and known-good benchmark.

**Warning signs:**
- PRs combine dead-code deletion + API redesign + behavior change.

**Phase to address:**
**All phases (execution policy)**

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Remove "unused" branches without evidence artifact | Faster cleanup | Hidden protocol/safety regressions | Never |
| Bulk modernization in hot path | Quick lint score improvement | Timing regressions, safety drift | Never |
| Defer HIL preflight until end | Faster early velocity | Late discovery of hardware-only failures | Only for non-control modules |
| Replace specific errors with generic catch-all | Simpler signatures | Wrong fail-safe behavior | Rarely, and not in safety path |
| Refactor multiple safety boundaries in one PR | Cleaner architecture sooner | Hard to debug/regressions expensive | Never |

## Integration Gotchas (LibreRoaster-Specific)

| Integration Surface | Common Mistake | Correct Approach |
|---------------------|----------------|------------------|
| Artisan command handling (`process_artisan_command` + handler chain) | Re-introduce duplicate manual setpoint logic outside authoritative handler | Keep one source of truth and test command -> handler -> actuator path end-to-end |
| Safety telemetry and STATUS path | Treat observability fields as optional during cleanup | Freeze validation fields through Phase 5 signoff |
| SSR guard + PID/manual apply helpers | Refactor without preserving guard feedback timing and clamp semantics | Add invariant tests that assert guard busy/timeout behavior before and after refactor |
| Host stubs vs target hardware | Trust host-only behavior for go/no-go | Require target-preflight matrix and real command replay |

## Performance Traps (100 ms Control Loop Context)

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Added indirection/allocation in tick path | Jitter, watchdog near-miss, delayed actuator update | Allocation-free hot path policy; release timing gate | Under combined serial + control load |
| Over-logging in control/safety loop | Timing drift in release with logs enabled | Structured low-overhead logs + sampling strategy | During sustained command bursts |
| Safety checks moved behind async/indirect calls | Non-deterministic emergency reaction latency | Keep fail-safe checks on direct path with bounded work | During fast fault transitions |

## "Looks Done But Isn't" Checklist

- [ ] **Dead-code pass:** Every deletion has evidence (map diff, transcript diff, or test) and target-feature review.
- [ ] **Modernization pass:** No bulk mechanical edits in safety/control modules without behavior-lock tests.
- [ ] **SOLID refactor:** Single-writer invariants for manual/safety state are documented and verified.
- [ ] **HIL readiness:** Command-to-actuator correlation is measurable and archived.
- [ ] **Hardware signoff:** Real Artisan Scope session includes nominal + fault-injection evidence.

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Protocol/control regression after cleanup | HIGH | Revert last risky slice, replay transcript suite, reintroduce guarded path with deprecation marker |
| Timing regression in loop | MEDIUM-HIGH | Restore previous hot path, collect release timing trace, re-apply refactor in smaller steps |
| Lost observability for validation | MEDIUM | Re-add telemetry hooks, regenerate status contract tests, rerun HIL preflight |
| Safety-semantic error collapse | HIGH | Restore typed error classes, rebind fail-safe actions, rerun emergency/fault scenarios |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Linker/side-effect dead code deletion | Phase 1 | Symbol/section diff + target build + boot/run smoke |
| Artisan branch false-positive dead code | Phase 0 + 1 | Recorded transcript replay + response shape assertions |
| SOLID authority split | Phase 3 | Single-writer contract tests + integration flow test |
| Hot-path abstraction regressions | Phase 3 + 4 | Release loop timing gate + watchdog telemetry trend |
| Error semantic collapse | Phase 2 | Error-class to safety-action mapping tests |
| Observability hook deletion | Phase 0 + 1 | STATUS/telemetry parity checklist |
| HIL readiness skipped | Phase 4 | Preflight matrix complete before on-roaster run |
| Real hardware validation gaps | Phase 5 | Evidence pack: commands, actuator traces, safety events |

## Sources

### High confidence (official docs/specs)
- Rust Reference - `used`, `no_mangle`, `link_section`, `export_name`: https://doc.rust-lang.org/reference/abi.html#the-used-attribute
- Rust Edition Guide (2024 unsafe attributes): https://doc.rust-lang.org/edition-guide/rust-2024/unsafe-attributes.html
- rustc codegen options (`lto`, dead-code linking behavior, panic strategy): https://doc.rust-lang.org/rustc/codegen-options/index.html
- rustc lint docs (`dead_code` limitations): https://doc.rust-lang.org/rustc/lints/listing/warn-by-default.html#dead-code
- Embedded Rust Book (concurrency, critical sections, shared state hazards): https://docs.rust-embedded.org/book/concurrency/
- Embassy executor docs (static tasks, no heap model): https://docs.embassy.dev/embassy-executor/0.9.1/
- esp-hal docs (`no_std`, feature stability caveats, `mem::forget` warning): https://docs.rs/esp-hal/latest/esp_hal/

### High confidence (project-local evidence)
- Project milestone context and active v5.0 goals: `.planning/PROJECT.md`
- Handler/control ownership and actuator application flow: `src/control/roaster_refactored.rs`
- Command handler state authority details: `src/control/handlers.rs`

### Medium confidence (community-maintained guidance)
- Embassy FAQ (release/LTO gotchas, dead-code/link behavior during setup, resource usage practices): https://github.com/embassy-rs/embassy/blob/main/docs/pages/faq.adoc

---
*Pitfalls research for: LibreRoaster v5.0 quality hardening (dead code elimination, Rust modernization, SOLID refactors, real hardware validation)*
*Researched: 2026-03-07*
