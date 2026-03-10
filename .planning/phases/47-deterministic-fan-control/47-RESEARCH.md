# Phase 47: deterministic-fan-control - Research

**Researched:** 2026-02-17
**Domain:** Embedded fan/LEDC command serialization for Artisan protocol
**Confidence:** MEDIUM

## Summary

Fan control touches `src/hardware/fan.rs` (hardware path + host stub), the shared `Ledc`/timer wiring in `src/main.rs` and `AppBuilder`, the SSR guard/telemetry in `src/control/roaster_refactored.rs`, and the command multiplexer (`src/input/multiplexer.rs`) that gates Artisan channels. The current fan controller updates an in-memory duty and logs the request, but it never calls `ChannelIFace::set_duty`/`start_duty_fade`, so Artisan READ responses can report one value while the LEDC hardware still idles. Phase 47 must close that gap while respecting the shared timer/channel ownership that `main.rs` already configures (timer 0 drives Channel 0 for the fan and Channel 1 for the SSR) plus the dual build targets (`#[cfg(target_arch = "riscv32")]` hardware path vs. `fan_host.rs`).

Technology constraints: the firmware is `#![no_std]` (per `.planning/STATE.md`) so any new duty math must avoid pulling in `FloatCore`; the SSR work already introduced saturating/integer conversions, so reuse those patterns (e.g., `fixed::Saturating`) when mapping percentages to 8-bit duty to keep `cargo test --no-default-features` fast. The fan and SSR share the same LEDC timer/clock source (only one `Ledc::timer(Timer0)` is configured in `main.rs`), so all writes/fades must be serialized. The command multiplexer already keeps USB vs UART from conflicting, but there is no similar guard for the fan vs SSR LEDC writes.

User-observable instrumentation: `SystemStatus::fan_output` (consumed by `ArtisanFormatter::format_read_response_full`) is the primary telemetry for verifying FAN-01, so every physical `set_duty` (or fade completion) must immediately update that field. `SystemStatus::ssr_cycle_guard_busy_until_ms` and the `SsrCycleGuard` logs already prove FAN-02 on the heater side; similar instrumentation (e.g., logging `LEDC` fades and queue length) is needed so humans can see when a fan write waits for an SSR fade to finish and why. Planner tasks should therefore touch `src/hardware/fan.rs`, `src/application/app_builder.rs`, `src/control/roaster_refactored.rs`, `src/input/multiplexer.rs`, `src/output/artisan.rs`, `src/config/constants.rs`, and `src/control/ssr_scheduler.rs`.

- What was researched: hardware fan controller internals, command multiplexer, shared LEDC timer wiring, telemetry (`SystemStatus` + `ArtisanFormatter`), and ESP-IDF + esp-hal LEDC usage notes.
- Standard approach: treat `esp-hal::ledc::channel::ChannelIFace` as the single writer, call `set_duty`/`start_duty_fade` immediately, guard access with `critical_section`/`embassy-sync::Mutex`, and never issue overlapping writes while a fade is running.
- Key recommendation: add a tiny LEDC command bus (mutexed queue or dedicated task) that both the fan controller and SSR guard use, update `SystemStatus::fan_output` with the real hardware value, and surface serialization in the multiplexer/formatter logs so FAN-02’s behavior is visible.

