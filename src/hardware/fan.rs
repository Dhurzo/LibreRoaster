use crate::control::traits::Fan;
use crate::control::RoasterError;
use crate::hardware::ledc_bus::LedcChannelHandle;
use core::marker::PhantomData;
use esp_hal::ledc::{channel::ChannelIFace, LowSpeed};
use libm::floorf;
use log::{debug, error, info};

#[derive(Debug, Clone, PartialEq)]
pub enum FanError {
    InitializationError { source: &'static str },
    InvalidSpeed { source: &'static str },
    PwmError { source: &'static str },
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

pub struct FanController<'a> {
    ledc_handle: Option<LedcChannelHandle<'a>>,
    current_speed: f32,
}

const FADE_THRESHOLD_DUTY: u8 = 12;

impl<'a> FanController<'a> {
    pub fn new() -> Result<Self, FanError> {
        info!("No LEDC peripherals available - fan control disabled");
        Ok(Self {
            ledc_handle: None,
            current_speed: 0.0,
        })
    }

    pub fn with_handle(handle: LedcChannelHandle<'a>) -> Result<Self, FanError> {
        info!("Fan controller attached to LEDC bus");
        Ok(Self {
            current_speed: handle.applied_percent(),
            ledc_handle: Some(handle),
        })
    }

    fn percentage_to_duty(percentage: f32) -> u8 {
        let clamped = percentage.clamp(0.0, 100.0);
        let scaled = clamped * 255.0 / 100.0;
        let rounded = floorf(scaled + 0.5).min(255.0);
        rounded as u8
    }

    fn fade_duration(delta: u8) -> u16 {
        let base = delta as u16 * 4;
        base + 80
    }

    pub fn set_speed(&mut self, speed_percent: f32) -> Result<(), FanError> {
        let clamped_speed = speed_percent.clamp(0.0, 100.0);
        let target_duty = Self::percentage_to_duty(clamped_speed);

        if let Some(handle) = self.ledc_handle {
            let current_duty = handle.applied_duty();
            let duty_delta = if target_duty >= current_duty {
                target_duty - current_duty
            } else {
                current_duty - target_duty
            };

            if duty_delta > FADE_THRESHOLD_DUTY {
                let duration = Self::fade_duration(duty_delta);
                debug!(
                    "Fan fade {} → {} (Δ{}), {}ms",
                    current_duty, target_duty, duty_delta, duration
                );
                handle
                    .start_duty_fade(current_duty, target_duty, duration)
                    .map_err(|_| FanError::PwmError {
                        source: "start_duty_fade_failed",
                    })?;
            } else {
                debug!("Fan set duty {} (delta {})", target_duty, duty_delta);
                handle
                    .set_duty(target_duty)
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

    pub fn enable(&mut self) {
        if let Err(_) = self.set_speed(100.0) {
            error!("Failed to enable fan");
        } else {
            info!("Fan enabled at 100%");
        }
    }

    pub fn disable(&mut self) {
        if let Err(_) = self.set_speed(0.0) {
            error!("Failed to disable fan");
        } else {
            info!("Fan disabled");
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.current_speed > 0.0
    }
}

impl<'a> Fan for FanController<'a> {
    fn set_speed(&mut self, duty: f32) -> Result<(), RoasterError> {
        self.set_speed(duty)
            .map_err(|_| RoasterError::HardwareError)
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

pub struct SimpleLedcFan<'a, C>
where
    C: ChannelIFace<'a, LowSpeed>,
{
    channel: C,
    _phantom: PhantomData<&'a ()>,
}

impl<'a, C> SimpleLedcFan<'a, C>
where
    C: ChannelIFace<'a, LowSpeed>,
{
    pub fn new(channel: C) -> Self {
        Self {
            channel,
            _phantom: PhantomData,
        }
    }
}

impl<'a, C> Fan for SimpleLedcFan<'a, C>
where
    C: ChannelIFace<'a, LowSpeed>,
{
    fn set_speed(&mut self, speed_percent: f32) -> Result<(), RoasterError> {
        let clamped = speed_percent.clamp(0.0, 100.0);
        let max_duty = 255;
        let duty = ((clamped / 100.0) * max_duty as f32) as u32;

        self.channel
            .set_duty(duty as u8)
            .map_err(|_| RoasterError::HardwareError)?;

        debug!("SimpleLedcFan set to {:.1}% (duty {})", clamped, duty);
        Ok(())
    }
}

unsafe impl<'a, C> Send for SimpleLedcFan<'a, C> where C: ChannelIFace<'a, LowSpeed> {}

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
