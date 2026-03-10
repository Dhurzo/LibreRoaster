# Research: Phase 46 — SSR Reliability Foundation

## Context snapshot
- **Goal (ROADMAP):** SSR commands must map to the right LEDC duty, respect the datasheet cycle time, and surface retries/logs when the applied duty drifts.
- **Requirements (REQUIREMENTS.md):** SSR-01/02/03 cover saturating math, cycle guard, LEDC verification. TEST-01 must prove the combined behavior on hardware.
- **Current code:** `src/hardware/ssr.rs` already wraps LEDC via `ChannelIFace` but clamps percentages with a double division and never reads duty back. `RoasterControl` updates the heater through a `Heater` trait without any cycle-aware gating.

## Key insights
1. **Duty math is broken:** `set_percentage` clamps 0–100 but writes `(duty / 100)` which truncates the 8-bit PS output and leaves 100% below 255. Fixing this requires a helper that scales `percentage / 100` against `(1 << SSR_PWM_RESOLUTION) - 1`, saturates, and returns the right `u8` for the LEDC channel.
2. **Cycle guard missing:** There is no concept of a command queue or busy flag. The datasheet demands at least 1 s between full LEDC cycles, and requirement SSR-02 requires rejecting or queuing commands plus reporting when the guard is active. This needs a scheduler that tracks the last apply instant (embassy `Instant`) and exposes when the next cycle is allowed.
3. **Duty monitoring is blind:** `ChannelIFace` supports reading the current duty (e.g., `get_duty` or register peek). SSR-03 and TEST-01 mandate comparing that readback against the commanded duty with ±2-tick tolerance and triggering retries/fault logs when hardware drifts.
4. **Control loop wiring:** The scheduler and monitor must surface status through `SystemStatus` so Artisan commands can warn (or refuse) while a guard is active and telemetry can report a busy window. Extending `SystemStatus` with the next busy timestamp (millisecond) keeps it `Copy` and serializable.

## Execution notes
- Plan early tasks around `src/hardware/ssr.rs` (duty helpers + tolerance constants), a new `src/control/ssr_scheduler.rs`, and updates to `src/control/roaster_refactored.rs` to consult the guard and to record monitor mismatches.
- Split verification between unit tests (`tests/ssr_pwm_conversion.rs`, `tests/ssr_scheduler.rs`, `tests/ssr_monitor.rs`) and a hardware verification checklist (`tests/TEST-01-SSR-Guard.md`).
- Honor existing constraints: LEDC is 25 kHz 8-bit, the scheduler must run on single-core ESP32-C3 with embassy, and `SYSTEM_STATUS` must stay `Copy`.
