# Phase 84: SOLID Seam Hardening and Fault Injection - Research

**Researched:** 2026-03-07
**Domain:** Embedded safety hardening for the ESP32-C3 control loop (Embassy async + Artisan protocol)
**Confidence:** MEDIUM-HIGH

## Summary

Phase 84 must lock down the seams between command handlers, hardware actuation, and safety instruments so that SOLID-oriented cleanups can happen without disturbing timing or authority. The existing architecture already routes Artisan commands through `ServiceContainer` channels, a 100 ms stage-tracked control loop, and a tightly ordered apply/guard/watchdog sequence; this phase needs to preserve those boundaries while peeling out policy/hardware responsibilities and adding hard proofs.

This work also formalises the fault-injection requirements (watchdog, guard, communications) by treating the host/hardware instrumented evidence pack as the enforcement mechanism. The standard stack is the same Embassy/esp-hal/embedded-hal runtime plus the host-side harness (`serialport` + `csv`) and quality gates (`cargo-nextest`, `cargo-udeps`, `cargo-geiger`) that already accompany the v5.0 roadmap, so nothing new needs to be introduced from scratch.

Putting it together: tighten the handler/hardware seams through ports-and-policies traits, keep the 100 ms loop deterministic by not adding indirection inside `control_loop_task`, and capture fault-injection traces through the existing STATUS/telemetry hooks and host harness so watchdog/guard/comms scenarios can be replayed and audited.

- Handler/hardware authority already lives in `src/control/roaster_refactored.rs`/`handlers.rs` and must stay the single writer for manual outputs and safety flags while seam extraction happens around it.
- The stage-tracked control loop (`src/application/tasks.rs`) already records sensor → control → actuator → watchdog → telemetry ordering; keep that state machine intact when pulling out new handlers or instrumentation.
- Fault injection should reuse the approved evidence path (`tests/hardware/…` with `serialport`/`csv`) plus the `STATUS` payload that exposes watchdog/guard metadata rather than building bespoke tooling.

