// Audit A-TC4 (2026-08-12): added the `test` feature gate — the stage
// instrumentation path pulls the host Embassy time driver, which fails to
// link on a plain `cargo test` without the `test` feature (same failure
// mode documented in CONTEXT.md).
#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]

use libreroaster::application::stage_instrumentation::{
    GuardState, StageName, StageReporter, WatchdogState,
};

#[test]
fn test_stage_reporter_sequence() {
    let reporter = StageReporter::new();
    let mut reports = Vec::new();

    let sequence = [
        (StageName::SensorRead, 5, GuardState::Ok, WatchdogState::Ok),
        (
            StageName::ControlUpdate,
            12,
            GuardState::Ok,
            WatchdogState::Ok,
        ),
        (
            StageName::LedcWrite,
            15,
            GuardState::Timeout,
            WatchdogState::Ok,
        ),
        (
            StageName::WatchdogFeed,
            20,
            GuardState::Ok,
            WatchdogState::Ok,
        ),
        (
            StageName::TelemetryEmit,
            25,
            GuardState::Ok,
            WatchdogState::Ok,
        ),
    ];

    for (stage, elapsed, guard, watchdog) in sequence.iter() {
        if let Some(report) = reporter.report_simple(*stage, *elapsed as u64, *guard, *watchdog) {
            reports.push(report);
        }
    }

    assert_eq!(reports.len(), 5);

    // Verify ordering and format
    assert!(reports[0].contains("STAGE,SensorRead"));
    assert!(reports[0].contains("elapsed=5ms"));
    assert!(reports[1].contains("STAGE,ControlUpdate"));
    assert!(reports[2].contains("guard=1"));
    assert!(reports[3].contains("watchdog=ok"));
    assert!(reports[4].contains("STAGE,TelemetryEmit"));
}

#[test]
fn test_stage_reporter_fault_injection() {
    let reporter = StageReporter::new();

    // Test with failure marker
    let report = reporter
        .report(
            StageName::WatchdogFeed,
            40,
            GuardState::Ok,
            WatchdogState::Fail,
            Some("timeout"),
        )
        .expect("Failed to create report");

    assert!(report.contains("STAGE,WatchdogFeed"));
    assert!(report.contains("watchdog=fail"));
    assert!(report.contains("failure=timeout"));
}