**Primary recommendation:** Have the fan controller become the LEDC channel owner and route every LEDC update (fan or SSR fade) through a single serialized path that writes via `ChannelIFace::set_duty`/`start_duty_fade`, keeps `SystemStatus::fan_output` in sync, and emits logging/telemetry whenever a write waits on the SSR guard.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `esp-hal` (with `esp32c3` + `unstable` features) | ~1.0.0 | LEDC timers + channel control | Provides `ChannelIFace::set_duty`/`start_duty_fade` and `TimerIFace` used by both the fan controller and SSR writer; docs confirm LEDC writes only take effect after `set_duty` + `update` and that fades block other updates (HIGH confidence). |
| `embassy-executor` | 0.9.1 | Async task runtime | Already spawns Artisan UART/USB tasks and the control loop in `main.rs`; keeps the single-core ESP32-C3 scheduled so LEDC writes can run cooperatively. |
| `critical-section` | 1.2.0 | Protect shared LEDC state | `FanController` already wraps `PwmState` in a critical section, so extend the same guard (or `embassy-sync::Mutex`) to serialize fan+SSR writes and avoid `ledc_set_duty` thread-safety issues documented by ESP-IDF. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `fixed` | 1.30.0 | Saturating duty math without `FloatCore` | Use when scaling 0–100% to 8-bit duty to respect the “no floating point in `no_std` builds” constraint and match the SSR conversions already locked in by Phase 46. |
| `fugit` | 0.3.9 | Timer frequency/duty helpers (`Rate`, `Duration`) | Use the same crate already chosen for the LEDC timer config to calculate fade durations and ensure the fan shares the same frequency/resolution budget. |
| `embassy-sync` | 0.6.1 | `Mutex`/`Signal` for concurrency | Use in addition to `critical_section` when building a small queue that serializes LEDC writes across tasks and may need to sleep while waiting for the SSR guard. |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `esp-hal` LEDC API | Raw `esp-idf::ledc` C bindings | Would require manual thread-safety wrappers (`ledc_set_duty_and_update`) and duplicate timer setup; risk double-wiring the timer compared to using the existing `Ledc` instance already passed through `AppBuilder`. |
| `fixed`/saturating math | `f32` arithmetic | `f32` introduces `FloatCore` in `no_std` builds and makes saturated scaling harder; the SSR guard already proved the saturating approach works and eliminates rounding mismatches in telemetry. |

**Installation:**
```bash
cargo add esp-hal@~1.0 embassy-executor@0.9.1 critical-section@1.2 fixed@1.30 fugit@0.3.9 embassy-sync@0.6.1
```

## Architecture Patterns

### Recommended Project Structure
```
src/
├── hardware/        # LEDC fan/SSR, host stubs, shared peripherals (FanController, SsrControl)
├── control/         # RoasterControl, SsrCycleGuard, PID + telemetry plumbing (SystemStatus)
├── input/           # Artisan parser, CommandMultiplexer, ServiceContainer
└── output/          # ArtisanFormatter that reads SystemStatus + fan_output
```

### Pattern 1: Serialized LEDC Bus
**What:** A single logical owner (FanController + SSR guard) serializes `ChannelIFace::set_duty`/`start_duty_fade` calls even though hardware provides only one timer and channel pair for each output. Use `critical_section`/`embassy-sync::Mutex` plus a small queue so the fan waits for the SSR guard or vice versa, then immediately update `SystemStatus::fan_output` and log the effective duty.
**When to use:** Every Artisan fan/heater command in this phase (FAN-01/FAN-02); the scheduler already guards `SsrCycleGuard` writes, so extend that guard to include fan updates or share the same mutex.
**Example:**
```rust
// Source: src/control/roaster_refactored.rs
match self.ssr_guard.next_cycle_allowed(now) {
    Ok(_) => {
        self.ssr_guard.mark_cycle(now);
        self.heater.set_power(clamped)?;
        self.status.ssr_output = clamped;
        self.update_guard_busy_ms(now);
    }
    Err(busy_until) => {
        warn!("SSR cycle busy until {:?}", busy_until);
        self.status.ssr_cycle_guard_busy_until_ms = Self::busy_window_ms(now, busy_until);
    }
}
```

### Anti-Patterns to Avoid
- **Parallel LEDC writes from fan + SSR:** The ESP-IDF LEDC doc warns `ledc_set_duty`/`ledc_update_duty` are not thread-safe and cannot run while a fade is active. Do not fire both writes concurrently or from ISR/foreground tasks without `critical_section` guards.
- **Duplicating LEDC wiring per transport:** The existing `CommandMultiplexer` already prevents USB/UART double-commands; adding another dispatcher that races writes to the same LEDC timer will cause audible jumps.

