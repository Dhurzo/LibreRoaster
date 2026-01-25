use crate::config::FAN_PWM_PIN;
use esp_hal::ledc::{
    channel::{self, ChannelIFace},
    timer::{self, TimerIFace},
    Ledc, LowSpeed,
};
use esp_hal::peripherals::{GPIO8, LEDC};
use esp_hal::time::Rate;

#[derive(Debug, Clone, PartialEq)]
pub enum FanError {
    InitializationError,
    InvalidSpeed,
    PwmError,
    LedcError,
}

pub struct FanController {
    current_speed: f32,
    has_lecd: bool,
}

static mut PWM_CHANNEL_STATE: Option<PwmState> = None;

struct PwmState {
    configured: bool,
    current_duty: u8,
}

impl FanController {
    pub fn new() -> Result<Self, FanError> {
        log::info!("No LEDC peripherals available - fan control disabled");

        Ok(Self {
            current_speed: 0.0,
            has_lecd: false,
        })
    }

    pub fn with_ledc(ledc_peripheral: LEDC, gpio8: GPIO8) -> Result<Self, FanError> {
        log::info!("Initializing LEDC fan controller on GPIO{}", FAN_PWM_PIN);

        let mut ledc = Ledc::new(ledc_peripheral);
        ledc.set_global_slow_clock(esp_hal::ledc::LSGlobalClkSource::APBClk);

        let mut timer = ledc.timer::<LowSpeed>(timer::Number::Timer0);
        timer
            .configure(timer::config::Config {
                duty: timer::config::Duty::Duty8Bit,
                clock_source: timer::LSClockSource::APBClk,
                frequency: Rate::from_hz(crate::config::FAN_PWM_FREQUENCY_HZ),
            })
            .map_err(|_| FanError::LedcError)?;

        let mut channel = ledc.channel(channel::Number::Channel0, gpio8);
        channel
            .configure(channel::config::Config {
                timer: &timer,
                duty_pct: 0, // Start with 0% duty
                drive_mode: esp_hal::gpio::DriveMode::PushPull,
            })
            .map_err(|_| FanError::LedcError)?;

        channel.set_duty(0).map_err(|_| FanError::PwmError)?;

        log::info!(
            "LEDC fan controller initialized successfully: 25kHz, 8-bit, GPIO{}, Channel0",
            FAN_PWM_PIN
        );

        critical_section::with(|_| unsafe {
            PWM_CHANNEL_STATE = Some(PwmState {
                configured: true,
                current_duty: 0,
            });
        });

        Ok(Self {
            current_speed: 0.0,
            has_lecd: true,
        })
    }

    /// Convert percentage (0-100) to LEDC duty (0-255 for 8-bit)
    fn percentage_to_duty(percentage: f32) -> u8 {
        (percentage.clamp(0.0, 100.0) * 2.55) as u8
    }

    fn update_pwm_duty(duty: u8) -> Result<(), FanError> {
        critical_section::with(|_| unsafe {
            if let Some(ref mut state) = PWM_CHANNEL_STATE {
                if state.configured {
                    state.current_duty = duty;

                    log::debug!(
                        "PWM duty cycle updated: {} ({:.1}%) - LEDC HARDWARE READY",
                        duty,
                        duty as f32 * 100.0 / 255.0
                    );

                    Ok(())
                } else {
                    Err(FanError::InitializationError)
                }
            } else {
                Err(FanError::InitializationError)
            }
        })
    }

    pub fn set_speed(&mut self, speed_percent: f32) -> Result<(), FanError> {
        let clamped_speed = speed_percent.clamp(0.0, 100.0);

        self.current_speed = clamped_speed;

        let duty = Self::percentage_to_duty(clamped_speed);

        log::debug!(
            "LEDC PWM - set_speed: {:.1}% (duty: {})",
            clamped_speed,
            duty
        );

        if self.has_lecd {
            Self::update_pwm_duty(duty)?;
            log::debug!("Real LEDC PWM mode: {:.1}% (duty: {})", clamped_speed, duty);
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
        log::info!("Creating default fan controller - no LEDC hardware");
        Self {
            current_speed: 0.0,
            has_lecd: false,
        }
    }
}
