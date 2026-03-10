use crate::config::PID_SAMPLE_TIME_MS;

const DEFAULT_KP: f32 = 2.0;
const DEFAULT_KI: f32 = 0.25;
const DEFAULT_KD: f32 = 0.05;
const OUTPUT_CAP: f32 = 100.0;
const SATURATION_EPSILON: f32 = 0.01;

/// Provides the latest actuator status so the PID can obey actual outputs and guard resets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PidFeedback {
    pub desired_output: f32,
    pub applied_output: f32,
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
        self.guard_busy || (self.desired_output - self.applied_output) > SATURATION_EPSILON
    }
}

pub struct CoffeeRoasterPid {
    kp: f32,
    ki: f32,
    kd: f32,
    integrator: f32,
    last_error: f32,
    derivative_rate: f32,
    last_update_ms: Option<u32>,
    target: f32,
    enabled: bool,
    integrator_clamped: bool,
    saturation_active: bool,
    last_feedback: Option<PidFeedback>,
}

#[derive(Debug)]
pub enum PidError {
    InitializationError,
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
            derivative_rate: 0.0,
            last_update_ms: None,
            target: 0.0,
            enabled: false,
            integrator_clamped: false,
            saturation_active: false,
            last_feedback: None,
        }
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_target(&mut self, target: f32) -> Result<(), PidError> {
        self.target = target;
        Ok(())
    }

    /// Pushes actuator/guard feedback into the PID so the integrator can gate itself when the hardware cannot accept more energy.
    pub fn update_feedback(&mut self, feedback: PidFeedback) {
        self.last_feedback = Some(feedback);
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

        if self.should_integrate() {
            self.integrator += error * dt;
            self.integrator_clamped = false;
        } else {
            self.integrator_clamped = true;
        }

        let derivative = if dt > 0.0 {
            let derivative = (error - self.last_error) / dt;
            self.derivative_rate = derivative;
            derivative
        } else {
            self.derivative_rate
        };

        self.last_error = error;
        self.saturation_active = self
            .last_feedback
            .map(|feedback| feedback.is_saturated())
            .unwrap_or(false);

        let mut mv = (self.kp * error) + (self.ki * self.integrator) + (self.kd * derivative);
        mv = mv.clamp(0.0, OUTPUT_CAP);
        mv = self.bound_to_actuator(mv);

        self.last_update_ms = Some(timestamp_ms);
        mv
    }

    fn delta_seconds(&self, timestamp_ms: u32) -> f32 {
        const DEFAULT_SECONDS: f32 = PID_SAMPLE_TIME_MS as f32 / 1000.0;

        if let Some(last_ms) = self.last_update_ms {
            let delta = timestamp_ms.saturating_sub(last_ms);
            if delta == 0 {
                return DEFAULT_SECONDS;
            }

            return (delta as f32) / 1000.0;
        }

        DEFAULT_SECONDS
    }

    fn should_integrate(&self) -> bool {
        self.last_feedback
            .map(|feedback| !feedback.is_saturated())
            .unwrap_or(true)
    }

    fn bound_to_actuator(&mut self, mv: f32) -> f32 {
        if let Some(feedback) = self.last_feedback {
            let applied = feedback.applied_output.clamp(0.0, OUTPUT_CAP);
            if mv > applied {
                self.integrator_clamped = true;
                return applied;
            }
        }

        mv
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
