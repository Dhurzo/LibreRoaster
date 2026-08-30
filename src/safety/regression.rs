//! Over-temperature regression runner.
//!
//! On `riscv32` + the `regression` feature, `regression_task` waits for a trigger
//! and runs one over-temp self-test pass (heater/fan ramp, forced emergency
//! shutdown, fixture replay, watchdog feeding, then P9 recovery). The fixture
//! catalogue is intentionally empty until HIL-validated fixtures exist; the run
//! then emits an explicit `SAFETY OT-REGRESSION-EMPTY` marker. All other builds
//! get no-op stubs of the same entry points.

#[cfg(all(target_arch = "riscv32", feature = "regression"))]
mod target_impl {
    use crate::application::service_container::{ContainerError, ServiceContainer};
    use crate::config::{ArtisanCommand, SystemStatus, WATCHDOG_FEED_INTERVAL_MS};
    use crate::hardware::sensors::conversion::{SensorConversionHub, SensorSample};
    use crate::logging::traceability::TRACE_EVENT_MAX_LEN;
    use crate::output::artisan::ArtisanFormatter;
    use embassy_executor::{task, Spawner};
    use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
    use embassy_sync::channel::Channel;
    use embassy_time::{Duration, Instant, Timer};
    use heapless::String;
    use log::{info, warn};

    // Bug B15: the previous design `include!`-ed `tests/fixtures/max31856_sequences.rs`,
    // a file that was deleted in `b6d7173` ("code clean up v2"). The deleted file also
    // pulled in `embedded_hal_mock` (a host test-only crate) and a `bean_transactions` /
    // `env_transactions` field set the regression runner never reads — both are
    // incompatible with the embedded (`--features embedded,regression`) build this module
    // is gated to. Restoring that file would have re-broken the build it claims to fix.
    // Instead we declare the fixture surface inline with only the fields the runner
    // touches (`name`, `reading`, `status_builder`, `expected_status_line`), and
    // `canonical_fixtures()` returns an empty slice. The empty catalogue makes the
    // module type-check cleanly across host and embedded; the run-time replay loop
    // simply iterates zero fixtures. A future HIL-validated fixture set can be added
    // here without touching the runner — and without pulling host test deps into the
    // embedded build.
    mod fixture_catalog {
        use crate::config::SystemStatus;
        use crate::hardware::sensors::conversion::FixtureReading;

        /// Builds the expected `SystemStatus` for a fixture.
        pub type StatusBuilder = fn() -> SystemStatus;

        /// A single over-temperature regression fixture (name + expected line).
        pub struct RegressionFixture {
            pub name: &'static str,
            pub reading: FixtureReading,
            pub status_builder: StatusBuilder,
            pub expected_status_line: &'static str,
        }

