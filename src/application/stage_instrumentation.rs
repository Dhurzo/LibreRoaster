//! Stage instrumentation reporter for the 100ms control loop.
//!
//! This module provides a deterministic reporter that serializes stage names,
//! elapsed time, guard state, and watchdog state into heapless strings for
//! the Artisan output channel.
//!
//! The reporter produces deterministic `STAGE,<name>,elapsed=<ms>,guard=<flag>,watchdog=<state>`
//! entries for every control loop stage, enabling observers to verify the
//! SensorRead->ControlUpdate->LedcWrite->WatchdogFeed->TelemetryEmit sequence
//! even under fault conditions.

use crate::logging::traceability::TRACE_EVENT_MAX_LEN;
use core::fmt::Write;
use heapless::String;

/// Maximum length for stage reporter output strings.
/// Sufficient for: "STAGE,TelemetryEmit,elapsed=98ms,guard=0,watchdog=ok,failure=timeout"
pub const STAGE_REPORT_MAX_LEN: usize = TRACE_EVENT_MAX_LEN;

/// Control loop stages that the reporter tracks.
/// These must match the stages in tasks.rs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageName {
    Idle,
    SensorRead,
    ControlUpdate,
    LedcWrite,
    WatchdogFeed,
    TelemetryEmit,
}

impl StageName {
    /// Convert stage enum to static string slice for serialization.
    pub fn as_str(&self) -> &'static str {
        match self {
            StageName::Idle => "Idle",
            StageName::SensorRead => "SensorRead",
            StageName::ControlUpdate => "ControlUpdate",
            StageName::LedcWrite => "LedcWrite",
            StageName::WatchdogFeed => "WatchdogFeed",
            StageName::TelemetryEmit => "TelemetryEmit",
        }
    }
}

/// Guard state representation for the reporter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardState {
    /// No guard timeout occurred.
    Ok,
    /// Guard timeout occurred.
    Timeout,
}

impl GuardState {
    pub fn as_flag(&self) -> u8 {
        match self {
            GuardState::Ok => 0,
            GuardState::Timeout => 1,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            GuardState::Ok => "ok",
            GuardState::Timeout => "timeout",
        }
    }
}

/// Watchdog state representation for the reporter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchdogState {
    /// Watchdog feed succeeded.
    Ok,
    /// Watchdog feed failed.
    Fail,
    /// No feed attempted yet.
    None,
}

impl WatchdogState {
    pub fn as_str(&self) -> &'static str {
        match self {
            WatchdogState::Ok => "ok",
            WatchdogState::Fail => "fail",
            WatchdogState::None => "none",
        }
    }
}

/// Stage instrumentation reporter.
///
/// Serializes stage events into deterministic strings for the Artisan output channel.
/// Uses heapless::String to avoid heap allocation in the hot path.
#[derive(Clone, Debug, Default)]
pub struct StageReporter {
    _private: (), // Prevent construction outside of new()
}

impl StageReporter {
    /// Create a new StageReporter instance.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Report a stage transition with timing and state information.
    ///
    /// Format: `STAGE,<name>,elapsed=<ms>,guard=<flag>,watchdog=<state>`
    ///
    /// # Arguments
    /// * `stage` - The stage that was entered
    /// * `elapsed_ms` - Milliseconds elapsed since tick start
    /// * `guard_state` - Current guard timeout state
    /// * `watchdog_state` - Current watchdog feed state
    /// * `failure_marker` - Optional failure marker for fault conditions
    ///
    /// # Returns
    /// A heapless String containing the formatted report, or None if formatting failed.
    pub fn report(
        &self,
        stage: StageName,
        elapsed_ms: u64,
        guard_state: GuardState,
        watchdog_state: WatchdogState,
        failure_marker: Option<&'static str>,
    ) -> Option<String<STAGE_REPORT_MAX_LEN>> {
        let mut output = String::<STAGE_REPORT_MAX_LEN>::new();

        // Use write! macro for deterministic formatting without heap allocation
        if write!(
            output,
            "STAGE,{},elapsed={}ms,guard={},watchdog={}",
            stage.as_str(),
            elapsed_ms,
            guard_state.as_flag(),
            watchdog_state.as_str()
        )
        .is_err()
        {
            return None;
        }

        // Add failure marker if present (for fault injection scenarios)
        if let Some(marker) = failure_marker {
            if write!(output, ",failure={}", marker).is_err() {
                return None;
            }
        }

        Some(output)
    }

