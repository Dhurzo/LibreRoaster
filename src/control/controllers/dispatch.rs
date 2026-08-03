use crate::config::*;
use crate::control::handlers::{
    ArtisanCommandHandler, SystemCommandHandler, TemperatureCommandHandler,
};
use crate::control::pid::PidFeedback;
use crate::control::policies::{ManualCommandPolicy, ManualPolicyOutcome};
use crate::control::{RoasterCommandHandler, RoasterError};
use embassy_time::Instant;
use log::{info, warn};

pub struct CommandDispatcher {
    pub(crate) temp_handler: TemperatureCommandHandler,
    pub(crate) artisan_handler: ArtisanCommandHandler,
    pub(crate) system_handler: SystemCommandHandler,
}

impl CommandDispatcher {
    pub fn new() -> Result<Self, RoasterError> {
        let temp_handler = TemperatureCommandHandler::new()?;
        Ok(Self {
            temp_handler,
            artisan_handler: ArtisanCommandHandler::new(),
            system_handler: SystemCommandHandler,
        })
    }

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

    pub fn can_handle_manual(&self, command: RoasterCommand) -> bool {
        <ArtisanCommandHandler as ManualCommandPolicy>::can_handle(&self.artisan_handler, command)
    }

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
        // Ensure telemetry starts emitting when PID is enabled.
        // Without this, SETTARGET/SETTARGET+START leaves the system
        // in a state where pid_enabled=true but is_continuous_enabled()=false,
        // causing is_streaming() to return true (blocking START) while
        // emit_telemetry_stage() produces no output.
        self.get_output_manager_mut().enable_continuous_output();
        info!("PID control re-enabled with target: {:.1}°C", target_temp);
        Ok(())
    }

    pub fn disable_pid(&mut self) {
        self.temp_handler.disable_pid();
    }

    pub fn get_pid_output(&mut self, bean_temp: f32, current_time: Instant) -> f32 {
        self.temp_handler.get_pid_output(bean_temp, current_time)
    }

    pub fn set_pid_feedback(&mut self, feedback: PidFeedback) {
        self.temp_handler.set_pid_feedback(feedback);
    }

    pub fn set_pid_target(&mut self, target: f32) -> Result<(), RoasterError> {
        self.temp_handler.set_pid_target(target)
    }

    pub fn get_output_manager(&self) -> &crate::control::OutputController {
        self.temp_handler.get_output_manager()
    }

    pub fn get_output_manager_mut(&mut self) -> &mut crate::control::OutputController {
        self.temp_handler.get_output_manager_mut()
    }

    pub fn is_streaming(&self, status: &SystemStatus) -> bool {
        self.temp_handler
            .get_output_manager()
            .is_continuous_enabled()
            || status.pid_enabled
            || status.artisan_control
    }

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

    pub fn artisan_manual_heater(&self) -> f32 {
        self.artisan_handler.get_manual_heater()
    }

    pub fn artisan_manual_fan(&self) -> f32 {
        self.artisan_handler.get_manual_fan()
    }

    pub fn clear_artisan_manual(&mut self) {
        self.artisan_handler.clear_manual();
    }

    pub fn pid_integrator_value(&self) -> f32 {
        self.temp_handler.pid_integrator_value()
    }

    pub fn pid_saturation_active(&self) -> bool {
        self.temp_handler.pid_saturation_active()
    }

    pub fn pid_integrator_clamped(&self) -> bool {
        self.temp_handler.pid_integrator_clamped()
    }

    pub fn set_pid_gains(&mut self, kp: f32, ki: f32, kd: f32) -> Result<(), RoasterError> {
        self.temp_handler.set_pid_gains(kp, ki, kd)
    }

    pub fn set_pid_cycle_time(&mut self, ms: u32) {
        self.temp_handler.set_pid_cycle_time(ms);
    }

    pub fn set_pid_output_limits(&mut self, min: f32, max: f32) {
        self.temp_handler.set_pid_output_limits(min, max);
    }

    /// Returns the PID output limits currently in effect (clamped to [0,100]
    /// and swapped to ensure min <= max), not the raw inputs.
    pub fn pid_output_limits(&self) -> (f32, f32) {
        self.temp_handler.pid_output_limits()
    }
}

pub enum CommandDispatchResult {
    StopStreaming,
    Handled(Result<(), RoasterError>),
}
