//! PID controller for bean-temperature regulation with anti-windup protection.
//!
//! `CoffeeRoasterPid` is a positional P/I/D controller whose integrator gates itself on actuator feedback (`PidFeedback`) and on its own output clamp, so the heater cannot wind up while saturated or guard-blocked.

use crate::config::PID_SAMPLE_TIME_MS;

const DEFAULT_KP: f32 = 2.0;
const DEFAULT_KI: f32 = 0.25;
const DEFAULT_KD: f32 = 0.05;
const SATURATION_EPSILON: f32 = 1.0;

/// Provides the latest actuator status so the PID can obey actual outputs and guard resets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PidFeedback {
    /// Heater duty requested by the controller on the last tick (0-100 %).
    pub desired_output: f32,
    /// Heater duty actually applied after slew/clamping (0-100 %).
    pub applied_output: f32,
    /// True while the SSR zero-cross cycle guard is busy.
    pub guard_busy: bool,
}

impl PidFeedback {
    /// Creates a new feedback snapshot coming from the actuator/LEDC guard.
    pub const fn new(desired_output: f32, applied_output: f32, guard_busy: bool) -> Self {
        Self {
            desired_output,
            applied_output,
            guard_busy,
        }
    }

    /// Indicates saturation or guard blocking so the integrator should hold.
    pub fn is_saturated(&self) -> bool {
        self.guard_busy
            || !self.desired_output.is_finite()
            || !self.applied_output.is_finite()
            || (self.desired_output - self.applied_output) > SATURATION_EPSILON
    }
}

/// Positional PID controller for bean temperature with conditional-integration anti-windup.
pub struct CoffeeRoasterPid {
    kp: f32,
    ki: f32,
    kd: f32,
    integrator: f32,
    last_error: f32,
    /// Bug #5: `last_error` is only meaningful after the controller has
    /// observed at least one real sample. Before that, `(error - last_error)
    /// / dt` would be `(error - 0) / dt` — a massive derivative spike that
    /// injects a one-tick heater surge on PID enable. We gate the derivative
    /// term behind this flag and skip it entirely on the first tick (setting
    /// `last_error = error` so tick #2 onward computes a real slope).
    last_error_initialized: bool,
    derivative_rate: f32,
    last_update_ms: Option<u32>,
    target: f32,
    enabled: bool,
    integrator_clamped: bool,
    saturation_active: bool,
    last_feedback: Option<PidFeedback>,
    cycle_time_ms: u32,
    output_min: f32,
    output_max: f32,
}

/// Errors returned by the PID controller.
#[derive(Debug, PartialEq)]
pub enum PidError {
    /// Construction or reset of the controller failed.
    InitializationError,
    /// A computation input was invalid (e.g., a non-finite target).
    ComputationError,
}

impl CoffeeRoasterPid {
    /// Creates a controller with tuned default gains that can be overridden via `with_gains` if needed.
    pub fn new() -> Result<Self, PidError> {
        Ok(Self::with_gains(DEFAULT_KP, DEFAULT_KI, DEFAULT_KD))
    }

    /// Allows the caller to provide custom PID tuning while keeping other state reset.
    pub fn with_gains(kp: f32, ki: f32, kd: f32) -> Self {
        Self {
            kp,
            ki,
            kd,
            integrator: 0.0,
            last_error: 0.0,
            last_error_initialized: false,
            derivative_rate: 0.0,
            last_update_ms: None,
            target: 0.0,
            enabled: false,
            integrator_clamped: false,
            saturation_active: false,
            last_feedback: None,
            cycle_time_ms: PID_SAMPLE_TIME_MS,
            output_min: 0.0,
            output_max: 100.0,
        }
    }

    /// Update PID gains in place, preserving `enabled`, `target`,
    /// `output_min/max`, `cycle_time_ms` and `derivative_rate`; resets the
    /// integrator and last-error baseline.
    ///
    /// Bug B5: `set_pid_gains` (handlers/temperature.rs) used to replace the
    /// whole controller with `CoffeeRoasterPid::with_gains(...)`, which
    /// rebuilds with `enabled: false` and `target: 0.0`. The status field
    /// `pid_enabled` was NOT touched, so telemetry kept reporting the PID as
    /// active while `compute_output` returned 0.0 — silently cutting the
    /// heater to 0% any time the operator tuned gains from Artisan's PID
    /// dialog. Resetting the integrator here avoids a one-tick I-term jump
    /// from the new gain on the already-accumulated error.
    pub fn set_gains(&mut self, kp: f32, ki: f32, kd: f32) {
        self.kp = kp;
        self.ki = ki;
        self.kd = kd;
        self.integrator = 0.0;
        self.last_error = 0.0;
        self.last_error_initialized = false;
    }