    /// Report a stage transition without failure marker.
    ///
    /// Convenience method for the common case.
    pub fn report_simple(
        &self,
        stage: StageName,
        elapsed_ms: u64,
        guard_state: GuardState,
        watchdog_state: WatchdogState,
    ) -> Option<String<STAGE_REPORT_MAX_LEN>> {
        self.report(stage, elapsed_ms, guard_state, watchdog_state, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stage_reporter_new() {
        let _reporter = StageReporter::new();
        // Just verify it can be created and cloned
        let _clone = _reporter.clone();
    }

    #[test]
    fn test_stage_name_strings() {
        assert_eq!(StageName::Idle.as_str(), "Idle");
        assert_eq!(StageName::SensorRead.as_str(), "SensorRead");
        assert_eq!(StageName::ControlUpdate.as_str(), "ControlUpdate");
        assert_eq!(StageName::LedcWrite.as_str(), "LedcWrite");
        assert_eq!(StageName::WatchdogFeed.as_str(), "WatchdogFeed");
        assert_eq!(StageName::TelemetryEmit.as_str(), "TelemetryEmit");
    }

    #[test]
    fn test_guard_state() {
        assert_eq!(GuardState::Ok.as_flag(), 0);
        assert_eq!(GuardState::Timeout.as_flag(), 1);
        assert_eq!(GuardState::Ok.as_str(), "ok");
        assert_eq!(GuardState::Timeout.as_str(), "timeout");
    }

    #[test]
    fn test_watchdog_state() {
        assert_eq!(WatchdogState::Ok.as_str(), "ok");
        assert_eq!(WatchdogState::Fail.as_str(), "fail");
        assert_eq!(WatchdogState::None.as_str(), "none");
    }

    #[test]
    fn test_report_simple() {
        let reporter = StageReporter::new();

        let output =
            reporter.report_simple(StageName::SensorRead, 5, GuardState::Ok, WatchdogState::Ok);

        assert!(output.is_some());
        let s = output.unwrap();
        assert!(s.contains("STAGE,SensorRead"));
        assert!(s.contains("elapsed=5ms"));
        assert!(s.contains("guard=0"));
        assert!(s.contains("watchdog=ok"));
    }

    #[test]
    fn test_report_with_guard_timeout() {
        let reporter = StageReporter::new();

        let output = reporter.report_simple(
            StageName::LedcWrite,
            25,
            GuardState::Timeout,
            WatchdogState::Ok,
        );

        assert!(output.is_some());
        let s = output.unwrap();
        assert!(s.contains("guard=1"));
    }

    #[test]
    fn test_report_with_watchdog_fail() {
        let reporter = StageReporter::new();

        let output = reporter.report_simple(
            StageName::WatchdogFeed,
            45,
            GuardState::Ok,
            WatchdogState::Fail,
        );

        assert!(output.is_some());
        let s = output.unwrap();
        assert!(s.contains("watchdog=fail"));
    }

    #[test]
    fn test_report_with_failure_marker() {
        let reporter = StageReporter::new();

        let output = reporter.report(
            StageName::SensorRead,
            10,
            GuardState::Ok,
            WatchdogState::Ok,
            Some("sensor_error"),
        );

        assert!(output.is_some());
        let s = output.unwrap();
        assert!(s.contains("failure=sensor_error"));
    }

    #[test]
    fn test_report_deterministic() {
        let reporter = StageReporter::new();

        // Same inputs should produce identical output
        let output1 = reporter.report_simple(
            StageName::ControlUpdate,
            30,
            GuardState::Ok,
            WatchdogState::Ok,
        );

        let output2 = reporter.report_simple(
            StageName::ControlUpdate,
            30,
            GuardState::Ok,
            WatchdogState::Ok,
        );

        assert_eq!(output1, output2);
    }

    #[test]
    fn test_report_all_stages() {
        let reporter = StageReporter::new();
        let stages = [
            StageName::Idle,
            StageName::SensorRead,
            StageName::ControlUpdate,
            StageName::LedcWrite,
            StageName::WatchdogFeed,
            StageName::TelemetryEmit,
        ];

        for (i, stage) in stages.iter().enumerate() {
            let output =
                reporter.report_simple(*stage, i as u64, GuardState::Ok, WatchdogState::Ok);
            assert!(output.is_some(), "Failed to report stage {:?}", stage);
            assert!(
                output.unwrap().contains(stage.as_str()),
                "Output missing stage name"
            );
        }
    }
}
