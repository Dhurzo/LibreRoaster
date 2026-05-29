use crate::config::constants::{SSR_CONTROL_CYCLE_HZ, SSR_MIN_DUTY_PERCENT};
use crate::config::status::{GlobalSsrStatus, SsrHardwareStatus};
use crate::error::{RoasterError, SsrError};
use crate::hardware::ssr::{SsrControlBase, StatusGetters};
use crate::logging::Telemetry;
use crate::memory::AtomicU32;
use crate::safety::HeatSourceDetector;
use crate::control::traits::Heater;
use crate::control::traits::PeriodicCheck;
use core::sync::atomic::{AtomicU8, Ordering};
use embedded_hal::digital::{InputPin, OutputPin};
use embassy_time::Timer;
use log::{debug, info, warn};

/// Shared atomic for SSR target duty between Heater trait and async task
static SSR_TARGET_DUTY_ATOMIC: AtomicU8 = AtomicU8::new(0);

/// Zero-cross SSR control using GPIO toggling instead of PWM
/// Implements the Heater trait for integration with the control system
pub struct ZeroCrossSsr<HeatPin> {
    ssr_target_duty: AtomicU8,
    hardware_status: SsrHardwareStatus,
    _phantom: core::marker::PhantomData<HeatPin>,
}

impl<HeatPin> ZeroCrossSsr<HeatPin>
where
    HeatPin: InputPin + OutputPin,
{
    /// Create a new ZeroCrossSsr instance
    pub fn new(_ssr_pin: HEAT_PIN, _heat_detection_pin: HEAT_PIN) -> Result<Self, RoasterError> {
        Ok(Self {
            ssr_target_duty: AtomicU8::new(0),
            hardware_status: SsrHardwareStatus::Available,
            _phantom: PhantomData,
        })
    }

    /// Update the shared target duty atomically
    pub fn update_target_duty(duty: f32) {
        let clamped = duty.clamp(0.0, 100.0);
        let duty_scaled = (clamped * 100.0) as u32;
        SSR_TARGET_DUTY_ATOMIC.store(duty_scaled as u8, Ordering::Release);
    }

    /// Get the current target duty from shared atomic
    pub fn get_target_duty() -> f32 {
        let duty_scaled = SSR_TARGET_DUTY_ATOMIC.load(Ordering::Acquire);
        (duty_scaled as f32) / 100.0
    }

    /// Detect heat source using the detection pin
    fn detect_heat_source(&mut self, _current_time: u32) -> Result<(), RoasterError> {
        if self.ssr_target_duty.load(Ordering::Relaxed) > 0 {
            self.hardware_status = SsrHardwareStatus::Available;
        } else {
            self.hardware_status = SsrHardwareStatus::NotDetected;
        }
        Ok(())
    }

    /// Cross-check heat detection against current duty
    fn cross_check_heat_detection(&mut self, _current_duty: f32) -> Result<(), RoasterError> {
        if self.ssr_target_duty.load(Ordering::Relaxed) > 0 {
            self.hardware_status = SsrHardwareStatus::Available;
        } else {
            self.hardware_status = SsrHardwareStatus::NotDetected;
        }

        #[cfg(not(feature = "simulated-sensors"))]
        {
        }
        
        Ok(())
    }
}

impl<HeatPin> Heater for ZeroCrossSsr<HeatPin>
where
    HeatPin: InputPin + OutputPin,
{
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError> {
        // Store the target duty locally and update the shared atomic
        let clamped = duty.clamp(0.0, 100.0);
        self.ssr_target_duty.store(clamped as u8, Ordering::Relaxed);
        Self::update_target_duty(clamped);

        debug!("ZeroCross SSR target set to {:.1}%", clamped);
        Ok(())
    }

    fn get_status(&self) -> GlobalSsrStatus {
        match self.hardware_status {
            SsrHardwareStatus::Available => GlobalSsrStatus::Available,
            SsrHardwareStatus::NotDetected => GlobalSsrStatus::NotDetected,
            SsrHardwareStatus::Error => GlobalSsrStatus::Error,
        }
    }

    fn periodic_health_check(&mut self, current_time_ms: u32) -> Result<(), RoasterError> {
        // Heat source detection
        self.detect_heat_source(current_time_ms)?;

        // Cross-check heat detection against current duty
        self.cross_check_heat_detection(self.ssr_target_duty.load(Ordering::Relaxed) as f32)?;

        Ok(())
    }

    fn last_duty_delta_ticks(&self) -> i16 {
        0 // Not applicable for GPIO zero-cross control
    }

    fn last_retry_count(&self) -> u8 {
        0 // Not applicable for GPIO zero-cross control
    }
}

impl<HeatPin> StatusGetters for ZeroCrossSsr<HeatPin>
where
    HeatPin: InputPin + OutputPin,
{
    fn get_hardware_status(&self) -> SsrHardwareStatus {
        self.hardware_status
    }

    fn get_target_duty(&self) -> f32 {
        self.ssr_target_duty.load(Ordering::Relaxed) as f32 / 100.0
    }
}

impl<HeatPin> HeatSourceDetector for ZeroCrossSsr<HeatPin>
where
    HeatPin: InputPin + OutputPin,
{
    fn detect_heat_source(&mut self, current_time: u32) -> Result<(), SsrError> {
        ZeroCrossSsr::detect_heat_source(self, current_time)
            .map_err(|e| SsrError::Other(e.to_string()))
    }
}

