//! Artisan+ manual control policy for roaster commands.
//!
//! Evaluates SetHeaterManual/SetFanManual/UP/DOWN/SetUnits into a
//! `ManualPolicyOutcome`; handler state commits only after the hardware
//! write succeeds (`commit_manual_heater` / `commit_manual_fan`).

// Artisan manual command handler for roaster control
//
// This module handles Artisan+ manual commands:
// - SetHeaterManual - Direct heater control (0-100%)
// - SetFanManual - Direct fan control (0-100%)
// - IncreaseHeater - Increase heater by 5%
// - DecreaseHeater - Decrease heater by 5%
// - SetUnits - Set temperature scale preference (Celsius/Fahrenheit)
//
// These commands provide manual control over roaster outputs.

use crate::config::{RoasterCommand, SystemStatus, TemperatureScale, TemperatureSettings};
use crate::control::policies::{ManualCommandPolicy, ManualPolicyOutcome};
use log::{info, warn};

/// Artisan+ manual command handler.
///
/// Evaluates heater/fan/units commands into a `ManualPolicyOutcome`, committing
/// the applied manual values only after the hardware write succeeds.
pub struct ArtisanCommandHandler {
    manual_heater: f32,
    manual_fan: f32,
    temp_settings: TemperatureSettings,
}

impl ArtisanCommandHandler {
    /// Create a new Artisan command handler
    pub fn new() -> Self {
        Self {
            manual_heater: 0.0,
            manual_fan: 0.0,
            temp_settings: TemperatureSettings::default(),
        }
    }

    /// Get manual heater value
    pub fn get_manual_heater(&self) -> f32 {
        self.manual_heater
    }

    /// Get manual fan value
    pub fn get_manual_fan(&self) -> f32 {
        self.manual_fan
    }

    /// Commit a manually-set heater value AFTER the hardware write was
    /// accepted (Bug C, 2026-08-03). `evaluate` must NOT mutate
    /// `manual_heater` — a `ssr_cycle_busy` rejection of the write would
    /// otherwise leave the handler state ahead of the mode flags
    /// (`artisan_control`/`pid_enabled` still point at PID control) and the
    /// operator's value would be silently ignored for a tick. Committing here,
    /// post-write, keeps `manual_heater` in lockstep with what the SSR is
    /// actually applying. UP/DOWN base their delta on this committed value,
    /// which is exactly the desired baseline.
    pub fn commit_manual_heater(&mut self, value: f32) {
        self.manual_heater = value;
    }

    /// Commit a manually-set fan value after the hardware write was accepted.
    /// Same rationale as `commit_manual_heater`.
    pub fn commit_manual_fan(&mut self, value: f32) {
        self.manual_fan = value;
    }

    /// Set manual heater and fan values
    pub fn set_manual_values(&mut self, heater: f32, fan: f32) {
        self.manual_heater = heater;
        self.manual_fan = fan;
    }

    /// Clear manual control values
    pub fn clear_manual(&mut self) {
        self.manual_heater = 0.0;
        self.manual_fan = 0.0;
    }

    const HEATER_DELTA: i8 = 5;

    /// Apply heater delta (increase or decrease)
    ///
    /// # Arguments
    ///
    /// * `current_value` - Current heater value
    /// * `direction` - Direction: 1 for increase, -1 for decrease
    ///
    /// # Returns
    ///
    /// New heater value clamped to 0-100 range
    fn apply_heater_delta(current_value: f32, direction: i8) -> f32 {
        // Bug R6 (2026-07-26): the previous `(current_value as i16 + delta as
        // i16)` truncated fractional heater values (e.g. 47.6 → 47) on every
        // UP/DOWN — a small error that accumulates across presses and drifts
        // the displayed value from the applied one. Do the math in f32.
        let delta = (direction as f32) * (Self::HEATER_DELTA as f32);
        (current_value + delta).clamp(0.0, 100.0)
    }
}

