//! Policy contracts for the handler-to-hardware seam.
//!
//! This module defines the ports-and-policies pattern where handlers return
//! policy outcomes describing desired heater/fan targets and status flags
//! without directly writing to hardware. RoasterControl acts as the single
//! writer that consumes these outcomes and applies them via hardware ports.
//!
//! The separation ensures:
//! - Hardware authority stays centralized in RoasterControl
//! - Policy logic can be refactored without touching hardware code
//! - Testing can verify policy decisions independently of hardware

use crate::config::{RoasterCommand, SsrHardwareStatus, SystemStatus};

/// Outcome of a manual command policy evaluation.
///
/// Describes the desired heater/fan adjustments and status flags that
/// RoasterControl should apply through hardware ports. This struct
/// carries the "intent" of a command without performing hardware writes.
#[derive(Debug, Clone, Default)]
pub struct ManualPolicyOutcome {
    /// Desired heater output (0-100%), None if unchanged
    pub heater_target: Option<f32>,
    /// Desired fan speed (0-100%), None if unchanged
    pub fan_target: Option<f32>,
    /// Whether PID control should be enabled
    pub pid_enabled: Option<bool>,
    /// Whether artisan manual control mode is active
    pub artisan_control: Option<bool>,
    /// Whether to clear manual values (e.g., on STOP)
    pub clear_manual: bool,
    /// Policy evaluation was successful
    pub success: bool,
    /// Optional error message if policy evaluation failed
    pub error_message: Option<alloc::string::String>,
}

impl ManualPolicyOutcome {
    /// Create a successful outcome with heater target
    pub fn heater(target: f32) -> Self {
        Self {
            heater_target: Some(target.clamp(0.0, 100.0)),
            fan_target: None,
            pid_enabled: Some(false),
            artisan_control: Some(true),
            clear_manual: false,
            success: true,
            error_message: None,
        }
    }

    /// Create a successful outcome with fan target
    pub fn fan(target: f32) -> Self {
        Self {
            heater_target: None,
            fan_target: Some(target.clamp(0.0, 100.0)),
            pid_enabled: Some(false),
            artisan_control: Some(true),
            clear_manual: false,
            success: true,
            error_message: None,
        }
    }

    /// Create a successful outcome for heater increment
    pub fn heater_increment(current: f32, delta: f32) -> Self {
        let new_value = (current + delta).clamp(0.0, 100.0);
        Self::heater(new_value)
    }

    /// Create a successful outcome for heater decrement
    pub fn heater_decrement(current: f32, delta: f32) -> Self {
        let new_value = (current - delta).clamp(0.0, 100.0);
        Self::heater(new_value)
    }

    /// Create a successful outcome for stopping (clearing manual values)
    pub fn stopped() -> Self {
        Self {
            heater_target: Some(0.0),
            fan_target: Some(0.0),
            pid_enabled: Some(false),
            artisan_control: Some(false),
            clear_manual: true,
            success: true,
            error_message: None,
        }
    }

    /// Create a failed outcome with error message
    pub fn failed(message: &str) -> Self {
        Self {
            heater_target: None,
            fan_target: None,
            pid_enabled: None,
            artisan_control: None,
            clear_manual: false,
            success: false,
            error_message: Some(alloc::string::String::from(message)),
        }
    }

    /// Apply this policy outcome to SystemStatus
    ///
    /// Note: This mutates status but does NOT write to hardware.
    /// Hardware writes are performed by RoasterControl after policy evaluation.
    pub fn apply_to_status(&self, status: &mut SystemStatus) {
        if let Some(heater) = self.heater_target {
            status.ssr_output = heater;
        }
        if let Some(fan) = self.fan_target {
            status.fan_output = fan;
        }
        if let Some(pid) = self.pid_enabled {
            status.pid_enabled = pid;
        }
        if let Some(artisan) = self.artisan_control {
            status.artisan_control = artisan;
        }
        if self.clear_manual {
            status.ssr_output = 0.0;
            status.fan_output = 0.0;
        }
    }
}

/// Safety policy outcome describing emergency or safety responses.
#[derive(Debug, Clone, Default)]
pub struct SafetyPolicyOutcome {
    /// Emergency shutdown triggered
    pub emergency_active: bool,
    /// Fault condition flag
    pub fault_condition: bool,
    /// SSR output should be zeroed
    pub zero_ssr: bool,
    /// PID should be disabled
    pub disable_pid: bool,
    /// Hardware status to set
    pub ssr_hardware_status: Option<SsrHardwareStatus>,
    /// Reason for emergency (if any)
    pub reason: Option<alloc::string::String>,
}

impl SafetyPolicyOutcome {
    /// Create a normal (non-emergency) outcome
    pub fn normal() -> Self {
        Self {
            emergency_active: false,
            fault_condition: false,
            zero_ssr: false,
            disable_pid: false,
            ssr_hardware_status: None,
            reason: None,
        }
    }

    /// Create an emergency shutdown outcome
    pub fn emergency(reason: &str) -> Self {
        Self {
            emergency_active: true,
            fault_condition: true,
            zero_ssr: true,
            disable_pid: true,
            ssr_hardware_status: Some(SsrHardwareStatus::Error),
            reason: Some(alloc::string::String::from(reason)),
        }
    }

    /// Apply this safety outcome to SystemStatus
    ///
    /// Note: This mutates status but does NOT write to hardware.
    pub fn apply_to_status(&self, status: &mut SystemStatus) {
        if self.zero_ssr {
            status.ssr_output = 0.0;
        }
        if self.disable_pid {
            status.pid_enabled = false;
        }
        status.fault_condition = self.fault_condition;
        if let Some(ssr_status) = self.ssr_hardware_status {
            status.ssr_hardware_status = ssr_status;
        }
    }
}

/// Policy trait for handling manual Artisan commands.
///
/// Implementers evaluate commands and return policy outcomes describing
/// desired hardware state without performing hardware writes. The single
/// writer (RoasterControl) consumes these outcomes and applies them.
pub trait ManualCommandPolicy: Send {
    /// Evaluate a manual command and return the policy outcome.
    ///
    /// The implementation should NOT write to hardware ports directly.
    /// Instead, it returns a ManualPolicyOutcome that describes the desired
    /// state. RoasterControl is responsible for applying the outcome via
    /// hardware ports.
    fn evaluate(
        &mut self,
        command: RoasterCommand,
        status: &mut SystemStatus,
    ) -> ManualPolicyOutcome;

    /// Check if this policy can handle the given command.
    fn can_handle(&self, command: RoasterCommand) -> bool;
}

/// Policy trait for handling safety commands.
///
/// Implementers evaluate safety-related commands and return safety outcomes
/// that describe the required safety response without touching hardware.
pub trait SafetyPolicy: Send {
    /// Evaluate a safety command and return the safety outcome.
    fn evaluate(
        &mut self,
        command: RoasterCommand,
        status: &mut SystemStatus,
    ) -> SafetyPolicyOutcome;

    /// Check if this policy can handle the given command.
    fn can_handle(&self, command: RoasterCommand) -> bool;

    /// Check if emergency is currently active.
    fn is_emergency_active(&self) -> bool;

    /// Clear the emergency flag.
    fn clear_emergency(&mut self);
}
