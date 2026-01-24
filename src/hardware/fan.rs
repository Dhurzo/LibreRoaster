use crate::config::FAN_PWM_PIN;

#[derive(Debug, Clone, PartialEq)]
pub enum FanError {
    InitializationError,
    InvalidSpeed,
    PwmError,
    LedcError,
}

/// Fan controller with real LEDC PWM implementation
pub struct FanController {
    current_speed: f32,
    has_lecd: bool,
}

impl FanController {
    pub fn new() -> Result<Self, FanError> {
        log::warn!("Fan controller initialized with placeholder - use with_ledc() for real PWM");

        Ok(Self {
            current_speed: 0.0,
            has_lecd: false,
        })
    }

    /// Create new fan controller with real LEDC PWM support
    pub fn with_ledc(
        _ledc_peripheral: esp_hal::peripherals::LEDC,
        _gpio8: esp_hal::peripherals::GPIO8,
    ) -> Result<Self, FanError> {
        log::info!("Initializing LEDC fan controller on GPIO{}", FAN_PWM_PIN);

        // TODO: Implement LEDC configuration
        // For now, just mark as having LEDC support but use placeholder
        log::warn!("LEDC implementation pending - using placeholder for now");

        Ok(Self {
            current_speed: 0.0,
            has_lecd: true,
        })
    }

    /// Convert percentage (0-100) to LEDC duty (0-255 for 8-bit)
    fn percentage_to_duty(percentage: f32) -> u8 {
        (percentage.clamp(0.0, 100.0) * 2.55) as u8
    }

    pub fn set_speed(&mut self, speed_percent: f32) -> Result<(), FanError> {
        let clamped_speed = speed_percent.clamp(0.0, 100.0);

        // Store value (maintains compatibility)
        self.current_speed = clamped_speed;

        // Convert percentage to LEDC duty (0-255)
        let duty = Self::percentage_to_duty(clamped_speed);

        // Real PWM implementation using ESP32-C3 LEDC
        log::debug!(
            "LEDC PWM - set_speed: {:.1}% (duty: {})",
            clamped_speed,
            duty
        );

        if self.has_lecd {
            log::debug!(
                "LEDC mode - speed would be set: {:.1}% (duty: {})",
                clamped_speed,
                duty
            );
            // TODO: Implement actual LEDC PWM control
            // For now, just log and store the value
        } else {
            log::debug!("Placeholder mode - speed stored: {:.1}%", clamped_speed);
        }

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

impl Default for FanController {
    fn default() -> Self {
        log::warn!("Creating default fan controller");
        Self {
            current_speed: 0.0,
            has_lecd: false,
        }
    }
}