    /// Enable the controller, resetting integrator and derivative state.
    pub fn enable(&mut self) {
        self.enabled = true;
        self.integrator = 0.0;
        self.last_error = 0.0;
        // Bug #5: defer derivative computation until we have observed one
        // real error sample. compute_output seeds `last_error` from the
        // first error it sees and returns derivative_rate = 0.0 on that
        // tick, eliminating the spike that the previous `last_error = 0.0`
        // baseline produced (e.g. (200-30)/0.1 = 1700 °C/s → 85% output).
        self.last_error_initialized = false;
        self.derivative_rate = 0.0;
        self.last_update_ms = None;
        self.last_feedback = None;
    }

    /// Disable the controller; `compute_output` returns 0.0 while disabled.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Whether the controller is currently enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set the target temperature; rejects non-finite values.
    pub fn set_target(&mut self, target: f32) -> Result<(), PidError> {
        if !target.is_finite() {
            return Err(PidError::ComputationError);
        }
        self.target = target;
        Ok(())
    }

    /// Pushes actuator/guard feedback into the PID so the integrator can gate itself when the hardware cannot accept more energy.
    pub fn update_feedback(&mut self, feedback: PidFeedback) {
        self.last_feedback = Some(feedback);
    }

    /// Set the PID cycle time in milliseconds (minimum 10).
    pub fn set_cycle_time(&mut self, ms: u32) {
        self.cycle_time_ms = ms.max(10);
    }

    /// Set the output clamp range; values are clamped to [0,100] and swapped if inverted.
    pub fn set_output_limits(&mut self, min: f32, max: f32) {
        self.output_min = min.clamp(0.0, 100.0);
        self.output_max = max.clamp(0.0, 100.0);
        if self.output_min > self.output_max {
            core::mem::swap(&mut self.output_min, &mut self.output_max);
        }
    }

    /// Returns the (post-clamp, post-swap) output limits currently applied by
    /// the PID. Callers that echo `set_output_limits` inputs into telemetry
    /// should report these values, not the raw inputs, so observers see the
    /// limits the PID is actually enforcing (clamped to [0,100] and swapped
    /// to satisfy `min <= max`).
    pub fn output_limits(&self) -> (f32, f32) {
        (self.output_min, self.output_max)
    }

    /// Reports the accumulated integrator term for telemetry consumers.
    pub fn integrator_value(&self) -> f32 {
        self.integrator
    }

    /// Exposes the derivative rate so STATUS telemetry can publish the true value instead of mirroring the desired SSR duty.
    pub fn derivative_value(&self) -> f32 {
        self.derivative_rate
    }

    /// Indicates whether gating currently prevents integrator growth.
    pub fn is_integrator_clamped(&self) -> bool {
        self.integrator_clamped
    }

    /// Indicates if saturation or guard activity is preventing each update from increasing MV.
    pub fn is_saturation_active(&self) -> bool {
        self.saturation_active
    }