## Don't Hand-Roll
| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| LEDC duty serialization | Your own atomic queue that pokes `ledc_set_duty` directly (C APIs) | `esp-hal::ledc::channel::ChannelIFace` guarded by `critical_section`/`embassy-sync::Mutex` | Doc warns the C APIs ignore writes during fades and lack thread safety, so reusing the safe HAL keeps the timer/resolution config centralized. |
| Fan fade scheduling | Manual delay loops writing successive duty levels | `ChannelIFace::start_duty_fade` plus `is_duty_fade_running` | LEDC fades already guarantee the hardware transitions smoothly; re-implementing them wastes CPU and may violate `ledc_fade_start`’s non-thread-safe restriction. |

**Key insight:** The hardware already provides fade helpers and atomic `set_duty`/`update` semantics; wrapping them in a mutex/queue is cheaper and safer than rebuilding the API.

## Common Pitfalls

### Pitfall 1: LEDC updates collide when fan + SSR share a timer
**What goes wrong:** `ledc_set_duty`/`ledc_update_duty` ignore writes from one task when another is touching the same channel, so commands race and the fan/SSR output freezes or jumps audibly.
**Why it happens:** ESP-IDF v5.5.2 docs warn those APIs are not thread-safe and that no other duty change can execute while a fade is in progress (`ledc_set_fade_*`/`ledc_fade_start`).
**How to avoid:** Guard all LEDC writes from fan and SSR with the same mutex or single task, wait for `is_duty_fade_running` to clear before issuing a new `set_duty`, and serialize fades so `update_duty` runs only once per logical request.
**Warning signs:** Logs show “LED C busy” or missing fade completions, Artisan READ reports stale fan output, or `ledc` driver emits `requested frequency and duty` errors.

### Pitfall 2: Interrupts/embassy tasks preempt LEDC writes
**What goes wrong:** If a high-priority interrupt or async task runs while `ledc_set_duty` is mid-sequence, the driver may keep old duty values or skip fade completion, producing audible clicks.
**Why it happens:** LEDC writes must execute from IRAM/`critical_section`; embedding them inside blocking I/O or future polling delays the update until after the interrupt. The ESP-IDF document emphasizes that `ledc_update_duty` works even inside ISRs but not if another context is already updating the same channel.
**How to avoid:** Keep LEDC writes short, run them inside the executor’s control loop, and guard the whole update with `critical_section` so interrupts can’t interleave. Use `embassy-sync::Mutex` when sleeping on another condition (e.g., waiting on SSR guard).
**Warning signs:** Fan commands are acknowledged but the LEDC output doesn’t change until the next interrupt, or audible steps occur whenever a UART/USB callback fires.

### Pitfall 3: DRAM-heavy tasks block LEDC command serialization
**What goes wrong:** A fan command waits while a heap allocation or logging call runs, so the SSR fade finishes but the fan update executes out of sync, triggering audible jumps.
**Why it happens:** Fan control currently logs every request and clamps percentages using `f32` math; both operations can allocate or take non-trivial time on `no_std` ESP32-C3 builds.
**How to avoid:** Keep fan/SSR updates lean (lock-free `fixed` math, minimal formatting), and perform `SystemStatus` and log updates only after the LEDC write completes.
**Warning signs:** Telemetry shows `fan_output` lagging the commanded speed, or audible jumps only when logging is verbose.

## Code Examples

### Verifying LEDC duty after SSR write
```rust
// Source: src/hardware/ssr.rs
fn monitor_ledc_after_set<'a, PWM>(pwm_channel: &mut PWM, commanded: u8, retry_count: &mut u8, last_delta: &mut i16) -> Result<(), SsrError>
where
    PWM: LedcDutyReader + ChannelIFace<'a, LowSpeed>,
{
    let readback = pwm_channel.read_duty_ticks();
    let delta = readback as i16 - commanded as i16;
    if delta.abs() <= SSR_DUTY_TOLERANCE_TICKS as i16 {
        return Ok(());
    }
    warn!("LEDC duty drift detected... retrying");
    *retry_count = retry_count.saturating_add(1);
    pwm_channel.set_duty(commanded)?;
    let rechecked = pwm_channel.read_duty_ticks();
    if (rechecked as i16 - commanded as i16).abs() > SSR_DUTY_TOLERANCE_TICKS as i16 {
        error!("LEDC duty mismatch persists");
        return Err(SsrError::PwmError);
    }
    Ok(())
}
```