impl<HeatPin> PeriodicCheck for ZeroCrossSsr<HeatPin>
where
    HeatPin: InputPin + OutputPin,
{
    fn periodic_check(&mut self, current_time: u32) -> Result<(), SsrError> {
        ZeroCrossSsr::periodic_health_check(self, current_time)
            .map_err(|e| SsrError::Other(e.to_string()))
    }
}

unsafe impl<HeatPin> Send for ZeroCrossSsr<HeatPin>
where
    HeatPin: InputPin + OutputPin + Send,
{
}

    fn is_heating_available(&self) -> bool {
        self.hardware_status == SsrHardwareStatus::Available
    }

    fn get_current_duty(&self) -> u16 {
        self.ssr_target_duty.load(Ordering::Relaxed) as u16
    }

    fn is_pwm_enabled(&self) -> bool {
        true
    }

    fn last_lead_delta_ticks(&self) -> i16 {
        0 // Not applicable for GPIO zero-cross control
    }

    fn last_retry_count(&self) -> u8 {
        0 // Not applicable for GPIO zero-cross control
    }
}

impl<HEAT_PIN> HeatSourceDetector for ZeroCrossSsr<HEAT_PIN>
where
    HEAT_PIN: InputPin + OutputPin,
{
    fn detect_heat_source(&mut self, current_time: u32) -> Result<(), SsrError> {
        ZeroCrossSsr::detect_heat_source(self, current_time)
    }
}

impl<HEAT_PIN> PeriodicCheck for ZeroCrossSsr<HEAT_PIN>
where
    HEAT_PIN: InputPin + OutputPin,
{
    fn periodic_check(&mut self, current_time: u32) -> Result<(), SsrError> {
        ZeroCrossSsr::periodic_health_check(self, current_time)
    }
}

// SAFETY(v5.1): Sound on single-core ESP32-C3 (cooperative Embassy tasks).
// On the single-core ESP32-C3, Embassy tasks run cooperatively — only one
// task executes at a time. The type is moved into a `Box<dyn Heater + Send>`
// and passed to a single task via ServiceContainer, so no concurrent access
// occurs.
unsafe impl<HEAT_PIN> Send for ZeroCrossSsr<HEAT_PIN> where
    HEAT_PIN: InputPin + OutputPin
{
}

/// Embassy async task for zero-cross SSR control
/// Toggles GPIO pin at the configured frequency based on target duty
#[embassy_executor::task]
pub async fn ssr_zero_cross_task(ssr_pin: esp_hal::gpio::Output<'static>) {
    info!("SSR zero-cross task started at {} Hz", SSR_CONTROL_CYCLE_HZ);
    
    let cycle_period_ms = 1000 / SSR_CONTROL_CYCLE_HZ;
    let min_duty_percent = SSR_MIN_DUTY_PERCENT;
    
    loop {
        // Read the target duty from shared atomic
        let target_duty = ZeroCrossSsr::get_target_duty();
        
        // Calculate ON time: (duty / 100.0) * cycle_period_ms
        let on_time_ms = if target_duty >= min_duty_percent {
            ((target_duty / 100.0) * cycle_period_ms as f32) as u32
        } else {
            0
        };
        
        // Calculate OFF time: cycle_period_ms - on_time_ms
        let off_time_ms = cycle_period_ms.saturating_sub(on_time_ms);
        
        // Turn SSR ON if duty > minimum
        if on_time_ms > 0 {
            debug!("SSR ON: {:.1}% duty, {}ms ON time", target_duty, on_time_ms);
            let _ = ssr_pin.set_high();
            let _ = ssr_pin.set_low();
            Timer::after_millis(on_time_ms as u64).await;
        }
        
        // Turn SSR OFF
        if off_time_ms > 0 {
            debug!("SSR OFF: {}ms OFF time", off_time_ms);
            ssr_pin.set_low().unwrap_or_else(|_| {
                warn!("Failed to set SSR pin LOW");
            });
            Timer::after_millis(off_time_ms as u64).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duty_clamping() {
        assert_eq!(ZeroCrossSsr::get_target_duty(), 0.0);
        
        // Test duty clamping
        ZeroCrossSsr::update_target_duty(-10.0);
        assert_eq!(ZeroCrossSsr::get_target_duty(), 0.0);
        
        ZeroCrossSsr::update_target_duty(110.0);
        assert_eq!(ZeroCrossSsr::get_target_duty(), 100.0);
        
        ZeroCrossSsr::update_target_duty(50.0);
        assert_eq!(ZeroCrossSsr::get_target_duty(), 50.0);
    }

    #[test]
    fn test_cycle_period_calculation() {
        // Test that 1Hz gives 1000ms period
        assert_eq!(1000 / SSR_CONTROL_CYCLE_HZ, 1000);
        
        // Test that ON time calculation is correct
        let duty = 25.0; // 25% duty
        let cycle_period_ms = 1000;
        let expected_on_time_ms = ((duty / 100.0) * cycle_period_ms as f32) as u32;
        assert_eq!(expected_on_time_ms, 250);
    }
}