impl Default for ArtisanCommandHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ManualCommandPolicy for ArtisanCommandHandler {
    /// Evaluate manual command policy
    ///
    /// # Arguments
    ///
    /// * `command` - Roaster command to evaluate
    /// * `status` - Mutable system status
    ///
    /// # Returns
    ///
    /// Policy outcome with heater, fan, and control state
    fn evaluate(
        &mut self,
        command: RoasterCommand,
        status: &mut SystemStatus,
    ) -> ManualPolicyOutcome {
        match command {
            RoasterCommand::SetHeaterManual(value) => {
                if value > 100 {
                    warn!("Ignoring manual heater value above 100%: {}", value);
                    return ManualPolicyOutcome::failed("Invalid heater value >100%");
                }

                // M10: defer state mutation (`manual_heater`, `pid_enabled`,
                // `artisan_control`) to the actuator's `apply_policy_outcome`,
                // which ONLY commits if `apply_guarded_heater` accepts the
                // write. Pre-fix, the typed policy outcome called
                // `apply_to_status(status)` here (mutating `pid_enabled`/
                // `artisan_control`/`manual_heater` *before* the hardware
                // write), so a `reject_on_busy` `Err(ssr_cycle_busy)` left
                // Artisan with an "ERR" while the software state had already
                // adopted the new value — the next tick applied it blindly.
                // Bug C (2026-08-03): `manual_heater` itself no longer mutates
                // here either — it is committed after the write succeeds via
                // `self.commit_manual_heater(...)` in `apply_policy_outcome`.
                let outcome = ManualPolicyOutcome::heater(value as f32);
                // NB: deliberate no `apply_to_status(status)` here. The
                // state side-effects live in `apply_policy_outcome` after the
                // hardware write succeeds.

                info!("Artisan+ manual heater set to: {}%", value);
                outcome
            }

            RoasterCommand::SetFanManual(value) => {
                if value > 100 {
                    warn!("Ignoring manual fan value above 100%: {}", value);
                    return ManualPolicyOutcome::failed("Invalid fan value >100%");
                }

                // Bug C (2026-08-03): the `self.manual_fan` mutation moved to
                // `commit_manual_fan` in `apply_policy_outcome` (post-write).
                // Audit MA-8 (2026-08-11): the `outcome.apply_to_status(status)`
                // that used to live here (writing `status.fan_output` BEFORE
                // the hardware write) is REMOVED for full parity with the
                // heater branch (M10 discipline). It had two defects:
                //   1. On a fan-write failure (`set_fan_speed` → Err), the
                //      status claimed the new value while the hardware never
                //      received it — the exact "state ahead of hardware" bug
                //      class Bug C fixed for the heater.
                //   2. It wrote the UN-floored value: an `OT2 0` with the
                //      heater energized briefly claimed 0 % before the
                //      `FAN_MIN_SAFETY_PCT` floor in `apply_policy_outcome`
                //      re-clamped it.
                // `status.fan_output` is now published ONLY by
                // `ActuatorController::set_fan_speed` (success) with the
                // floor applied, and `commit_manual_fan` commits the
                // handler-local value post-write — single writer, same as
                // the heater side.
                let outcome = ManualPolicyOutcome::fan(value as f32);

                info!("Artisan+ manual fan set to: {}%", value);
                outcome
            }

            RoasterCommand::IncreaseHeater => {
                // Bug #8: baseline on `self.manual_heater`, not `status.ssr_output`.
                // Bug C: `manual_heater` holds the last COMMITTED value (only
                // updated after the hardware accepts a write), so increases
                // climb from what the SSR is actually applying.
                let current = self.manual_heater;
                let new_value = Self::apply_heater_delta(current, 1);

                let outcome = ManualPolicyOutcome::heater(new_value);
                // M10: no `apply_to_status(status)` for heater; see comment
                // in `SetHeaterManual` and the commit site in
                // `RoasterControl::apply_policy_outcome`.

                info!("Artisan+ UP: heater increased to {:.0}%", new_value);
                outcome
            }

            RoasterCommand::DecreaseHeater => {
                // Bug C: baseline on the last committed `manual_heater`.
                let current = self.manual_heater;
                let new_value = Self::apply_heater_delta(current, -1);

                let outcome = ManualPolicyOutcome::heater(new_value);
                // M10: no `apply_to_status(status)` for heater; see comment
                // in `SetHeaterManual`.

                info!("Artisan+ DOWN: heater decreased to {:.0}%", new_value);
                outcome
            }

            RoasterCommand::SetUnits(is_fahrenheit) => {
                let scale = if is_fahrenheit {
                    TemperatureScale::Fahrenheit
                } else {
                    TemperatureScale::Celsius
                };
                self.temp_settings.set_scale(scale);
                status.temperature_settings.set_scale(scale);
                info!("Artisan+ units set to: {:?}", scale);

                ManualPolicyOutcome {
                    heater_target: None,
                    fan_target: None,
                    pid_enabled: None,
                    artisan_control: None,
                    clear_manual: false,
                    success: true,
                    error_message: None,
                }
            }

            _ => ManualPolicyOutcome::failed("Command not handled by ManualCommandPolicy"),
        }
    }

    /// Check if this handler can process the given command
    fn can_handle(&self, command: RoasterCommand) -> bool {
        matches!(
            command,
            RoasterCommand::SetHeaterManual(_)
                | RoasterCommand::SetFanManual(_)
                | RoasterCommand::IncreaseHeater
                | RoasterCommand::DecreaseHeater
                | RoasterCommand::SetUnits(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let handler = ArtisanCommandHandler::new();
        assert_eq!(handler.get_manual_heater(), 0.0);
        assert_eq!(handler.get_manual_fan(), 0.0);
    }

    #[test]
    fn test_set_manual_values() {
        let mut handler = ArtisanCommandHandler::new();
        handler.set_manual_values(75.0, 50.0);
        assert_eq!(handler.get_manual_heater(), 75.0);
        assert_eq!(handler.get_manual_fan(), 50.0);
    }

    #[test]
    fn test_apply_heater_delta_constant() {
        assert_eq!(ArtisanCommandHandler::HEATER_DELTA, 5);
    }

    #[test]
    fn test_apply_heater_delta_increase() {
        let current = 50.0;
        let result = ArtisanCommandHandler::apply_heater_delta(current, 1);
        assert_eq!(result, 55.0);
    }

    #[test]
    fn test_apply_heater_delta_decrease() {
        let current = 50.0;
        let result = ArtisanCommandHandler::apply_heater_delta(current, -1);
        assert_eq!(result, 45.0);
    }

    #[test]
    fn test_apply_heater_delta_at_max() {
        let current = 98.0;
        let result = ArtisanCommandHandler::apply_heater_delta(current, 1);
        assert_eq!(result, 100.0);
    }

    #[test]
    fn test_apply_heater_delta_at_min() {
        let current = 3.0;
        let result = ArtisanCommandHandler::apply_heater_delta(current, -1);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_clear_manual() {
        let mut handler = ArtisanCommandHandler::new();
        handler.set_manual_values(75.0, 50.0);
        handler.clear_manual();
        assert_eq!(handler.get_manual_heater(), 0.0);
        assert_eq!(handler.get_manual_fan(), 0.0);
    }

    #[test]
    fn test_default() {
        let handler = ArtisanCommandHandler::default();
        assert_eq!(handler.get_manual_heater(), 0.0);
        assert_eq!(handler.get_manual_fan(), 0.0);
    }
}
