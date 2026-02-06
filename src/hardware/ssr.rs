use core::marker::PhantomData;
use embedded_hal::digital::{InputPin, OutputPin};
use esp_hal::ledc::channel::ChannelIFace;
use esp_hal::ledc::LowSpeed;
use log::{debug, error, info, warn};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SsrError {
    OutputError,
    InputError,
    HeatSourceNotDetected,
    PwmError,
}

impl embedded_hal::digital::Error for SsrError {
    fn kind(&self) -> embedded_hal::digital::ErrorKind {
        embedded_hal::digital::ErrorKind::Other
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SsrHardwareStatus {
    Available,
    NotDetected,
    Error,
}

pub struct SsrControl<'a, PIN, DETECT, PWM>
where
    PIN: OutputPin,
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed>,
{
    #[allow(dead_code)]
    // Stored for ownership - pin is set low during initialization and kept alive
    // to prevent accidental reconfiguration by the HAL. PWM controls actual SSR output.
    pin: PIN,
    detection_pin: DETECT,
    pwm_channel: PWM,
    hardware_status: SsrHardwareStatus,
    current_duty: u16,
    last_detection_check: Option<u32>,
    is_pwm_enabled: bool,
    _phantom: PhantomData<&'a ()>,
}

impl<'a, PIN, DETECT, PWM> SsrControl<'a, PIN, DETECT, PWM>
where
    PIN: OutputPin,
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed>,
{
    pub fn new_with_pwm_and_detection(
        mut pin: PIN,
        detection_pin: DETECT,
        pwm_channel: PWM,
    ) -> Result<Self, SsrError> {
        pin.set_low().map_err(|_| SsrError::OutputError)?;
        pwm_channel.set_duty(0).map_err(|_| SsrError::PwmError)?;

        let mut ssr = SsrControl {
            pin,
            detection_pin,
            pwm_channel,
            hardware_status: SsrHardwareStatus::NotDetected,
            current_duty: 0,
            last_detection_check: None,
            is_pwm_enabled: true,
            _phantom: PhantomData,
        };

        ssr.detect_heat_source(0)?;

        info!(
            "SSR control initialized with PWM - heat source: {:?}",
            ssr.hardware_status
        );
        Ok(ssr)
    }

    fn detect_heat_source(&mut self, current_time: u32) -> Result<(), SsrError> {
        match self.detection_pin.is_low() {
            Ok(is_detected) => {
                let new_status = if is_detected {
                    SsrHardwareStatus::Available
                } else {
                    SsrHardwareStatus::NotDetected
                };

                if new_status != self.hardware_status {
                    match new_status {
                        SsrHardwareStatus::Available => {
                            info!("Heat source detected - SSR heating operational");
                        }
                        SsrHardwareStatus::NotDetected => {
                            warn!("Heat source not detected - SSR commands work but no heat generated");
                        }
                        _ => {}
                    }
                    self.hardware_status = new_status;
                }

                self.last_detection_check = Some(current_time);
                Ok(())
            }
            Err(_) => {
                if self.hardware_status != SsrHardwareStatus::Error {
                    error!("SSR detection pin error - switching to error state");
                    self.hardware_status = SsrHardwareStatus::Error;
                }
                Err(SsrError::InputError)
            }
        }
    }

    pub fn periodic_check(&mut self, current_time: u32) -> Result<(), SsrError> {
        let should_check = if let Some(last_check) = self.last_detection_check {
            current_time.saturating_sub(last_check) >= crate::config::HEAT_SOURCE_CHECK_INTERVAL_MS
        } else {
            true
        };

        if should_check {
            self.detect_heat_source(current_time)?;
        }

        Ok(())
    }

    pub fn get_hardware_status(&self) -> SsrHardwareStatus {
        self.hardware_status
    }

    pub fn is_heating_available(&self) -> bool {
        self.hardware_status == SsrHardwareStatus::Available
    }

    pub fn set_percentage(&mut self, percentage: f32) -> Result<(), SsrError> {
        let clamped = percentage.clamp(0.0, 100.0);
        let max_duty = 255; // Assuming 8-bit PWM resolution (0-255)
        let duty = ((clamped / 100.0) * max_duty as f32) as u32;

        self.pwm_channel
            .set_duty((duty / 100) as u8)
            .map_err(|_| SsrError::PwmError)?;
        self.current_duty = duty as u16;

        debug!(
            "SSR set to {:.1}% (duty {}), heat available: {}",
            clamped,
            duty,
            self.is_heating_available()
        );

        Ok(())
    }

    pub fn get_current_duty(&self) -> u16 {
        self.current_duty
    }

    pub fn is_pwm_enabled(&self) -> bool {
        self.is_pwm_enabled
    }
}

pub struct SsrControlSimple<'a, DETECT, PWM>
where
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed>,
{
    detection_pin: DETECT,
    pwm_channel: PWM,
    hardware_status: SsrHardwareStatus,
    current_duty: u16,
    last_detection_check: Option<u32>,
    is_pwm_enabled: bool,
    _phantom: PhantomData<&'a ()>,
}

impl<'a, DETECT, PWM> SsrControlSimple<'a, DETECT, PWM>
where
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed>,
{
    pub fn new(detection_pin: DETECT, pwm_channel: PWM) -> Result<Self, SsrError> {
        pwm_channel.set_duty(0).map_err(|_| SsrError::PwmError)?;

        let mut ssr = SsrControlSimple {
            detection_pin,
            pwm_channel,
            hardware_status: SsrHardwareStatus::NotDetected,
            current_duty: 0,
            last_detection_check: None,
            is_pwm_enabled: true,
            _phantom: PhantomData,
        };

        ssr.detect_heat_source(0)?;

        info!(
            "SSR control initialized (simple mode) - heat source: {:?}",
            ssr.hardware_status
        );
        Ok(ssr)
    }

    fn detect_heat_source(&mut self, current_time: u32) -> Result<(), SsrError> {
        match self.detection_pin.is_low() {
            Ok(is_detected) => {
                let new_status = if is_detected {
                    SsrHardwareStatus::Available
                } else {
                    SsrHardwareStatus::NotDetected
                };

                if new_status != self.hardware_status {
                    match new_status {
                        SsrHardwareStatus::Available => {
                            info!("Heat source detected - SSR heating operational");
                        }
                        SsrHardwareStatus::NotDetected => {
                            warn!("Heat source not detected - SSR commands work but no heat generated");
                        }
                        _ => {}
                    }
                    self.hardware_status = new_status;
                }

                self.last_detection_check = Some(current_time);
                Ok(())
            }
            Err(_) => {
                if self.hardware_status != SsrHardwareStatus::Error {
                    error!("SSR detection pin error - switching to error state");
                    self.hardware_status = SsrHardwareStatus::Error;
                }
                Err(SsrError::InputError)
            }
        }
    }

    pub fn periodic_check(&mut self, current_time: u32) -> Result<(), SsrError> {
        let should_check = if let Some(last_check) = self.last_detection_check {
            current_time.saturating_sub(last_check) >= crate::config::HEAT_SOURCE_CHECK_INTERVAL_MS
        } else {
            true
        };

        if should_check {
            self.detect_heat_source(current_time)?;
        }

        Ok(())
    }

    pub fn get_hardware_status(&self) -> SsrHardwareStatus {
        self.hardware_status
    }

    pub fn is_heating_available(&self) -> bool {
        self.hardware_status == SsrHardwareStatus::Available
    }

    pub fn set_percentage(&mut self, percentage: f32) -> Result<(), SsrError> {
        let clamped = percentage.clamp(0.0, 100.0);
        let max_duty = 255;
        let duty = ((clamped / 100.0) * max_duty as f32) as u32;

        self.pwm_channel
            .set_duty((duty / 100) as u8)
            .map_err(|_| SsrError::PwmError)?;
        self.current_duty = duty as u16;

        debug!(
            "SSR set to {:.1}% (duty {}), heat available: {}",
            clamped,
            duty,
            self.is_heating_available()
        );

        Ok(())
    }

    pub fn get_current_duty(&self) -> u16 {
        self.current_duty
    }

    pub fn is_pwm_enabled(&self) -> bool {
        self.is_pwm_enabled
    }
}

impl<'a, DETECT, PWM> Heater for SsrControlSimple<'a, DETECT, PWM>
where
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed>,
{
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError> {
        self.set_percentage(duty)
            .map_err(|_| RoasterError::HardwareError)
    }

    fn get_status(&self) -> GlobalSsrStatus {
        match self.get_hardware_status() {
            SsrHardwareStatus::Available => GlobalSsrStatus::Available,
            SsrHardwareStatus::NotDetected => GlobalSsrStatus::NotDetected,
            SsrHardwareStatus::Error => GlobalSsrStatus::Error,
        }
    }
}

unsafe impl<'a, DETECT, PWM> Send for SsrControlSimple<'a, DETECT, PWM>
where
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed>,
{
}

use crate::config::constants::SsrHardwareStatus as GlobalSsrStatus;
use crate::control::traits::Heater;
use crate::control::RoasterError;

impl<'a, PIN, DETECT, PWM> Heater for SsrControl<'a, PIN, DETECT, PWM>
where
    PIN: OutputPin,
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed>,
{
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError> {
        self.set_percentage(duty)
            .map_err(|_| RoasterError::HardwareError)
    }

    fn get_status(&self) -> GlobalSsrStatus {
        match self.get_hardware_status() {
            SsrHardwareStatus::Available => GlobalSsrStatus::Available,
            SsrHardwareStatus::NotDetected => GlobalSsrStatus::NotDetected,
            SsrHardwareStatus::Error => GlobalSsrStatus::Error,
        }
    }
}

// SAFETY: SsrControl has exclusive access to its peripherals.
// On single-core ESP32-C3 with Embassy, passing ownership between tasks is safe
// as long as we don't access it concurrently (which ownership prevents).
// The inner Channel contains a non-Sync reference to Timer, preventing auto-Send.
unsafe impl<'a, PIN, DETECT, PWM> Send for SsrControl<'a, PIN, DETECT, PWM>
where
    PIN: OutputPin,
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed>,
{
}
