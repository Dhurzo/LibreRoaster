//! Command dispatch sub-controller for roaster control.
//!
//! Routes parsed `RoasterCommand`s to the temperature/system handlers,
//! evaluates Artisan manual policies with commit-after-write discipline,
//! and fronts PID management (gains, target, feedback) for the loop.

use crate::config::*;
use crate::control::handlers::{
    ArtisanCommandHandler, SystemCommandHandler, TemperatureCommandHandler,
};
use crate::control::pid::PidFeedback;
use crate::control::policies::{ManualCommandPolicy, ManualPolicyOutcome};
use crate::control::{RoasterCommandHandler, RoasterError};
use embassy_time::Instant;
use log::{info, warn};

/// Command routing facade over the temperature, Artisan, and system handlers.
pub struct CommandDispatcher {
    pub(crate) temp_handler: TemperatureCommandHandler,
    pub(crate) artisan_handler: ArtisanCommandHandler,
    pub(crate) system_handler: SystemCommandHandler,
}

impl CommandDispatcher {
    /// Build the dispatcher and its three sub-handlers.
    pub fn new() -> Result<Self, RoasterError> {
        let temp_handler = TemperatureCommandHandler::new()?;
        Ok(Self {
            temp_handler,
            artisan_handler: ArtisanCommandHandler::new(),
            system_handler: SystemCommandHandler,
        })
    }

    /// Route a command to the first handler that accepts it.
    ///
    /// `StopRoast` short-circuits to `StopStreaming`; unmatched commands error.
    pub fn process_command(
        &mut self,
        command: RoasterCommand,
        current_time: Instant,
        status: &mut SystemStatus,
    ) -> CommandDispatchResult {
        if matches!(command, RoasterCommand::StopRoast) {
            return CommandDispatchResult::StopStreaming;
        }

        let mut handlers: [&mut dyn RoasterCommandHandler; 2] =
            [&mut self.temp_handler, &mut self.system_handler];

        for handler in &mut handlers {
            if handler.can_handle(command) {
                let result = handler.handle_command(command, current_time, status);
                return CommandDispatchResult::Handled(result);
            }
        }

        warn!("No handler found for command: {:?}", command);
        CommandDispatchResult::Handled(Err(RoasterError::InvalidState {
            source: Some("no_handler_found"),
        }))
    }

    /// Whether the Artisan manual policy accepts this command.
    pub fn can_handle_manual(&self, command: RoasterCommand) -> bool {
        <ArtisanCommandHandler as ManualCommandPolicy>::can_handle(&self.artisan_handler, command)
    }

    /// Evaluate a manual command without committing state (commit-after-write).
    pub fn evaluate_manual_policy(
        &mut self,
        command: RoasterCommand,
        status: &mut SystemStatus,
    ) -> ManualPolicyOutcome {
        self.artisan_handler.evaluate(command, status)
    }

    /// Commit a manual heater value AFTER the hardware write was accepted
    /// (Bug C, 2026-08-03). See `ArtisanCommandHandler::commit_manual_heater`.
    pub fn commit_manual_heater(&mut self, value: f32) {
        self.artisan_handler.commit_manual_heater(value);
    }

    /// Commit a manual fan value AFTER the hardware write was accepted
    /// (Bug C, 2026-08-03). See `ArtisanCommandHandler::commit_manual_fan`.
    pub fn commit_manual_fan(&mut self, value: f32) {
        self.artisan_handler.commit_manual_fan(value);
    }

    /// Arm PID control with a target; only the first call enables the controller.
    ///
    /// Repeated calls (Artisan ramp/soak re-sends SV) update the target only.
    pub fn enable_pid(
        &mut self,
        target_temp: f32,
        status: &mut SystemStatus,
    ) -> Result<(), RoasterError> {
        status.artisan_control = false;
        self.temp_handler.set_pid_target(target_temp)?;
        // Bug A3 (2026-07-25): Artisan's ramp/soak profile re-sends SV on
        // every step. Calling `enable_pid` unconditionally would call
        // `PidController::enable` every time, which clears the I-term and
        // derivative history → steady-state droop on every ramp step. Only
        // arm the controller the first time we transition out of manual
        // mode; subsequent calls just update the target.
        if !self.temp_handler.pid_is_enabled() {
            self.temp_handler.enable_pid();
        }
        status.pid_enabled = true;
        status.target_temp = target_temp;
        // BUG-08 (2026-08-21): enabling the PID no longer enables the
        // continuous telemetry stream — it is opt-in via `STREAM;ON`. The
        // stale comment about `is_streaming()` blocking START was obsoleted
        // by the state-based START gate (bug V2-4), and the `#DUMP` ring
        // feed is driven by the roast logger state, not this flag.
        info!("PID control re-enabled with target: {:.1}°C", target_temp);
        Ok(())
    }

