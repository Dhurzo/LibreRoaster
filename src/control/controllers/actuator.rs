use crate::config::constants::SsrHardwareStatus;
use crate::config::*;
use crate::control::traits::{Fan, Heater};
use crate::control::RoasterError;
use crate::control::SsrCycleGuard;
use crate::logging::edge_log_gate::EdgeLogGate;
use alloc::boxed::Box;
use embassy_time::Instant;
use log::{debug, error, info, warn};

const SSR_SLEW_RATE_PER_SEC: f32 = 50.0;

pub struct ActuatorController {
    heater: Box<dyn Heater + Send>,
    fan: Box<dyn Fan + Send>,
    ssr_guard: SsrCycleGuard,
    last_desired_output: f32,
    slewing_output: f32,
    last_slew_update: Option<Instant>,
    /// BUG-04: rising-edge gate for the "SSR cycle busy" warn (one warn per
    /// busy episode; re-arms when a write is accepted).
    cycle_busy_gate: EdgeLogGate,
}

impl ActuatorController {
    pub fn new(heater: Box<dyn Heater + Send>, fan: Box<dyn Fan + Send>) -> Self {
        Self {
            heater,
            fan,
            ssr_guard: SsrCycleGuard::new(),
            last_desired_output: 0.0,
            slewing_output: 0.0,
            last_slew_update: None,
            cycle_busy_gate: EdgeLogGate::new(),
        }
    }

    pub fn apply_guarded_heater(
        &mut self,
        desired: f32,
        now: Instant,
        reject_on_busy: bool,
        status: &mut SystemStatus,
    ) -> Result<f32, RoasterError> {
        // Bug S5 (2026-08-05): NaN/±Inf must never reach the clamp/slew/status
        // logic. `NaN.clamp(0.0, 100.0)` passes NaN through, the slew computes
        // NaN, and `status.ssr_output = NaN` disarms the comms-idle and
        // MAX_ROAST_TIME backstops (`NaN > 0.0 == false`). Rejecting with an
        // Err is fail-safe: the control loop escalates to emergency shutdown,
        // and a command path surfaces `ERR handler_failed`.
        if !desired.is_finite() {
            return Err(RoasterError::InvalidState {
                source: Some("non_finite_heater_output"),
            });
        }
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
            self.cycle_busy_gate.rising(false);
            return Ok(0.0);
        }