**Primary recommendation:** Preserve the stage-ordered control loop and handler authority while documenting/validating each fault-injection scenario through the existing instrumentation (STATUS telemetry + host harness) so SOLID seam hardening can be audited without new runtime risk.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `embassy-executor` | `0.9.1` | Deterministic async runtime for the ESP32-C3 control loop | Official docs describe static task allocation, fairness, and low-overhead timers that keep the 100 ms loop deterministic and prevent heap panics, which is critical when preserving fault-ordering guarantees ([docs.embassy.dev/embassy-executor/0.9.1/](https://docs.embassy.dev/embassy-executor/0.9.1/)). |
| `embassy-sync` | `0.7.2` | Async channels/mutexes that glue handlers, sensors, and telemetry together | Provides the `Channel`, `Mutex`, and watch primitives already used by `ServiceContainer` for safe pipelining—sanctioned by the Embassy project for embedded communication ([docs.embassy.dev/embassy-sync/](https://docs.embassy.dev/embassy-sync/)). |
| `esp-hal` | `1.0.0` | ESP32-specific hardware abstraction (GPIO, PWM, watchdog, USB CDC) | The official esp-hal book emphasises idiomatic, safe peripheral singletons and `no_std` drivers, which is what the firmware already relies on for heater/fan guard timeouts and watchdog handling ([docs.rs/esp-hal/latest/esp_hal/](https://docs.rs/esp-hal/latest/esp_hal/)). |
| `embedded-hal` | `1.0.0` | Platform-agnostic traits under the HAL layer (PWM, SPI, delay) | The HAL team’s mission is to erase device specificity so control code can stay SOLID; `process_artisan_command` already depends on these traits for sensors/fans ([docs.rs/embedded-hal/latest/embedded_hal/](https://docs.rs/embedded-hal/latest/embedded_hal/)). |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `serialport` | `4.8.1` | Host harness that opens USB CDC/UART ports for evidence capture | Use in hardware validation harnesses (`tests/hardware/...`) to record command/telemetry timelines for each fault/instrumentation run ([.planning/research/STACK.md](.planning/research/STACK.md)). |
| `csv` | `1.4.0` | Structured evidence artifact format | Emit instrumentation timelines to machine-readable files so auditors can replay what happened during watchdog/guard faults ([.planning/research/STACK.md](.planning/research/STACK.md)). |
| `cargo-nextest` | `0.9.129` | Host test orchestration with retries and JUnit output | Gate SOLID refactors behind deterministic host automation before touching hardware ([.planning/research/STACK.md](.planning/research/STACK.md)). |
| `cargo-udeps` | `0.1.60` | Rare but high-signal unused dependency sweeps | Runs as a nightly gate so dependency cleanup for SOLID seams stays honest ([.planning/research/STACK.md](.planning/research/STACK.md)). |
| `cargo-geiger` | `0.13.0` | Track unsafe surface while refactoring | Prevents SOLID-driven changes from accidentally expanding unsafe areas in safety-critical modules ([.planning/research/STACK.md](.planning/research/STACK.md)). |
| `embedded-hal-mock` | `0.11.1` | Host-side mock hardware for watchdog/guard/comms failure injection | Allows command paths to be exercised without real hardware and still exercise the same traits used in production ([docs.rs/embedded-hal-mock/latest/embedded_hal_mock/](https://docs.rs/embedded-hal-mock/latest/embedded_hal_mock/)). |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `serialport` + `csv` evidence harness | Manual Artisan screenshots/logs | Faster but non-reproducible; fails to provide the structured `STATUS`/telemetry artifacts required by the roadmap audit (`.planning/research/STACK.md`). |
| `cargo-nextest` | Plain `cargo test` | Simpler local loop but lacks partitioned retries and machine-readable reports critical for gating SOLID refactors. |
| `cargo-udeps` | `cargo-machete` only | Missing some unused dependency cases; `machete` stays in the loop, but `udeps` is still needed for deep audits. |

**Installation:**
```bash
cargo +stable install --locked cargo-nextest@0.9.129
cargo +stable install --locked cargo-machete@0.9.1
cargo +nightly install --locked cargo-udeps@0.1.60
cargo +stable install --locked cargo-geiger@0.13.0
```

Add `serialport = "4.8.1"` and `csv = "1.4.0"` to the host validation harness dev-dependencies so that the hardware evidence runner can compile alongside the firmware. Keep `embedded-hal-mock` (`eh1` feature) in `dev-dependencies` to simulate watchdog/feed failures without hardware.

## Architecture Patterns

### Recommended Project Structure
```
.planning/
├── quality/            # Dead-code inventory + dependency map + gate outputs (new for v5.0)
├── phases/             # Phase scaffolding (this folder)
├── research/           # Stack/architecture/pitfalls (existing documentation)
tests/
└── hardware/           # Artisan Scope evidence checklists and harnesses
src/
├── application/
│   ├── tasks.rs        # Stage-tracked 100 ms loop (sensor → control → actuator → watchdog → telemetry)
│   └── service_container.rs  # Shared channels + watchdog aggregator
├── control/
│   ├── roaster_refactored.rs # Router + policy seams
│   └── handlers.rs            # Single authoritative command handlers
├── hardware/
│   ├── uart/             # Transport ingestion + fault injection hooks
│   └── ledc_guard.rs     # Guard timeouts watched by instrumentation
├── safety/
│   └── watchdog.rs       # Watchdog feeder, failure tracking, STATUS metadata
└── output/
    └── artisan.rs     # Formatter for READ/STATUS with telemetry + watchdog counters
```

### Pattern 1: Stage-Tracked Control Loop
**What:** `control_loop_task` progresses through SensorRead → ControlUpdate → LedcWrite → WatchdogFeed → TelemetryEmit using `StageTracker` so each responsibility is ordered, timed, and budgeted (`src/application/tasks.rs`).
**When to use:** Whenever a refactor touches the handler/hardware boundary or adds instrumentation, keep this stage order and share the same `ServiceContainer` handles. Fault injection triggers (watchdog feed failure, guard timeout) can also use the same tracker to ensure the fault is visible in telemetry before allowing the loop to sleep.
**Example:**
```rust
stage_tracker.set_stage(ControlLoopStage::WatchdogFeed);
let watchdog_snapshot = ServiceContainer::with_roaster_async(|roaster| {
    let status = roaster.status_mut();
    status.watchdog_feed_ok = ServiceContainer::get_instance()
        .with_watchdog(|watchdog| watchdog.feed_async(status.bean_temp))
        .is_ok();
    status.ledc_guard_timeouts = ledc_guard::total_timeouts();
    WatchdogSnapshot {
        feed_ok: status.watchdog_feed_ok,
        last_failure: status.watchdog_last_failure,
        guard_timeouts: status.ledc_guard_timeouts,
    }
}).await;
```
*(Source: `src/application/tasks.rs`, stage-enforced feed + guard logging.)*

### Pattern 2: Ports-and-Policies Handler Separation
**What:** Keep hardware access (heater, fan, watchdog) as ports and move policy decisions to handler/router seams, so SOLID refactors can replace/adapt handlers without touching the hardware layer (`.planning/research/ARCHITECTURE.md`).
**When to use:** When extracting command processing from `process_artisan_command` or other monolithic paths; move semantic decisions into traits (e.g., manual command policy classifiers) before pulling out the hardware apply/guard invocations.
**Example:** Implement a `ManualCommandPolicy` trait that returns success/failure before the guard + watchdog is touched, ensuring invariants remain documented even as handlers split.

### Anti-Patterns to Avoid
- **Big-bang refactor in `RoasterControl`:** Splitting authority in one large PR makes safety regressions hard to isolate; extract seams first (`.planning/research/ARCHITECTURE.md`).
- **Quality checks inside control path:** Adding heavy diagnostics or allocations to the 100 ms loop can shift timing and break watchdog ordering; keep instrumentation outside the direct loop and emit metrics via the host harness.

## Don't Hand-Roll
| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Fault injection evidence harness | Custom logging/short-lived shell script | Host harness using `serialport` + `csv` to capture Artisan commands, safety telemetry, and STATUS snapshots (`.planning/research/STACK.md`). | Provides structured, auditable data suitable for hardware evidence instead of brittle manual screenshots. |
| Watchdog/guard instrumentation | New telemetry channel + ad-hoc watchdog tracker | Existing `ServiceContainer` + `safety::watchdog` + `SystemStatus` fields (watchdog feed OK/counts, LEDC timeouts, regression active). | Keeps the single source of truth for safe behavior and reuses documented telemetry (`internalDoc/INSTRUMENTATION_README.MD`). |

**Key insight:** Fault injection benefits from reusing the documented STATUS payload and the host harness instead of inventing new telemetry because auditors rely on this existing contract, and duplicating it introduces drift and unverified fields.

## Common Pitfalls

### Pitfall 1: Splitting safety authority during SOLID refactors
**What goes wrong:** The handler authority over manual heater/fan outputs or safety flags is duplicated, so command handling diverges from real actuator state (`.planning/research/PITFALLS.md`, “SOLID Refactor Fragments Safety-Critical Authority”).
**Why it happens:** SOLID is treated as a purely structural goal instead of preserving existing invariants.
**How to avoid:** Document single-writer owners for manual/safety fields, enforce them in tests, and keep `RoasterCommandHandler` as the authoritative path during seam extraction.
**Warning signs:** Multiple components mutating the same safety/status fields or integration tests needing identical execution order to pass.

### Pitfall 2: Loop performance regression from over-abstraction
**What goes wrong:** Additional indirection or allocations inside the 100 ms control loop increases jitter and raises watchdog near-misses (`.planning/research/PITFALLS.md`, “Performance Regressions from Over-Abstraction”).
**Why it happens:** Refactors improve readability at the cost of real-time budget.
**How to avoid:** Keep hot-path code allocation-free, add release-mode timing gates, and track stage timing via `StageTracker` so regressions are obvious.
**Warning signs:** Watchdog failures or increased guard timeouts after refactor-only PRs, especially under serial bursts.

## Code Examples

### Ordered watchdog/guard telemetry
```rust
stage_tracker.set_stage(ControlLoopStage::WatchdogFeed);
let watchdog_snapshot = ServiceContainer::with_roaster_async(|roaster| {
    let status = roaster.status_mut();
    let feed_result = ServiceContainer::get_instance().with_watchdog(|watchdog| {
        watchdog.feed_async(status.bean_temp)
    });
    if feed_result.is_ok() {
        status.watchdog_feed_ok = true;
        status.watchdog_last_failure = None;
        status.watchdog_consecutive_failures = 0;
    } else {
        status.watchdog_feed_ok = false;
        status.watchdog_last_failure = Some("watchdog feed failed");
        status.watchdog_consecutive_failures =
            status.watchdog_consecutive_failures.saturating_add(1);
    }
    status.ledc_guard_timeouts = ledc_guard::total_timeouts();
    WatchdogSnapshot { ... }
}).await;
```
*(Source: `src/application/tasks.rs` lines 263‑355.)*

## State of the Art
| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Monolithic control loop with mixed diagnostics and actuator updates | Stage-tracked 100 ms loop that strictly orders Sensor → Control → Ledc → Watchdog → Telemetry and exposes stage timing via logs (`src/application/tasks.rs`). | v5.0 architecture research (2026) aims to keep this order; instrumentation now sits in host harnesses. | Allows SOLID refactors to touch only one responsibility at a time without breaking watchdog ordering or timing budgets. |
| Reactive hardware evidence (ad-hoc screenshots/logs) | Host evidence harness with `serialport` + `csv` + `STATUS` telemetry that is replayable and machine-readable (`.planning/research/STACK.md`, `internalDoc/INSTRUMENTATION_README.MD`). | Introduced alongside the new quality gate architecture (Phase 5 hardware readiness). | Fault injection runs now produce auditable artifacts for watchdog/guard/comms faults instead of manual notes. |

**Deprecated/outdated:**
- Manual, unstructured instrumentation captures (screenshots, ad-hoc serial dumps) should no longer be used; rely on the documented `STATUS` payload and structured harness.

## Open Questions
1. **Precise fault-injection matrix for watchdog/guard/comms scenarios**
   - What exact sequence of command drops, watchdog starves, LEDC guard timeouts, and communication failures must be exercised for hardware sign-off? (`.planning/REQUIREMENTS.md` and `.planning/ROADMAP.md` only say “run fault-injection scenarios”).
   - Recommendation: Document Scenario IDs + expected STATUS snapshot outcomes so the host harness can replay and verify each path without ad-hoc judgement.

## Sources
### Primary (HIGH confidence)
- `.planning/research/ARCHITECTURE.md` – outlines the stage-tracked control loop, ServiceContainer responsibilities, and quality gate structure.
- `.planning/research/STACK.md` – standard stack, hardware harness expectations, and install advice for quality tooling.
- `.planning/research/PITFALLS.md` – enumerates SOLID/fault-injection pitfalls and prevention phases.
- `https://docs.embassy.dev/embassy-executor/0.9.1/` – embassy executor capabilities that keep the realtime loop deterministic.
- `https://docs.embassy.dev/embassy-sync/` – channels/mutex primitives that the code already consumes.
- `https://docs.rs/esp-hal/latest/esp_hal/` – ESP32-specific HAL semantics that drive watchdog/fan instrumentation.

### Secondary (MEDIUM confidence)
- `https://docs.rs/embedded-hal/latest/embedded_hal/` – HAL design goals that justify depending on standardized traits rather than custom register code.
- `https://docs.rs/embedded-hal-mock/latest/embedded_hal_mock/` – mock harness guidance for exercising watchdog/guard/comms scenarios without hardware.
- `internalDoc/INSTRUMENTATION_README.MD` – STATUS payload definition that exposes watchdog, guard, and regression metadata for instrumentation.
- `src/application/tasks.rs` – concrete stage tracker + watchdog feed implementation that must remain intact during seam hardening.

## Metadata
**Confidence breakdown:**
- Standard stack: HIGH – official docs for Embassy/esp-hal plus the phase stack research confirm the approved libraries.
- Architecture: MEDIUM-HIGH – derived from `.planning/research/ARCHITECTURE.md` and code in `src/application/tasks.rs`/`handlers.rs`.
- Pitfalls: MEDIUM – based on `.planning/research/PITFALLS.md` and roadmap requirements.

**Research date:** 2026-03-07
**Valid until:** 2026-04-06