    /// Disable PID control.
    pub fn disable_pid(&mut self) {
        self.temp_handler.disable_pid();
    }

    /// Compute the PID output for the given bean temperature.
    pub fn get_pid_output(&mut self, bean_temp: f32, current_time: Instant) -> f32 {
        self.temp_handler.get_pid_output(bean_temp, current_time)
    }

    /// Push actuator/LEDC guard feedback into the PID integrator.
    pub fn set_pid_feedback(&mut self, feedback: PidFeedback) {
        self.temp_handler.set_pid_feedback(feedback);
    }

    /// Update the PID target temperature (°C).
    pub fn set_pid_target(&mut self, target: f32) -> Result<(), RoasterError> {
        self.temp_handler.set_pid_target(target)
    }

    /// Output manager (continuous-output state machine).
    pub fn get_output_manager(&self) -> &crate::control::OutputController {
        self.temp_handler.get_output_manager()
    }

    /// Mutable access to the output manager.
    pub fn get_output_manager_mut(&mut self) -> &mut crate::control::OutputController {
        self.temp_handler.get_output_manager_mut()
    }

    /// True if continuous output is enabled or control (PID/manual) is active.
    pub fn is_streaming(&self, status: &SystemStatus) -> bool {
        self.temp_handler
            .get_output_manager()
            .is_continuous_enabled()
            || status.pid_enabled
            || status.artisan_control
    }

    /// Stop streaming: disable continuous output, PID, and manual state.
    ///
    /// Zeros SSR output; the caller sets fan speed separately.
    pub fn stop_streaming(&mut self, status: &mut SystemStatus) {
        self.temp_handler
            .get_output_manager_mut()
            .disable_continuous_output();
        self.temp_handler.disable_pid();
        status.pid_enabled = false;
        status.artisan_control = false;
        self.artisan_handler.clear_manual();
        status.ssr_output = 0.0;
        // fan_output is set by caller (roaster_control.rs stop_streaming) after set_fan_raw
        status.ssr_cycle_guard_busy_until_ms = 0;
    }

    /// Last committed manual heater value (%).
    pub fn artisan_manual_heater(&self) -> f32 {
        self.artisan_handler.get_manual_heater()
    }

    /// Last committed manual fan value (%).
    pub fn artisan_manual_fan(&self) -> f32 {
        self.artisan_handler.get_manual_fan()
    }

    /// Clear the committed Artisan manual values.
    pub fn clear_artisan_manual(&mut self) {
        self.artisan_handler.clear_manual();
    }

    /// Current PID integrator value.
    pub fn pid_integrator_value(&self) -> f32 {
        self.temp_handler.pid_integrator_value()
    }

    /// Whether the PID output is currently saturated.
    pub fn pid_saturation_active(&self) -> bool {
        self.temp_handler.pid_saturation_active()
    }

    /// Whether the PID integrator is clamped.
    pub fn pid_integrator_clamped(&self) -> bool {
        self.temp_handler.pid_integrator_clamped()
    }

    /// Set PID gains (rejects negative values).
    pub fn set_pid_gains(&mut self, kp: f32, ki: f32, kd: f32) -> Result<(), RoasterError> {
        self.temp_handler.set_pid_gains(kp, ki, kd)
    }

    /// Set the PID cycle time in milliseconds.
    pub fn set_pid_cycle_time(&mut self, ms: u32) {
        self.temp_handler.set_pid_cycle_time(ms);
    }

    /// Set the raw PID output limits (min/max %).
    pub fn set_pid_output_limits(&mut self, min: f32, max: f32) {
        self.temp_handler.set_pid_output_limits(min, max);
    }

    /// Returns the PID output limits currently in effect (clamped to [0,100]
    /// and swapped to ensure min <= max), not the raw inputs.
    pub fn pid_output_limits(&self) -> (f32, f32) {
        self.temp_handler.pid_output_limits()
    }
}

/// Outcome of dispatching one command.
pub enum CommandDispatchResult {
    /// `StopRoast` received: stop streaming before any handler runs.
    StopStreaming,
    /// A handler processed the command (may be an error).
    Handled(Result<(), RoasterError>),
}