        /// Returns the catalogue of regression fixtures (empty until a HIL set is added).
        pub fn canonical_fixtures() -> &'static [RegressionFixture] {
            &[]
        }
    }

    static REGRESSION_TRIGGER: Channel<CriticalSectionRawMutex, (), 1> = Channel::new();

    /// Signal the regression task to run an over-temperature self-test.
    pub fn request_regression() {
        let _ = REGRESSION_TRIGGER.sender().try_send(());
    }

    #[task]
    /// Embassy task that waits for a trigger and runs the over-temp regression.
    pub async fn regression_task() {
        let receiver = REGRESSION_TRIGGER.receiver();
        loop {
            receiver.receive().await;
            let spawner = unsafe { Spawner::for_current_executor().await };
            run_overtemp_regression(&spawner).await;
        }
    }

    async fn run_overtemp_regression(_spawner: &Spawner) {
        let mut runner = OverTempTestRunner::new();
        runner.run_once().await;
    }

    struct OverTempTestRunner;

    impl OverTempTestRunner {
        /// Create a fresh over-temperature test runner.
        pub fn new() -> Self {
            Self
        }

        /// Execute one over-temperature regression pass (ramp, shutdown, replay).
        pub async fn run_once(&mut self) {
            info!("Over-temp regression requested");

            // Bug #3: if `emergency_shutdown` fails (heater/writer/etc. rejected
            // the off command), the heater and fan were just set to 100% and
            // would remain there unattended. We must NOT continue the
            // regression in that state. Capture the shutdown result and, on
            // failure, emit a SAFETY error and abort before replaying any
            // fixtures or feeding the watchdog again.
            let shutdown_failed = ServiceContainer::with_roaster_async(|roaster| {
                roaster.mark_overtemp_regression_active(true);
                if let Err(err) = roaster.process_artisan_command(ArtisanCommand::SetHeater(100)) {
                    warn!("Regression heater ramp failed: {:?}", err);
                }
                if let Err(err) = roaster.process_artisan_command(ArtisanCommand::SetFan(100)) {
                    warn!("Regression fan ramp failed: {:?}", err);
                }
                let shutdown_result = roaster.emergency_shutdown("Over-temp regression");
                // Bug NEW-1 (2026-07-26): `emergency_shutdown` returns
                // `Err(EmergencyShutdown)` BY DESIGN (actuator.rs — it is the
                // "emergency armed" signal, not a failure). The V2-6
                // `is_err()` capture therefore made `failed` ALWAYS true, so
                // the runner aborted (SAFETY OT-REGRESSION-ABORTED
                // shutdown_failed) before replaying any fixture — the whole
                // regression feature was dead code. The real failure indicator
                // is the SSR hardware status: after the actuator's retries,
                // `Error` means the heater did NOT shut off.
                if let Err(ref err) = shutdown_result {
                    warn!("Regression shutdown returned: {:?}", err);
                }
                roaster.get_status().ssr_hardware_status
                    == crate::config::constants::SsrHardwareStatus::Error
            })
            .await;

            let shutdown_failed = shutdown_failed.unwrap_or(true);

            if shutdown_failed {
                let mut safety = String::<TRACE_EVENT_MAX_LEN>::new();
                let _ = safety.push_str("SAFETY OT-REGRESSION-ABORTED shutdown_failed");
                crate::hardware::error_counters::try_send_output(
                    ServiceContainer::get_output_channel(),
                    safety,
                );

                // Still clear the regression flag so the device does not
                // advertise a regression that did not actually run.
                // Bug P9 (2026-08-03): the emergency latch must NOT be cleared
                // on this path. `shutdown_failed` means the regression could
                // not turn the heater OFF (SSR stuck in `Error` after the
                // retries) — a real hardware fault, not a test artifact.
                // Keeping the latch (heater output forced to 0, fan pinned at
                // 100 %, commands rejected) is the correct end state until the
                // operator intervenes physically; clearing it would drop the
                // forced cooldown fan exactly when the heater may be wedged on.
                // The SUCCESS path below is the P9 recovery (restore operator
                // control after a clean self-test).
                let _ = ServiceContainer::with_roaster_async(|roaster| {
                    roaster.mark_overtemp_regression_active(false);
                    Ok::<(), ContainerError>(())
                })
                .await;
                return;
            }

            // Bug #4: feed the watchdog for 400 ms. The software watchdog
            // timeout is 1000 ms (`WATCHDOG_TIMEOUT_MS` in watchdog.rs — Bug
            // audit 2026-08-02 raised it from 500 ms to cover three real
            // ~330 ms ticks), so 400 ms leaves a comfortable margin for
            // scheduler jitter. Bug L17 (2026-08-10): the previous comment
            // still quoted the stale 500 ms timeout.
            self.keep_feeding_watchdog(Duration::from_millis(400)).await;

            for fixture in fixture_catalog::canonical_fixtures() {
                self.replay_fixture(fixture).await;
            }

            // Bug V2-6: do not announce a regression pass with an empty
            // catalogue — that advertishes a regression that tested nothing.
            // An empty gate is left in deliberately for a future HIL-validated
            // fixture set; until then we emit an explicit EMPTY marker so the
            // host cannot mistake `SAFETY OT-REGRESSION` for a green run.
            let mut safety = String::<TRACE_EVENT_MAX_LEN>::new();
            if fixture_catalog::canonical_fixtures().is_empty() {
                let _ = safety.push_str("SAFETY OT-REGRESSION-EMPTY no_fixtures");
            } else {
                let _ = safety.push_str("SAFETY OT-REGRESSION");
            }
            crate::hardware::error_counters::try_send_output(
                ServiceContainer::get_output_channel(),
                safety,
            );

            self.keep_feeding_watchdog(Duration::from_millis(400)).await;

            // Bug P9 (2026-08-03): restore operator control at the end of the
            // run. `emergency_shutdown("Over-temp regression")` above armed
            // the safety latch (`activate_emergency` + `fault_condition` +
            // state=Error); the old body only cleared the regression flag, so
            // the roaster stayed in `Error` — every START/OT1/PREHEAT rejected
            // until an `OFF` or a power cycle. `clear_emergency_explicit` is
            // the single sanctioned un-latch path (same as the START
            // recovery): it returns the device to a recoverable `Idle`.
            let _ = ServiceContainer::with_roaster_async(|roaster| {
                roaster.mark_overtemp_regression_active(false);
                roaster.clear_emergency_explicit();
                Ok::<(), ContainerError>(())
            })
            .await;

            Timer::after(Duration::from_millis(250)).await;
        }

        async fn replay_fixture(&mut self, fixture: &'static fixture_catalog::RegressionFixture) {
            info!("Regression fixture replay: {}", fixture.name);

            // Bug V2-6: the previous `SensorConversionHub::new()` (no args)
            // only exists on the host build; on `riscv32 + !simulated-sensors`
            // it is E0061 (two-arg variant). `from_fixture` exists exactly
            // for this path and internally uses `new_uninit()` — which the
            // `regression` feature now makes select the safe
            // `simulated-sensors` branch (see Cargo.toml feature gating).
            let mut hub = match SensorConversionHub::from_fixture(fixture.reading) {
                Ok(hub) => hub,
                Err(err) => {
                    warn!("Fixture {} failed to build hub: {:?}", fixture.name, err);
                    return;
                }
            };
            let sample = match hub.sample_from_fixture(fixture.reading) {
                Ok(sample) => sample,
                Err(err) => {
                    warn!(
                        "Fixture {} failed to derive hub sample: {:?}",
                        fixture.name, err
                    );
                    return;
                }
            };

            let status = self.build_status_from_sample(sample, fixture);
            self.emit_status_line(&status, fixture);

            self.keep_feeding_watchdog(Duration::from_millis(WATCHDOG_FEED_INTERVAL_MS))
                .await;
        }

        fn build_status_from_sample(
            &self,
            sample: SensorSample,
            fixture: &'static fixture_catalog::RegressionFixture,
        ) -> SystemStatus {
            let mut status = (fixture.status_builder)();

            // Tie bean/env, PV, and other instrumentation columns directly to the SensorConversionHub output
            // so regression log snapshots use the same STATUS tail formatting as the live loop.
            status.bean_temp = sample.bean_temp;
            status.env_temp = sample.env_temp;
            status.pv = sample.bean_temp;

            status.watchdog_feed_ok = true;
            status.watchdog_last_failure = None;
            status.watchdog_consecutive_failures = 0;

            if sample.bean_fault.has_fault() || sample.env_fault.has_fault() {
                status.fault_condition = true;
            }
            status.overtemp_regression_active = true;

            status
        }

        fn emit_status_line(
            &self,
            status: &SystemStatus,
            fixture: &'static fixture_catalog::RegressionFixture,
        ) {
            let line = ArtisanFormatter::format_status_response(status);
            if line != fixture.expected_status_line {
                warn!(
                    "Fixture {} status mismatch. expected {} got {}",
                    fixture.name, fixture.expected_status_line, line
                );
            }

            if let Ok(buffer) = heapless::String::<
                { crate::logging::traceability::TRACE_EVENT_MAX_LEN },
            >::try_from(line.as_str())
            {
                crate::hardware::error_counters::try_send_output(
                    ServiceContainer::get_output_channel(),
                    buffer,
                );
            }
        }

        async fn keep_feeding_watchdog(&mut self, duration: Duration) {
            let start = Instant::now();

            while Instant::now().saturating_duration_since(start) < duration {
                if let Err(err) = ServiceContainer::get_instance()
                    .with_watchdog(|watchdog| watchdog.feed_async(0.0))
                {
                    warn!("Regression watchdog feed failed: {:?}", err);
                }

                Timer::after(Duration::from_millis(WATCHDOG_FEED_INTERVAL_MS)).await;
            }
        }
    }
}

#[cfg(all(target_arch = "riscv32", feature = "regression"))]
/// Re-export of the embedded over-temperature regression entry points.
pub use target_impl::{regression_task, request_regression};

#[cfg(not(all(target_arch = "riscv32", feature = "regression")))]
/// No-op regression request on non-regression builds.
pub fn request_regression() {}

#[cfg(not(all(target_arch = "riscv32", feature = "regression")))]
#[embassy_executor::task]
/// Idle stub regression task on non-regression builds.
pub async fn regression_task() {
    // Stub for non-regression builds - does nothing
    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_secs(3600)).await;
    }
}
