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
//!
//! # Memory Strategy
//!
//! This module is classified as **INITIALIZATION** with some **MIXED** operations:
//! - Policy evaluation typically occurs during command processing (not hot path)
//! - Error message creation uses heapless to prevent allocations in critical paths
//! - Most operations use stack-only primitives
//!
//! ## Memory Usage
//!
//! - `error_message`: `heapless::String<POLICY_MSG_MAX_LEN>` for predictable memory
//! - `reason`: `heapless::String<POLICY_MSG_MAX_LEN>` for safety messages
//! - All other fields: primitives (f32, bool, Option) with no allocations

use crate::config::{RoasterCommand, SsrHardwareStatus, SystemStatus};
use crate::memory::POLICY_MSG_MAX_LEN;
use heapless::String;

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
    pub error_message: Option<String<POLICY_MSG_MAX_LEN>>,
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
            error_message: String::try_from(message).ok(),
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
    pub reason: Option<String<POLICY_MSG_MAX_LEN>>,
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
            reason: String::try_from(reason).ok(),
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::config::SystemStatus;
    use crate::memory::POLICY_MSG_MAX_LEN;

    // ── ManualPolicyOutcome ─────────────────────────────

    #[test]
    fn heater_constructs_with_clamped_value() {
        let o = ManualPolicyOutcome::heater(75.0);
        assert_eq!(o.heater_target, Some(75.0));
        assert!(o.fan_target.is_none());
        assert_eq!(o.pid_enabled, Some(false));
        assert_eq!(o.artisan_control, Some(true));
        assert!(!o.clear_manual);
        assert!(o.success);
        assert!(o.error_message.is_none());
    }

    #[test]
    fn heater_clamps_above_100() {
        let o = ManualPolicyOutcome::heater(150.0);
        assert_eq!(o.heater_target, Some(100.0));
    }

    #[test]
    fn heater_clamps_below_0() {
        let o = ManualPolicyOutcome::heater(-10.0);
        assert_eq!(o.heater_target, Some(0.0));
    }

    #[test]
    fn fan_constructs_with_clamped_value() {
        let o = ManualPolicyOutcome::fan(50.0);
        assert!(o.heater_target.is_none());
        assert_eq!(o.fan_target, Some(50.0));
        assert_eq!(o.pid_enabled, Some(false));
        assert_eq!(o.artisan_control, Some(true));
        assert!(o.success);
    }

    #[test]
    fn fan_clamps_above_100() {
        let o = ManualPolicyOutcome::fan(200.0);
        assert_eq!(o.fan_target, Some(100.0));
    }

    #[test]
    fn fan_clamps_below_0() {
        let o = ManualPolicyOutcome::fan(-5.0);
        assert_eq!(o.fan_target, Some(0.0));
    }

    #[test]
    fn heater_increment_adds_delta() {
        let o = ManualPolicyOutcome::heater_increment(50.0, 10.0);
        assert_eq!(o.heater_target, Some(60.0));
    }

    #[test]
    fn heater_increment_clamps_at_100() {
        let o = ManualPolicyOutcome::heater_increment(95.0, 20.0);
        assert_eq!(o.heater_target, Some(100.0));
    }

    #[test]
    fn heater_decrement_subtracts_delta() {
        let o = ManualPolicyOutcome::heater_decrement(50.0, 10.0);
        assert_eq!(o.heater_target, Some(40.0));
    }

    #[test]
    fn heater_decrement_clamps_at_0() {
        let o = ManualPolicyOutcome::heater_decrement(10.0, 20.0);
        assert_eq!(o.heater_target, Some(0.0));
    }

    #[test]
    fn stopped_clears_all_outputs() {
        let o = ManualPolicyOutcome::stopped();
        assert_eq!(o.heater_target, Some(0.0));
        assert_eq!(o.fan_target, Some(0.0));
        assert_eq!(o.pid_enabled, Some(false));
        assert_eq!(o.artisan_control, Some(false));
        assert!(o.clear_manual);
        assert!(o.success);
    }

    #[test]
    fn failed_returns_error_message() {
        let msg = "test error";
        let o = ManualPolicyOutcome::failed(msg);
        assert!(!o.success);
        assert!(o.heater_target.is_none());
        assert!(o.fan_target.is_none());
        assert_eq!(o.error_message.as_deref(), Some(msg));
    }

    #[test]
    fn failed_message_exceeds_max_len_returns_none() {
        // heapless::String::try_from returns Err when message exceeds capacity,
        // and .ok() converts that to None — fail-closed behavior.
        let long_msg = "x".repeat(POLICY_MSG_MAX_LEN + 50);
        let o = ManualPolicyOutcome::failed(&long_msg);
        assert!(!o.success);
        assert!(o.error_message.is_none());
    }

    #[test]
    fn failed_message_at_max_len_is_preserved() {
        let exact_msg = "x".repeat(POLICY_MSG_MAX_LEN);
        let o = ManualPolicyOutcome::failed(&exact_msg);
        assert!(!o.success);
        assert_eq!(o.error_message.as_deref(), Some(exact_msg.as_str()));
    }

    #[test]
    fn apply_manual_heater_to_status() {
        let mut status = SystemStatus::default();
        let o = ManualPolicyOutcome::heater(80.0);
        o.apply_to_status(&mut status);
        assert_eq!(status.ssr_output, 80.0);
        assert!(!status.pid_enabled);
        assert!(status.artisan_control);
    }

    #[test]
    fn apply_manual_fan_to_status() {
        let mut status = SystemStatus::default();
        let o = ManualPolicyOutcome::fan(60.0);
        o.apply_to_status(&mut status);
        assert_eq!(status.fan_output, 60.0);
        assert!(!status.pid_enabled);
        assert!(status.artisan_control);
    }

    #[test]
    fn apply_manual_stopped_to_status() {
        let mut status = SystemStatus {
            ssr_output: 80.0,
            fan_output: 60.0,
            ..Default::default()
        };
        let o = ManualPolicyOutcome::stopped();
        o.apply_to_status(&mut status);
        assert_eq!(status.ssr_output, 0.0);
        assert_eq!(status.fan_output, 0.0);
        assert!(!status.pid_enabled);
        assert!(!status.artisan_control);
    }

    #[test]
    fn apply_manual_partial_ignores_none_fields() {
        let mut status = SystemStatus {
            pid_enabled: true,
            ..Default::default()
        };
        let o = ManualPolicyOutcome::fan(50.0);
        o.apply_to_status(&mut status);
        assert_eq!(status.ssr_output, 0.0);
        assert_eq!(status.fan_output, 50.0);
    }

    #[test]
    fn manual_policy_default_is_success_false() {
        let o = ManualPolicyOutcome::default();
        assert!(!o.success);
        assert!(o.heater_target.is_none());
        assert!(o.fan_target.is_none());
        assert!(o.error_message.is_none());
    }

    // ── SafetyPolicyOutcome ─────────────────────────────

    #[test]
    fn safety_normal_has_no_emergency() {
        let o = SafetyPolicyOutcome::normal();
        assert!(!o.emergency_active);
        assert!(!o.fault_condition);
        assert!(!o.zero_ssr);
        assert!(!o.disable_pid);
        assert!(o.ssr_hardware_status.is_none());
        assert!(o.reason.is_none());
    }

    #[test]
    fn safety_emergency_sets_all_flags() {
        let o = SafetyPolicyOutcome::emergency("overheat");
        assert!(o.emergency_active);
        assert!(o.fault_condition);
        assert!(o.zero_ssr);
        assert!(o.disable_pid);
        assert_eq!(o.ssr_hardware_status, Some(SsrHardwareStatus::Error));
        assert_eq!(o.reason.as_deref(), Some("overheat"));
    }

    #[test]
    fn safety_emergency_reason_exceeds_max_len_returns_none() {
        let long_reason = "y".repeat(POLICY_MSG_MAX_LEN + 50);
        let o = SafetyPolicyOutcome::emergency(&long_reason);
        assert_eq!(o.reason.as_deref(), None);
    }

    #[test]
    fn safety_emergency_reason_at_max_len_is_preserved() {
        let exact_reason = "y".repeat(POLICY_MSG_MAX_LEN);
        let o = SafetyPolicyOutcome::emergency(&exact_reason);
        assert_eq!(o.reason.as_deref(), Some(exact_reason.as_str()));
    }

    #[test]
    fn apply_safety_normal_does_not_mutate() {
        let mut status = SystemStatus {
            ssr_output: 50.0,
            pid_enabled: true,
            ..Default::default()
        };
        let o = SafetyPolicyOutcome::normal();
        o.apply_to_status(&mut status);
        assert_eq!(status.ssr_output, 50.0);
        assert!(status.pid_enabled);
        assert!(!status.fault_condition);
    }

    #[test]
    fn apply_safety_emergency_zeroes_ssr() {
        let mut status = SystemStatus {
            ssr_output: 80.0,
            pid_enabled: true,
            ssr_hardware_status: SsrHardwareStatus::Available,
            ..Default::default()
        };
        let o = SafetyPolicyOutcome::emergency("test");
        o.apply_to_status(&mut status);
        assert_eq!(status.ssr_output, 0.0);
        assert!(!status.pid_enabled);
        assert!(status.fault_condition);
        assert_eq!(status.ssr_hardware_status, SsrHardwareStatus::Error);
    }

    #[test]
    fn safety_policy_default_has_no_emergency() {
        let o = SafetyPolicyOutcome::default();
        assert!(!o.emergency_active);
        assert!(!o.fault_condition);
        assert!(!o.zero_ssr);
    }
}