    /// Computes the PID output while maintaining P/I/D state.
    ///
    /// The signature matches the old helper so existing call sites do not require rewiring. The actuator
    /// feedback hook (`update_feedback`) is intended to be invoked after each write so the controller
    /// knows when the guard rejected a cycle or saturation occurred.
    pub fn compute_output(&mut self, current_temp: f32, timestamp_ms: u32) -> f32 {
        if !self.enabled {
            return 0.0;
        }

        let dt = self.delta_seconds(timestamp_ms);
        let error = self.target - current_temp;

        // Bug B6: anti-windup must also see the PID's *own* output clamp, not
        // just the actuator saturation reported via `should_integrate()`.
        // In steady state at 100% (desired == applied == output_max) the
        // actuator reports no saturation yet the integrator keeps accumulating
        // `error * dt` every tick. A 2-minute stuck-at-100% with a 150 °C
        // error winds the integrator to ~ki*18000 → a huge overshoot once the
        // target is approached (plausibly tripping OVERTEMP=260 °C).
        // Compute the *predictive* unclamped MV (P+D plus the I-term we *would*
        // add this tick) and refuse to integrate when it has already hit a
        // rail in the direction of the error. This is the classic conditional
        // integration anti-windup on the controller's own clamp.
        let p_d = (self.kp * error) + (self.kd * self.estimate_derivative(error, dt));
        let unclamped = p_d + (self.ki * self.integrator);
        let clamped_hi = unclamped >= self.output_max && error > 0.0;
        let clamped_lo = unclamped <= self.output_min && error < 0.0;

        if self.should_integrate() && !clamped_hi && !clamped_lo {
            self.integrator += error * dt;
            self.integrator_clamped = false;
        } else {
            self.integrator_clamped = true;
        }

        // Bug #5: skip the derivative term on the first tick after enable.
        // Using `last_error = 0.0` as the baseline would produce a one-shot
        // spike (e.g. (170 - 0) / 0.1 = 1700 °C/s → kd*derivative = 85%
        // output surge). On the first tick we seed `last_error = error` so
        // the next tick computes a real slope, and we emit derivative = 0.0.
        let derivative = if self.last_error_initialized && dt > 0.0 {
            let derivative = (error - self.last_error) / dt;
            self.derivative_rate = derivative;
            derivative
        } else {
            // First tick after enable: no previous error to diff against.
            // Maintain derivative_rate = 0.0 so STATUS telemetry does not
            // report a phantom spike either.
            self.derivative_rate = 0.0;
            0.0
        };

        self.last_error = error;
        self.last_error_initialized = true;
        self.saturation_active = self
            .last_feedback
            .map(|feedback| feedback.is_saturated())
            .unwrap_or(false);

        let mut mv = (self.kp * error) + (self.ki * self.integrator) + (self.kd * derivative);
        mv = mv.clamp(self.output_min, self.output_max);
        mv = self.bound_to_actuator(mv);

        self.last_update_ms = Some(timestamp_ms);
        mv
    }

    /// Seconds since the last update, falling back to `cycle_time_ms` when unknown.
    fn delta_seconds(&self, timestamp_ms: u32) -> f32 {
        let default_seconds = self.cycle_time_ms as f32 / 1000.0;

        if let Some(last_ms) = self.last_update_ms {
            let delta = timestamp_ms.saturating_sub(last_ms);
            if delta == 0 {
                return default_seconds;
            }

            return (delta as f32) / 1000.0;
        }

        default_seconds
    }

    /// Anti-windup gate: integrate only while actuator feedback is not saturated.
    fn should_integrate(&self) -> bool {
        self.last_feedback
            .map(|feedback| !feedback.is_saturated())
            .unwrap_or(true)
    }

    /// Predictive derivative used by the B6 anti-windup pre-check.
    ///
    /// `compute_output` needs the P+D term *before* it has computed this
    /// tick's derivative (it has to decide whether to integrate first). We
    /// re-use the previous tick's derivative as the predictor — exactly what
    /// `derivative_rate` already holds. On the first tick after enable the
    /// derivative is 0.0 (Bug #5), so the predictive P+D reduces to just
    /// `kp*error`, which is the right behaviour for the anti-windup rail
    /// check: a non-zero `error` plus an integrator that is *already* at the
    /// rail is precisely the case where we must stop accumulating.
    fn estimate_derivative(&self, _error: f32, _dt: f32) -> f32 {
        self.derivative_rate
    }

