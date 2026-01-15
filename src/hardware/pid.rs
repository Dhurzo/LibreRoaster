#[derive(Debug, Clone, Copy)]
pub enum PidError {
    InvalidParameters,
}

pub struct PidController {
    // PID gains
    kp: f32,
    ki: f32,
    kd: f32,

    // State variables
    integral: f32,
    previous_error: f32,

    // Limits
    output_min: f32,
    output_max: f32,
    integral_limit: f32,

    // Timing
    sample_time_ms: u32,
    last_time: Option<u32>,
}

impl PidController {
    pub fn new(kp: f32, ki: f32, kd: f32, sample_time_ms: u32) -> Result<Self, PidError> {
        if kp < 0.0 || ki < 0.0 || kd < 0.0 || sample_time_ms == 0 {
            return Err(PidError::InvalidParameters);
        }

        Ok(PidController {
            kp,
            ki,
            kd,
            integral: 0.0,
            previous_error: 0.0,
            output_min: 0.0,
            output_max: 100.0,
            integral_limit: 100.0,
            sample_time_ms,
            last_time: None,
        })
    }

    pub fn set_limits(&mut self, min: f32, max: f32) {
        self.output_min = min;
        self.output_max = max;
        self.integral_limit = max;
    }

    pub fn reset(&mut self) {
        self.integral = 0.0;
        self.previous_error = 0.0;
        self.last_time = None;
    }

    pub fn compute(&mut self, setpoint: f32, measurement: f32, current_time: u32) -> f32 {
        // Calculate error
        let error = setpoint - measurement;

        // Calculate time delta
        let dt = if let Some(last_time) = self.last_time {
            let delta = current_time.saturating_sub(last_time) as f32;
            if delta < 0.0 {
                self.sample_time_ms as f32
            } else {
                delta
            }
        } else {
            self.sample_time_ms as f32
        };

        // Convert to seconds for calculations
        let dt_seconds = dt / 1000.0;

        // Proportional term
        let proportional = self.kp * error;

        // Integral term with anti-windup
        self.integral += error * dt_seconds;
        // Clamp integral term to prevent windup
        self.integral = self
            .integral
            .clamp(-self.integral_limit, self.integral_limit);
        let integral = self.ki * self.integral;

        // Derivative term
        let derivative = if dt_seconds > 0.0 {
            let derivative_term = (error - self.previous_error) / dt_seconds;
            self.kd * derivative_term
        } else {
            0.0
        };

        // Calculate total output
        let mut output = proportional + integral + derivative;

        // Clamp output to limits
        output = output.clamp(self.output_min, self.output_max);

        // Update state for next iteration
        self.previous_error = error;
        self.last_time = Some(current_time);

        output
    }

    pub fn get_gains(&self) -> (f32, f32, f32) {
        (self.kp, self.ki, self.kd)
    }

    pub fn set_gains(&mut self, kp: f32, ki: f32, kd: f32) -> Result<(), PidError> {
        if kp < 0.0 || ki < 0.0 || kd < 0.0 {
            return Err(PidError::InvalidParameters);
        }

        self.kp = kp;
        self.ki = ki;
        self.kd = kd;
        Ok(())
    }
}

// Coffee roaster specific PID controller
pub struct CoffeeRoasterPid {
    pid: PidController,
    target_temp: f32,
    max_temp: f32,
    enabled: bool,
}

impl CoffeeRoasterPid {
    pub fn new() -> Result<Self, PidError> {
        let mut pid = PidController::new(
            2.0,  // Kp - Proportional gain (optimized for coffee roaster)
            0.01, // Ki - Integral gain (small to prevent overshoot)
            0.5,  // Kd - Derivative gain (moderate for damping)
            100,  // Sample time in milliseconds (10Hz)
        )?;

        pid.set_limits(0.0, 100.0); // SSR output limit 0-100%

        Ok(CoffeeRoasterPid {
            pid,
            target_temp: 225.0, // Base roasting temperature
            max_temp: 250.0,    // Maximum safe temperature
            enabled: false,
        })
    }

    pub fn set_target(&mut self, temp: f32) -> Result<(), PidError> {
        if temp < 0.0 || temp > self.max_temp {
            return Err(PidError::InvalidParameters);
        }
        self.target_temp = temp;
        Ok(())
    }

    pub fn enable(&mut self) {
        self.enabled = true;
        self.pid.reset();
    }

    pub fn disable(&mut self) {
        self.enabled = false;
        self.pid.reset();
    }

    pub fn compute_output(&mut self, current_temp: f32, current_time: u32) -> f32 {
        if !self.enabled {
            return 0.0;
        }

        // Safety: shut down if temperature exceeds maximum
        if current_temp >= self.max_temp {
            return 0.0;
        }

        self.pid
            .compute(self.target_temp, current_temp, current_time)
    }

    pub fn get_target(&self) -> f32 {
        self.target_temp
    }

    pub fn get_max_temp(&self) -> f32 {
        self.max_temp
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pid_creation() {
        let pid = PidController::new(1.0, 0.1, 0.01, 100).unwrap();
        assert_eq!(pid.get_gains(), (1.0, 0.1, 0.01));
    }

    #[test]
    fn test_pid_invalid_parameters() {
        assert!(PidController::new(-1.0, 0.1, 0.01, 100).is_err());
        assert!(PidController::new(1.0, -0.1, 0.01, 100).is_err());
        assert!(PidController::new(1.0, 0.1, -0.01, 100).is_err());
        assert!(PidController::new(1.0, 0.1, 0.01, 0).is_err());
    }

    #[test]
    fn test_coffee_roaster_pid() {
        let mut cr_pid = CoffeeRoasterPid::new().unwrap();
        assert_eq!(cr_pid.get_target(), 225.0);
        assert_eq!(cr_pid.get_max_temp(), 250.0);
        assert!(!cr_pid.is_enabled());

        cr_pid.enable();
        assert!(cr_pid.is_enabled());

        let output = cr_pid.compute_output(200.0, 1000);
        assert!(output > 0.0);
    }
}
