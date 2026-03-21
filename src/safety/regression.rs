#[cfg(all(target_arch = "riscv32", feature = "regression"))]
mod target_impl {
    use crate::application::service_container::{ContainerError, ServiceContainer};
    use crate::config::{ArtisanCommand, SystemStatus, WATCHDOG_FEED_INTERVAL_MS};
    use crate::hardware::sensors::conversion::{FixtureReading, SensorConversionHub, SensorSample};
    use crate::logging::traceability::TRACE_EVENT_MAX_LEN;
    use crate::memory::SAFETY_ERROR_MSG_MAX_LEN;
    use crate::output::artisan::ArtisanFormatter;
    use embassy_executor::{task, Spawner};
    use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
    use embassy_sync::channel::Channel;
    use embassy_time::{Duration, Instant, Timer};
    use heapless::String;
    use log::{info, warn};

    #[allow(unsafe_code)]
    mod fixture_catalog {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/max31856_sequences.rs"
        ));
    }

    static REGRESSION_TRIGGER: Channel<CriticalSectionRawMutex, (), 1> = Channel::new();

    pub fn request_regression() {
        let _ = REGRESSION_TRIGGER.sender().try_send(());
    }

    #[task]
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
        pub fn new() -> Self {
            Self
        }

        pub async fn run_once(&mut self) {
            info!("Over-temp regression requested");

            let _ = ServiceContainer::with_roaster_async(|roaster| {
                roaster.mark_overtemp_regression_active(true);
                if let Err(err) = roaster.process_artisan_command(ArtisanCommand::SetHeater(100)) {
                    warn!("Regression heater ramp failed: {:?}", err);
                }
                if let Err(err) = roaster.process_artisan_command(ArtisanCommand::SetFan(100)) {
                    warn!("Regression fan ramp failed: {:?}", err);
                }
                if let Err(err) = roaster.emergency_shutdown("Over-temp regression") {
                    warn!("Regression shutdown failed: {:?}", err);
                }
                Ok::<(), ContainerError>(())
            })
            .await;

            self.keep_feeding_watchdog(Duration::from_millis(500)).await;

            for fixture in fixture_catalog::canonical_fixtures() {
                self.replay_fixture(fixture).await;
            }

            let mut safety = String::<TRACE_EVENT_MAX_LEN>::new();
            let _ = safety.push_str("SAFETY OT-REGRESSION");
            let _ = ServiceContainer::get_output_channel().try_send(safety);

            self.keep_feeding_watchdog(Duration::from_millis(500)).await;

            let _ = ServiceContainer::with_roaster_async(|roaster| {
                roaster.mark_overtemp_regression_active(false);
                Ok::<(), ContainerError>(())
            })
            .await;

            Timer::after(Duration::from_millis(250)).await;
        }

        async fn replay_fixture(&mut self, fixture: &'static fixture_catalog::RegressionFixture) {
            info!("Regression fixture replay: {}", fixture.name);

            let mut hub = SensorConversionHub::new();
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

            self.keep_feeding_watchdog(Duration::from_millis(WATCHDOG_FEED_INTERVAL_MS as u64))
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
            match ArtisanFormatter::format_status_response(status) {
                Ok(line) => {
                    if line != fixture.expected_status_line {
                        warn!(
                            "Fixture {} status mismatch. expected {} got {}",
                            fixture.name, fixture.expected_status_line, line
                        );
                    }

                    if let Ok(mut buffer) = heapless::String::<
                        crate::memory::SAFETY_ERROR_MSG_MAX_LEN,
                    >::try_from(line.as_str())
                    {
                        let _ = ServiceContainer::get_output_channel().try_send(buffer);
                    }
                }
                Err(err) => {
                    warn!(
                        "Fixture {} failed to format status response: {:?}",
                        fixture.name, err
                    );
                }
            }
        }

        async fn keep_feeding_watchdog(&mut self, duration: Duration) {
            let start = Instant::now();

            while Instant::now().duration_since(start) < duration {
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
pub use target_impl::{regression_task, request_regression};

#[cfg(not(all(target_arch = "riscv32", feature = "regression")))]
pub fn request_regression() {}

#[cfg(not(all(target_arch = "riscv32", feature = "regression")))]
#[embassy_executor::task]
pub async fn regression_task() {
    // Stub for non-regression builds - does nothing
    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_secs(3600)).await;
    }
}
