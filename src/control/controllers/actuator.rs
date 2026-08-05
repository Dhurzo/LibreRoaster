use crate::config::constants::SsrHardwareStatus;
use crate::config::*;
use crate::control::traits::{Fan, Heater};
use crate::control::RoasterError;
use crate::control::SsrCycleGuard;
use alloc::boxed::Box;
use embassy_time::Instant;
use log::{error, info, warn};

const SSR_SLEW_RATE_PER_SEC: f32 = 50.0;

pub struct ActuatorController {
    heater: Box<dyn Heater + Send>,
    fan: Box<dyn Fan + Send>,
    ssr_guard: SsrCycleGuard,
    last_desired_output: f32,
    last_health_check: Option<Instant>,
    slewing_output: f32,
    last_slew_update: Option<Instant>,
}

impl ActuatorController {
    pub fn new(heater: Box<dyn Heater + Send>, fan: Box<dyn Fan + Send>) -> Self {
        Self {
            heater,
            fan,
            ssr_guard: SsrCycleGuard::new(),
            last_desired_output: 0.0,
            last_health_check: None,
            slewing_output: 0.0,
            last_slew_update: None,
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
            self.slewing_output = 0.0;
            self.last_slew_update = Some(now);
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
                let actual_output = if clamped > 0.0 {
                    let mut actual_output = self.slewing_output;

                    if let Some(last_update) = self.last_slew_update {
                        let dt_secs = now.duration_since(last_update).as_micros() as f32 * 1e-6;

                        if dt_secs > 0.0 {
                            let max_step = SSR_SLEW_RATE_PER_SEC * dt_secs;
                            let step = (clamped - actual_output).min(max_step);
                            actual_output = (actual_output + step).min(clamped);
                        }
                    } else {
                        actual_output = clamped;
                    }

                    actual_output
                } else {
                    clamped
                };

                self.slewing_output = actual_output;
                self.last_slew_update = Some(now);

                let power_result = self.heater.set_power(actual_output);
                self.capture_ssr_monitor_metrics(status);
                power_result?;
                self.ssr_guard.mark_cycle(now);
                status.ssr_output = actual_output;
                status.saturation_active = false;
                status.integrator_clamped = false;
                self.update_guard_busy_ms(now, status);
                Ok(actual_output)
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

    /// Force the heater to 0 % with `EMERGENCY_HEATER_OFF_RETRIES` attempts.
    ///
    /// Returns `true` if any attempt succeeded. On total failure the SSR
    /// hardware status escalates to `Error` (the heater's physical state is
    /// unknown, so the status must not claim availability).
    ///
    /// Bug B-L / B-H / B-E (2026-08-04): extracted from `emergency_shutdown`
    /// so the retry discipline is shared by every emergency path (internal
    /// traps, manual Artisan STOP, safety policy outcomes) instead of being
    /// re-implemented with single-shot log-only writes.
    pub fn force_heater_off(&mut self, status: &mut SystemStatus) -> bool {
        let mut ok = false;
        for attempt in 0..crate::config::constants::EMERGENCY_HEATER_OFF_RETRIES {
            if self.heater.set_power(0.0).is_ok() {
                ok = true;
                break;
            }
            log::warn!("EMERGENCY: Heater off attempt {} failed", attempt + 1);
        }
        if !ok {
            log::error!(
                "EMERGENCY: Heater FAILED to shut off after {} retries",
                crate::config::constants::EMERGENCY_HEATER_OFF_RETRIES
            );
            status.ssr_hardware_status = crate::config::constants::SsrHardwareStatus::Error;
        }
        self.capture_ssr_monitor_metrics(status);
        ok
    }

    /// Force the fan to 100 % with `EMERGENCY_FAN_RETRIES` attempts.
    ///
    /// Returns `true` if any attempt succeeded. `status.fan_output` is only
    /// written after a successful write, so the telemetry reflects the
    /// physical state: the previous single-shot paths wrote
    /// `fan_output = 100.0` unconditionally, letting the status claim full
    /// cooling while the fan never moved.
    pub fn force_fan_100(&mut self, status: &mut SystemStatus) -> bool {
        for attempt in 0..crate::config::constants::EMERGENCY_FAN_RETRIES {
            if self.fan.emergency_set_speed(100.0).is_ok() {
                status.fan_output = 100.0;
                return true;
            }
            log::warn!("EMERGENCY: Fan 100% attempt {} failed", attempt + 1);
        }
        log::error!(
            "EMERGENCY: Fan FAILED to reach 100% after {} retries",
            crate::config::constants::EMERGENCY_FAN_RETRIES
        );
        false
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
        self.slewing_output = 0.0;
        self.last_slew_update = None;

        // Bug B-L (2026-08-04): the heater AND the fan are both retried now.
        // Previously the heater got `EMERGENCY_HEATER_OFF_RETRIES` attempts
        // while the fan got a single attempt with log-only error handling,
        // and `status.fan_output = 100.0` was written even when the write
        // failed. `force_fan_100` only publishes the value on success.
        self.force_heater_off(status);
        self.force_fan_100(status);

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

    pub fn periodic_health_check(&mut self, now: Instant) {
        let due = match self.last_health_check {
            Some(last) => (now - last).as_millis() >= 1000,
            None => true,
        };
        if due {
            self.last_health_check = Some(now);
            // Convert Embassy Instant to ms so the heater impl can apply its
            // own rate-limiting against real wall-clock time (not fake ticks).
            let current_time_ms = now.as_millis() as u32;
            self.heater.periodic_health_check(current_time_ms);
        }
    }

    fn busy_window_ms(now: Instant, busy_until: Instant) -> u64 {
        if busy_until > now {
            busy_until.duration_since(now).as_millis()
        } else {
            0
        }
    }
}