    /// Clamp the MV to the output limits and flag integrator clamping when desired exceeds applied.
    fn bound_to_actuator(&mut self, mv: f32) -> f32 {
        // Only clamp to the configured output range. Anti-windup is already
        // applied in `should_integrate()` by checking `feedback.is_saturated()`,
        // so we must NOT re-clamp the output to the actuator's previously
        // applied value here — that would pin the output to the first slew
        // step (~5%) for the whole roast (the bug closed by this change). The
        // actuator's own slew-rate limiter (`SSR_SLEW_RATE_PER_SEC = 50.0`,
        // ~5%/tick) physically bounds how fast the heater can ramp up. The PID
        // now always returns its MV clamped to [output_min, output_max]; the
        // actuator decides how much to apply. The PID rises, but does not wind
        // up, because the integrator stops accumulating while the actuator is
        // saturated (see `should_integrate`).
        let clamped = mv.clamp(self.output_min, self.output_max);

        if let Some(feedback) = self.last_feedback {
            let applied = feedback
                .applied_output
                .clamp(self.output_min, self.output_max);
            if clamped > applied + SATURATION_EPSILON {
                self.integrator_clamped = true;
            }
        }

        clamped
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        // a) PID output always clamped to [output_min, output_max]
        #[test]
        fn pid_output_always_clamped(
            kp in 0.5f32..5.0,
            ki in 0.1f32..0.5,
            kd in 0.01f32..0.1,
            target in 50.0f32..250.0,
            temp in 20.0f32..250.0,
            output_min in 0.0f32..50.0,
            output_max in 50.0f32..100.0,
            num_steps in 1u32..10
        ) {
            let mut pid = CoffeeRoasterPid::with_gains(kp, ki, kd);
            pid.set_output_limits(output_min, output_max);
            pid.enable();
            pid.set_target(target).unwrap();

            let mut current_temp = temp;
            let mut timestamp = 0u32;

            for _ in 0..num_steps {
                let output = pid.compute_output(current_temp, timestamp);
                assert!(output >= output_min && output <= output_max);

                // Simulate temperature change
                current_temp += (target - current_temp) * 0.1;
                timestamp += PID_SAMPLE_TIME_MS;
            }
        }

        // b) PID disabled always returns 0.0
        #[test]
        fn pid_disabled_returns_zero(
            kp in 0.5f32..5.0,
            ki in 0.1f32..0.5,
            kd in 0.01f32..0.1,
            target in 0.0f32..300.0,
            temp in 0.0f32..300.0,
            timestamp in 0u32..10000
        ) {
            let mut pid = CoffeeRoasterPid::with_gains(kp, ki, kd);
            pid.disable();
            pid.set_target(target).unwrap();

            let output = pid.compute_output(temp, timestamp);
            assert_eq!(output, 0.0);
        }

        // c) Integrator bounds test - integrator stays within reasonable bounds
        #[test]
        fn pid_integrator_bounds(
            kp in 0.5f32..5.0,
            ki in 0.1f32..0.5,
            kd in 0.01f32..0.1,
            target in 100.0f32..200.0,
            output_min in 0.0f32..20.0,
            output_max in 80.0f32..100.0
        ) {
            let mut pid = CoffeeRoasterPid::with_gains(kp, ki, kd);
            pid.set_output_limits(output_min, output_max);
            pid.enable();
            pid.set_target(target).unwrap();

            // Start with a large temperature difference to create sustained error
            let current_temp = 20.0;
            let mut timestamp = 0u32;

            // Run for 10 steps with sustained error
            for _i in 0..10 {
                let output = pid.compute_output(current_temp, timestamp);
                pid.update_feedback(PidFeedback::new(output, output, false));

                // Check that integrator doesn't grow unbounded
                let integrator = pid.integrator_value();
                assert!(integrator.is_finite() && integrator.abs() < 1000.0);

                // Temperature stays constant to maintain sustained error
                timestamp += PID_SAMPLE_TIME_MS;
            }
        }

        // d) NaN target is always rejected
        #[test]
        fn pid_nan_target_rejected(
            kp in 0.5f32..5.0,
            ki in 0.1f32..0.5,
            kd in 0.01f32..0.1
        ) {
            let mut pid = CoffeeRoasterPid::with_gains(kp, ki, kd);

            // Test NaN
            assert!(pid.set_target(f32::NAN).is_err());

            // Test infinity
            assert!(pid.set_target(f32::INFINITY).is_err());
            assert!(pid.set_target(f32::NEG_INFINITY).is_err());

            // Test finite values work
            assert!(pid.set_target(100.0).is_ok());
            assert!(pid.set_target(0.0).is_ok());
            assert!(pid.set_target(-50.0).is_ok());
        }

        // e) Fase 3 (BUG-CATCH-PLAN.md): hostile but FINITE inputs (huge
        // magnitudes, f32::MAX, subnormals, negative) must always produce a
        // finite output clamped into [output_min, output_max]. The NaN-input
        // boundary is documented separately (`nan_current_temp_produces_nan`,
        // unit test below): NaN propagates NaN out of the PID, and the
        // controller layer must reject it (NaN PV → emergency, roaster_
        // control.rs:679-682). This property proves every OTHER input class
        // cannot unclamp or poison the output.
        #[test]
        fn pid_output_finite_and_clamped_for_hostile_finite_inputs(
            kp in 0.5f32..5.0,
            ki in 0.1f32..0.5,
            kd in 0.01f32..0.1,
            target in prop_oneof![
                Just(f32::MAX),
                Just(f32::MIN_POSITIVE),
                Just(f32::MIN_POSITIVE * 0.5),
                -1e30f32..1e30,
            ],
            temp in prop_oneof![
                Just(f32::MAX),
                Just(f32::MIN_POSITIVE),
                Just(f32::MIN_POSITIVE * 0.5),
                Just(-0.0),
                -1e30f32..1e30,
            ],
            output_min in 0.0f32..50.0,
            output_max in 50.0f32..100.0,
            timestamp in 0u32..10000
        ) {
            let mut pid = CoffeeRoasterPid::with_gains(kp, ki, kd);
            pid.set_output_limits(output_min, output_max);
            pid.enable();
            // Out-of-range targets are rejected upstream (handler validates
            // 50..=300 °C); if this PID rejects it too, the contract holds
            // (the property only applies to accepted targets).
            if pid.set_target(target).is_ok() {
                let output = pid.compute_output(temp, timestamp);
                assert!(
                    output.is_finite(),
                    "PID output must stay finite for finite inputs, got {output:?}"
                );
                assert!(
                    output >= output_min && output <= output_max,
                    "PID output must be clamped to [output_min, output_max], got {output:?}"
                );
            }
        }
    }

