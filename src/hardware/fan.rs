//! ESP32-C3 fan control over an LEDC PWM channel.
//!
//! `FanController` wraps an `LedcChannelHandle`, converting a 0–100 % speed
//! into raw duty and choosing between smooth hardware fades and a direct write.
//! `emergency_set_speed` bypasses the fade for safety-critical shutdowns.

use crate::config::constants::FAN_PWM_RESOLUTION;
use crate::control::traits::Fan;
use crate::control::RoasterError;
use crate::hardware::ledc_bus::LedcChannelHandle;
use libm::floorf;
use log::{debug, info};

const FAN_MAX_DUTY: u32 = (1u32 << FAN_PWM_RESOLUTION) - 1;

/// Errors returned by fan control operations.
#[derive(Debug, Clone, PartialEq)]
pub enum FanError {
    /// Controller initialisation (LEDC attach) failed.
    InitializationError { source: &'static str },
    /// Requested speed was outside the valid range.
    InvalidSpeed { source: &'static str },
    /// LEDC duty write or fade start failed.
    PwmError { source: &'static str },
    /// Underlying LEDC/peripheral error.
    LedcError { source: &'static str },
}

impl embedded_hal::digital::Error for FanError {
    fn kind(&self) -> embedded_hal::digital::ErrorKind {
        match self {
            FanError::InitializationError { .. } => embedded_hal::digital::ErrorKind::Other,
            FanError::InvalidSpeed { .. } => embedded_hal::digital::ErrorKind::Other,
            FanError::PwmError { .. } => embedded_hal::digital::ErrorKind::Other,
            FanError::LedcError { .. } => embedded_hal::digital::ErrorKind::Other,
        }
    }
}

/// LEDC-driven fan controller (real hardware variant).
pub struct FanController<'a> {
    /// Optional attached LEDC channel; `None` means placeholder/no-hardware mode.
    ledc_handle: Option<LedcChannelHandle<'a>>,
    /// Last applied fan speed in percent (0–100).
    current_speed: f32,
}

const FADE_THRESHOLD_DUTY: u8 = 12;

impl<'a> FanController<'a> {
    /// Create a placeholder controller with no LEDC hardware attached.
    pub fn new() -> Result<Self, FanError> {
        info!("No LEDC peripherals available - fan control disabled");
        Ok(Self {
            ledc_handle: None,
            current_speed: 0.0,
        })
    }

    /// Create a controller driving the supplied LEDC channel handle.
    pub fn with_handle(handle: LedcChannelHandle<'a>) -> Result<Self, FanError> {
        info!("Fan controller attached to LEDC bus");
        Ok(Self {
            current_speed: handle.applied_percent(),
            ledc_handle: Some(handle),
        })
    }

    fn percentage_to_duty(percentage: f32) -> u8 {
        let clamped = percentage.clamp(0.0, 100.0);
        let scaled = clamped * FAN_MAX_DUTY as f32 / 100.0;
        let rounded = floorf(scaled + 0.5).min(FAN_MAX_DUTY as f32);
        rounded as u8
    }

    fn fade_duration(delta: u16) -> u16 {
        let base = delta * 4;
        base + 80
    }

    /// Emergency set speed: writes duty DIRECTLY to hardware, bypassing the LEDC fade engine.
    /// Use this during emergency shutdown where immediate fan response is safety-critical.
    /// Normal speed changes should use `set_speed()` which uses smooth hardware fading.
    pub fn emergency_set_speed(&mut self, percentage: f32) -> Result<(), FanError> {
        let duty = Self::percentage_to_duty(percentage);
        if let Some(handle) = &self.ledc_handle {
            handle
                .set_duty_raw(duty as u16)
                .map_err(|_| FanError::PwmError {
                    source: "emergency_set_duty_failed",
                })?;
        }
        self.current_speed = percentage;
        Ok(())
    }

    /// Set fan speed (0–100 %), using a hardware fade for large deltas.
    pub fn set_speed(&mut self, speed_percent: f32) -> Result<(), FanError> {
        let clamped_speed = speed_percent.clamp(0.0, 100.0);
        let target_duty = Self::percentage_to_duty(clamped_speed);

        if let Some(handle) = self.ledc_handle {
            // Bug DRH-1 (2026-07-26): start from the LIVE duty (DUTY_R), not
            // the cached config duty. If the previous fade is still running,
            // the cache holds its END target and the fade-vs-direct decision
            // (and the next fade's start) would jump to that target first —
            // a surge. DUTY_R is where the hardware actually is.
            let current_duty = handle.live_duty();
            let duty_delta = (target_duty as u16).abs_diff(current_duty);

            if duty_delta > FADE_THRESHOLD_DUTY as u16 {
                let duration = Self::fade_duration(duty_delta);
                // start_duty_fade expects percentage (0-100), convert raw duty
                let current_pct =
                    ((current_duty as u32 * 100 + FAN_MAX_DUTY / 2) / FAN_MAX_DUTY) as u8;
                let target_pct =
                    ((target_duty as u32 * 100 + FAN_MAX_DUTY / 2) / FAN_MAX_DUTY) as u8;
                debug!(
                    "Fan fade {}→{} duty ({}%→{}%), {}ms",
                    current_duty, target_duty, current_pct, target_pct, duration
                );
                handle
                    .start_duty_fade(current_pct, target_pct, duration)
                    .map_err(|_| FanError::PwmError {
                        source: "start_duty_fade_failed",
                    })?;
            } else {
                debug!("Fan set duty {} (delta {})", target_duty, duty_delta);
                handle
                    .set_duty_raw(target_duty as u16)
                    .map_err(|_| FanError::PwmError {
                        source: "set_duty_failed",
                    })?;
            }

            self.current_speed = handle.applied_percent();
        } else {
            debug!("Placeholder mode - speed stored: {:.1}%", clamped_speed);
            self.current_speed = clamped_speed;
        }

        Ok(())
    }

    pub fn get_speed(&self) -> f32 {
        self.current_speed
    }

    pub fn is_enabled(&self) -> bool {
        self.current_speed > 0.0
    }
}

impl<'a> Fan for FanController<'a> {
    fn set_speed(&mut self, duty: f32) -> Result<(), RoasterError> {
        self.set_speed(duty)
            .map_err(|_| RoasterError::HardwareError {
                source: Some("fan_set_speed"),
            })
    }

    fn emergency_set_speed(&mut self, percentage: f32) -> Result<(), RoasterError> {
        self.emergency_set_speed(percentage)
            .map_err(|_| RoasterError::HardwareError {
                source: Some("fan_emergency_set_speed"),
            })
    }

    fn get_speed(&self) -> f32 {
        self.current_speed
    }
}

impl<'a> Default for FanController<'a> {
    fn default() -> Self {
        info!("Creating default fan controller - no LEDC hardware");
        Self {
            ledc_handle: None,
            current_speed: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_digital_error_kind() {
        let err = FanError::InitializationError { source: "test" };
        assert!(matches!(
            err.kind(),
            embedded_hal::digital::ErrorKind::Other
        ));
    }
}