        match self.ssr_guard.next_cycle_allowed(now) {
            Ok(_) => {
                self.cycle_busy_gate.rising(false);
                let actual_output = if clamped > 0.0 {
                    let mut actual_output = self.slewing_output;

                    if let Some(last_update) = self.last_slew_update {
                        let dt_secs =
                            now.saturating_duration_since(last_update).as_micros() as f32 * 1e-6;

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
                // BUG-04: the guard can stay busy across several ticks while
                // the control loop keeps retrying; warn once per busy
                // episode instead of every tick (protocol channel integrity).
                if self.cycle_busy_gate.rising(true) {
                    warn!("SSR cycle busy until {:?}", busy_until);
                } else {
                    debug!("SSR cycle busy until {:?}", busy_until);
                }
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
        // Bug L1 (2026-08-10): resync the slew limiter. `force_heater_off`
        // writes 0 % straight through the SSR driver, bypassing
        // `apply_guarded_heater`'s off-branch (which is what normally resets
        // `slewing_output`). If a STOP and an `OT1` land in the same command
        // drain with no zero-output control tick in between, the limiter
        // would otherwise start the next ramp from the stale pre-stop value.
        // With `last_slew_update = None` the next `apply_guarded_heater`
        // applies the commanded value directly — same semantics as
        // `emergency_shutdown` below.
        self.slewing_output = 0.0;
        self.last_slew_update = None;
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
        // Bug S7 (2026-08-05): `ssr_output` is NOT zeroed unconditionally
        // anymore. If the heater is physically stuck ON (`force_heater_off`
        // failed every attempt), telemetry must not claim 0 % — the honest
        // signal is `ssr_hardware_status = Error`. `ssr_output` only drops to
        // 0 once the off-write actually succeeds (here, or via the next
        // control-tick `apply_guarded_heater(0.0)` off-branch).
        status.ssr_cycle_guard_busy_until_ms = 0;
        self.slewing_output = 0.0;
        self.last_slew_update = None;

        // Bug B-L (2026-08-04): the heater AND the fan are both retried now.
        // Previously the heater got `EMERGENCY_HEATER_OFF_RETRIES` attempts
        // while the fan got a single attempt with log-only error handling,
        // and `status.fan_output = 100.0` was written even when the write
        // failed. `force_fan_100` only publishes the value on success.
        let heater_off_ok = self.force_heater_off(status);
        if heater_off_ok {
            status.ssr_output = 0.0;
        }
        let fan_ok = self.force_fan_100(status);

        // Bug S4 (2026-08-05): internal-trap emergencies (overtemp, NaN, RoR,
        // watchdog) used to absorb a total fan failure silently — the caller
        // only ever saw `EmergencyShutdown`. Same rule as `stop_streaming` and
        // the command paths (B-E / B-H): no fan at 100 % means unsafe to
        // continue, so escalate with the fan-failure variant.
        if !fan_ok {
            log::error!(
                "EMERGENCY: Fan FAILED to reach 100% after {} retries — unsafe to continue",
                crate::config::constants::EMERGENCY_FAN_RETRIES
            );
            return Err(RoasterError::HardwareError {
                source: Some("emergency_fan_failed"),
            });
        }

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

    /// BUG-02 (2026-08-21): propagate the explicit-recovery re-arm to the
    /// heater driver and refresh the status published in telemetry. Called
    /// only from `clear_emergency_explicit` / `handle_stop` (operator
    /// recovery), never from internal stop paths.
    pub fn rearm_heater_hardware_status(&mut self, status: &mut SystemStatus) {
        self.heater.rearm_hardware_status();
        status.ssr_hardware_status = self.heater.get_status();
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
        // Bug H7 (2026-08-10): the 1000 ms gate was removed — it quantized
        // the sampling to 4 ticks (1240 ms real cadence → 40 ms of PWM phase
        // separation at 5 Hz), which broke the aliasing-proof argument that
        // `heat_presence`'s debounce relies on (consecutive samples must be
        // one control-loop tick apart, ~330 ms → ~130 ms of phase separation,
        // provably in the (100, 200) ms band). The heater impl no longer
        // rate-limits internally (bug audit 2026-08-02 in ssr.rs), so the
        // gate here was the only residual throttle. A GPIO read per tick is
        // negligible; this restores the documented cadence.
        let current_time_ms = now.as_millis() as u32;
        self.heater.periodic_health_check(current_time_ms);
    }

    fn busy_window_ms(now: Instant, busy_until: Instant) -> u64 {
        if busy_until > now {
            busy_until.saturating_duration_since(now).as_millis()
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod proptest_tests {
        #![allow(clippy::unwrap_used)]

        use super::*;
        use crate::common::{StubFan, StubHeater};
        use proptest::prelude::*;

        proptest! {
            /// Fase 3 (BUG-CATCH-PLAN.md): hostile but FINITE `desired`
            /// outputs (huge magnitudes, f32::MAX, subnormals, negatives,
            /// ±0.0) must never poison `status.ssr_output`. The NaN boundary
            /// is documented by the S5 reproduction test
            /// (`safety_repro_tests.rs`, currently fails — NaN propagates and
            /// disarms the comms-idle / MAX_ROAST_TIME backstops because
            /// `NaN > 0.0 == false`). Every finite input class must clamp
            /// into [0, 100] and stay finite.
            #[test]
            fn guarded_heater_output_stays_finite_and_clamped(
                desired in prop_oneof![
                    Just(f32::MAX),
                    Just(f32::MIN_POSITIVE),
                    Just(f32::MIN_POSITIVE * 0.5),
                    Just(-0.0),
                    Just(0.0),
                    Just(100.0),
                    -1e30f32..1e30,
                ]
            ) {
                let mut act = ActuatorController::new(
                    Box::new(StubHeater::new()),
                    Box::new(StubFan::new()),
                );
                let mut status = SystemStatus::default();
                let now = Instant::from_millis(1_000);

                let result = act.apply_guarded_heater(desired, now, false, &mut status);
                assert!(result.is_ok(), "finite desired must be accepted, got {result:?}");
                assert!(
                    status.ssr_output.is_finite(),
                    "S5-class: ssr_output must stay finite, got {:?}",
                    status.ssr_output
                );
                assert!(
                    (0.0..=100.0).contains(&status.ssr_output),
                    "ssr_output must clamp into [0,100], got {:?}",
                    status.ssr_output
                );
            }
        }
    }
}
