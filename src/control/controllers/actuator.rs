use crate::config::constants::SsrHardwareStatus;
use crate::config::*;
use crate::control::traits::{Fan, Heater};
use crate::control::RoasterError;
use crate::control::SsrCycleGuard;
use alloc::boxed::Box;
use embassy_time::Instant;
use log::{error, info, warn};

pub struct ActuatorController {
    heater: Box<dyn Heater + Send>,
    fan: Box<dyn Fan + Send>,
    ssr_guard: SsrCycleGuard,
    last_desired_output: f32,
}

impl ActuatorController {
    pub fn new(heater: Box<dyn Heater + Send>, fan: Box<dyn Fan + Send>) -> Self {
        Self {
            heater,
            fan,
            ssr_guard: SsrCycleGuard::new(),
            last_desired_output: 0.0,
        }
    }

    pub fn apply_guarded_heater(
        &mut self,
        desired: f32,
        now: Instant,
        reject_on_busy: bool,
        status: &mut SystemStatus,
    ) -> Result<f32, RoasterError> {
        let clamped = desired.clamp(0.0, 100.0);
        self.update_guard_busy_ms(now, status);

        if clamped <= 0.0 {
            let power_result = self.heater.set_power(0.0);
            self.capture_ssr_monitor_metrics(status);
            power_result?;
            status.ssr_output = 0.0;
            status.saturation_active = false;
            status.integrator_clamped = false;
            self.update_guard_busy_ms(now, status);
            return Ok(0.0);
        }

        match self.ssr_guard.next_cycle_allowed(now) {
            Ok(_) => {
                let power_result = self.heater.set_power(clamped);
                self.capture_ssr_monitor_metrics(status);
                power_result?;
                self.ssr_guard.mark_cycle(now);
                status.ssr_output = clamped;
                status.saturation_active = false;
                status.integrator_clamped = false;
                self.update_guard_busy_ms(now, status);
                Ok(clamped)
            }
            Err(busy_until) => {
                status.saturation_active = true;
                status.integrator_clamped = true;
                status.ssr_cycle_guard_busy_until_ms = Self::busy_window_ms(now, busy_until);
                warn!("SSR cycle busy until {:?}", busy_until);
                if reject_on_busy {
                    Err(RoasterError::InvalidState {
                        source: Some("ssr_cycle_busy"),
                    })
                } else {
                    Ok(status.ssr_output)
                }
            }
        }
    }

    pub fn set_fan_speed(
        &mut self,
        speed: f32,
        status: &mut SystemStatus,
    ) -> Result<(), RoasterError> {
        self.fan.set_speed(speed)?;
        status.fan_output = speed;
        Ok(())
    }

    pub fn emergency_shutdown(
        &mut self,
        reason: &str,
        status: &mut SystemStatus,
    ) -> Result<(), RoasterError> {
        error!("Emergency shutdown: {}", reason);
        status.state = crate::config::constants::RoasterState::Error;
        status.ssr_output = 0.0;
        status.ssr_cycle_guard_busy_until_ms = 0;

        if let Err(e) = self.heater.set_power(0.0) {
            log::error!("EMERGENCY: Heater FAILED to shut off: {:?}", e);
            // Fallback: try direct LEDC write
            // If the hardware has a direct GPIO for SSR, we'd drive it low here
            // For now, at minimum log the error
        }
        self.capture_ssr_monitor_metrics(status);
        if let Err(e) = self.fan.emergency_set_speed(100.0) {
            log::error!("EMERGENCY: Fan FAILED to reach 100%: {:?}", e);
        }
        status.fan_output = 100.0;

        Err(RoasterError::EmergencyShutdown {
            source: Some("emergency_shutdown"),
        })
    }

    pub fn capture_ssr_monitor_metrics(&mut self, status: &mut SystemStatus) {
        status.ssr_last_duty_delta_ticks = self.heater.last_duty_delta_ticks();
        status.ssr_retry_count = self.heater.last_retry_count();

        if status.ssr_last_duty_delta_ticks != 0 || status.ssr_retry_count != 0 {
            info!(
                "SSR monitor delta {} ticks, retries {}",
                status.ssr_last_duty_delta_ticks, status.ssr_retry_count
            );
        }
    }

    pub fn update_guard_busy_ms(&mut self, now: Instant, status: &mut SystemStatus) {
        let busy_until = self.ssr_guard.busy_until();
        status.ssr_cycle_guard_busy_until_ms = Self::busy_window_ms(now, busy_until);
    }

    pub fn get_ssr_hardware_status(&self) -> SsrHardwareStatus {
        self.heater.get_status()
    }

    pub fn set_heater_power(&mut self, power: f32) -> Result<(), RoasterError> {
        self.heater.set_power(power)
    }

    pub fn set_fan_raw(&mut self, speed: f32) -> Result<(), RoasterError> {
        self.fan.emergency_set_speed(speed)
    }

    pub fn set_last_desired_output(&mut self, output: f32) {
        self.last_desired_output = output;
    }

    pub fn last_desired_heater_output(&self) -> f32 {
        self.last_desired_output
    }

    pub fn ssr_guard_next_cycle_allowed(&self, now: Instant) -> Result<Instant, Instant> {
        self.ssr_guard.next_cycle_allowed(now)
    }

    pub fn ssr_guard_mark_cycle(&mut self, now: Instant) {
        self.ssr_guard.mark_cycle(now);
    }

    fn busy_window_ms(now: Instant, busy_until: Instant) -> u64 {
        if busy_until > now {
            busy_until.duration_since(now).as_millis()
        } else {
            0
        }
    }
}