### Command multiplexer gating
```rust
// Source: src/input/multiplexer.rs
pub fn on_command_received(&mut self, channel: CommChannel) -> bool {
    let now = Instant::now();
    match self.active_channel {
        CommChannel::None => { /* activate the caller */ }
        current if current == channel => { /* refresh timeout */ }
        _ => { return false; }
    }
    true
}
```

## State of the Art
| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| FanController only updated `current_speed` and logged the request | Phase 47 makes it write through `ChannelIFace::set_duty`/`start_duty_fade` and updates `SystemStatus::fan_output` so telemetry matches hardware | 2026-02 (Phase 47 start) | Artisan READ now reflects the actual PWM channel and hardware tests can assert the LEDC duty, satisfying FAN-01. |
| Fan and SSR updates raced on the shared timer | Introduce a serialized LEDC bus guarded by `critical_section`/`Mutex` and monitor `SsrCycleGuard` busy windows before emitting fan writes | 2026-02 (Phase 47) | Prevents audible jumps and timer collisions, fulfilling FAN-02. |

**Deprecated/outdated:**
- Directly invoking `ledc_set_duty`/`ledc_update_duty` from multiple tasks without a guard (the ESP-IDF doc warns this is unsafe and ignores updates during fades).
- Fan duty math that relies on `f32` rounding (links `FloatCore` into `no_std` builds) now replaced by saturating fixed-point helpers.

## Open Questions
1. **Fade duration policy for fan spin-up/spin-down**
   - What we know: `ChannelIFace::start_duty_fade` requires balancing frequency, resolution, and duration (docs warn of failure if freq×duration/(steps) ≥ 1024).
   - What’s unclear: The audible threshold (ms) acceptable for Artisan fans and whether SSR fades should block new fan writes.
   - Recommendation: Tune fade time empirically in hardware once serialized writes exist, but build the queue so it can lock the LEDC channel while a fade is running and report `busy_until` in telemetry.
2. **Telemetry needing real hardware duty ticks**
   - What we know: `SystemStatus::fan_output` already feeds Artisan READ; `monitor_ledc_after_set` exposes readback for SSR.
   - What’s unclear: Should we also add a `fan_last_duty_delta_ticks`/`fan_retry_count` field similar to the SSR metrics? Or is `fan_output` enough?
   - Recommendation: Capture the actual `percentage_to_duty` post-update and log it when it diverges, so planners can easily add `fan_ledc_delta` telemetry if tests require it.

## Sources
### Primary (HIGH confidence)
- https://docs.rs/esp-hal/latest/esp_hal/ledc/channel/trait.ChannelIFace.html — documents `set_duty`/`start_duty_fade` semantics on the unstable HAL feature.
- https://docs.espressif.com/projects/esp-idf/en/v5.5.2/api-reference/peripherals/ledc.html — warns `ledc_set_duty`/`ledc_update_duty`/`ledc_set_fade_*` are not thread-safe and that fades block other writes (justifies serialization). 

### Secondary (MEDIUM confidence)
- .planning/REQUIREMENTS.md — defines FAN-01/FAN-02 and ties them to LEDC updates, ensuring the planner focuses on those behaviors.
- .planning/STATE.md — records the `no-floating point` constraint, SSR guard telemetry fields, and the pending fan controller TODOs.

### Tertiary (LOW confidence)
- .planning/research/PITFALLS.md — catalogs ESP-IDF pain points around LEDC collisions, reaffirming the need for a serialized LEDC bus even though no new tests were run for Phase 47.

## Metadata
**Confidence breakdown:**
- Standard stack: MEDIUM — esp-hal/embassy docs confirm the required APIs, but some integration choices still require judgment.
- Architecture: MEDIUM — code inspection shows the shared LEDC path and telemetry fields, but the exact serialization mechanism still needs to be designed.
- Pitfalls: HIGH — backed by ESP-IDF LEDC documentation and the project’s own PITFALLS log.

**Research date:** 2026-02-17
**Valid until:** 2026-03-18