    #[test]
    fn integrator_accumulates_when_actuator_accepts_energy() {
        let mut pid = CoffeeRoasterPid::with_gains(1.0, 0.5, 0.1);
        pid.enable();
        pid.set_target(120.0).unwrap();

        let first_output = pid.compute_output(30.0, 0);
        pid.update_feedback(PidFeedback::new(first_output, first_output, false));

        let before = pid.integrator_value();
        pid.compute_output(35.0, PID_SAMPLE_TIME_MS);

        assert!(pid.integrator_value() > before);
        assert!(!pid.is_integrator_clamped());
    }

    #[test]
    fn integrator_holds_while_guard_is_busy() {
        let mut pid = CoffeeRoasterPid::with_gains(1.0, 0.5, 0.1);
        pid.enable();
        pid.set_target(150.0).unwrap();

        let first_output = pid.compute_output(70.0, 0);
        pid.update_feedback(PidFeedback::new(first_output, first_output, false));

        let before = pid.integrator_value();
        pid.update_feedback(PidFeedback::new(
            first_output + 40.0,
            first_output + 10.0,
            true,
        ));
        pid.compute_output(75.0, PID_SAMPLE_TIME_MS);

        assert!((pid.integrator_value() - before).abs() < f32::EPSILON);
        assert!(pid.is_integrator_clamped());
        assert!(pid.is_saturation_active());
    }

    #[test]
    fn integrator_resumes_after_guard_releases() {
        let mut pid = CoffeeRoasterPid::with_gains(1.0, 0.5, 0.1);
        pid.enable();
        pid.set_target(140.0).unwrap();

        let first_output = pid.compute_output(65.0, 0);
        pid.update_feedback(PidFeedback::new(first_output, first_output, false));

        pid.update_feedback(PidFeedback::new(
            first_output + 50.0,
            first_output + 15.0,
            true,
        ));
        pid.compute_output(70.0, PID_SAMPLE_TIME_MS);

        let during = pid.integrator_value();
        pid.update_feedback(PidFeedback::new(
            first_output + 20.0,
            first_output + 20.0,
            false,
        ));
        pid.compute_output(74.0, PID_SAMPLE_TIME_MS * 2);

        assert!(pid.integrator_value() > during);
        assert!(!pid.is_integrator_clamped());
        assert!(!pid.is_saturation_active());
    }

    #[test]
    fn nan_current_temp_produces_nan() {
        let mut pid = CoffeeRoasterPid::with_gains(1.0, 0.5, 0.1);
        pid.enable();
        pid.set_target(100.0).unwrap();
        let output = pid.compute_output(f32::NAN, 100);
        assert!(output.is_nan());
    }

    #[test]
    fn nan_target_is_rejected() {
        let mut pid = CoffeeRoasterPid::with_gains(1.0, 0.5, 0.1);
        assert!(pid.set_target(f32::NAN).is_err());
    }

    #[test]
    fn disabled_pid_always_returns_zero() {
        let mut pid = CoffeeRoasterPid::with_gains(1.0, 0.5, 0.1);
        pid.set_target(200.0).unwrap();
        assert_eq!(pid.compute_output(30.0, 0), 0.0);
    }

