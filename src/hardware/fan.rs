use crate::config::FAN_PWM_PIN;

#[derive(Debug, Clone, PartialEq)]
pub enum FanError {
    InitializationError,
    InvalidSpeed,
    PwmError,
}

/// Simple fan controller placeholder
/// TODO: Implement proper PWM when esp-hal API is stable
pub struct FanController {
    current_speed: f32,
}

impl FanController {
    pub fn new() -> Result<Self, FanError> {
        log::info!(
            "Fan controller initialized (placeholder - GPIO{})",
            FAN_PWM_PIN
        );

        Ok(Self { current_speed: 0.0 })
    }

    pub fn set_speed(&mut self, speed_percent: f32) -> Result<(), FanError> {
        let clamped_speed = speed_percent.clamp(0.0, 100.0);

        // TODO: Implement proper PWM control
        // For now, just store the value and log
        self.current_speed = clamped_speed;
        log::info!(
            "Fan speed set to: {}% (placeholder - no PWM yet)",
            clamped_speed
        );
        Ok(())
    }

    pub fn get_speed(&self) -> f32 {
        self.current_speed
    }

    pub fn enable(&mut self) {
        if let Err(_) = self.set_speed(100.0) {
            log::error!("Failed to enable fan");
        } else {
            log::info!("Fan enabled at 100%");
        }
    }

    pub fn disable(&mut self) {
        if let Err(_) = self.set_speed(0.0) {
            log::error!("Failed to disable fan");
        } else {
            log::info!("Fan disabled");
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.current_speed > 0.0
    }
}