    #[test]
    fn output_limits_swap_when_min_greater_than_max() {
        let mut pid = CoffeeRoasterPid::with_gains(1.0, 0.5, 0.1);
        pid.set_output_limits(80.0, 20.0);
        pid.enable();
        pid.set_target(200.0).unwrap();
        let output = pid.compute_output(30.0, 0);
        assert!((20.0..=80.0).contains(&output));
    }

    #[test]
    fn integrator_clamped_when_desired_exceeds_applied() {
        let mut pid = CoffeeRoasterPid::with_gains(1.0, 0.5, 0.1);
        pid.enable();
        pid.set_target(200.0).unwrap();

        let out1 = pid.compute_output(30.0, 0);
        pid.update_feedback(PidFeedback::new(out1, out1, false));
        let before = pid.integrator_value();

        pid.update_feedback(PidFeedback::new(100.0, 0.0, true));
        pid.compute_output(35.0, PID_SAMPLE_TIME_MS);

        assert!(pid.is_integrator_clamped());
        assert!((pid.integrator_value() - before).abs() < f32::EPSILON);
    }

    #[test]
    fn saturation_flagged_when_guard_busy() {
        let mut pid = CoffeeRoasterPid::with_gains(1.0, 0.5, 0.1);
        pid.enable();
        pid.set_target(200.0).unwrap();

        pid.compute_output(30.0, 0);
        pid.update_feedback(PidFeedback::new(50.0, 50.0, true));
        pid.compute_output(35.0, PID_SAMPLE_TIME_MS);

        assert!(pid.is_saturation_active());
    }

    // ── Bug B6: anti-windup must see the PID's own output clamp ──────────
    //
    // Steady state at output_max with no actuator saturation feedback: the
    // integrator must NOT keep accumulating `error * dt` indefinitely. The
    // pre-fix code only gated on `should_integrate()` (actuator saturation),
    // which is false in this scenario, so the integrator grew unbounded
    // → large overshoot when the target was finally approached.

    #[test]
    fn b6_windup_clamps_at_output_max_in_steady_state() {
        // Aggressive gains + small output range so we hit the rail quickly.
        let mut pid = CoffeeRoasterPid::with_gains(5.0, 0.5, 0.05);
        pid.set_output_limits(0.0, 100.0);
        pid.enable();
        pid.set_target(250.0).unwrap();

        // Simulate the plant stuck at 100 °C with the heater maxed: the
        // actuator reports NO saturation (desired == applied == output_max)
        // — exactly the regime B6 lived in. Pre-fix, the integrator would
        // grow ~ki*error*dt each tick with no clamp.
        let mut timestamp = 0u32;
        let mut max_integrator = 0.0f32;
        for _ in 0..200 {
            let out = pid.compute_output(100.0, timestamp);
            // Steady actuator: applied == desired, guard not busy.
            pid.update_feedback(PidFeedback::new(out, out, false));
            timestamp += PID_SAMPLE_TIME_MS;
            max_integrator = max_integrator.max(pid.integrator_value());
        }
        // The integrator must have stopped accumulating long before 200 ticks.
        // Pre-fix it would reach ki*150*200*0.1 = 1500 — way over 1000.
        assert!(
            max_integrator <= 1000.0,
            "B6: integrator must be anti-windup bounded, max={max_integrator}"
        );
        // Final MV must be clamped at output_max even with the integrator held.
        let final_mv = pid.compute_output(100.0, timestamp);
        assert!(
            final_mv <= 100.0 + f32::EPSILON,
            "B6: MV must be clamped at output_max, got {final_mv}"
        );
    }

    #[test]
    fn b6_windup_clamps_at_output_min_with_negative_error() {
        // Negative-error rail: target below PV (cooling scenario). The
        // integrator must not accumulate negative headroom past output_min.
        let mut pid = CoffeeRoasterPid::with_gains(5.0, 0.5, 0.05);
        pid.set_output_limits(0.0, 100.0);
        pid.enable();
        pid.set_target(0.0).unwrap();

        let mut timestamp = 0u32;
        let mut min_integrator = 0.0f32;
        for _ in 0..200 {
            let out = pid.compute_output(150.0, timestamp);
            pid.update_feedback(PidFeedback::new(out, out, false));
            timestamp += PID_SAMPLE_TIME_MS;
            min_integrator = min_integrator.min(pid.integrator_value());
        }
        assert!(
            min_integrator >= -1000.0,
            "B6: integrator must be anti-windup bounded (negative rail), min={min_integrator}"
        );
    }
}
