#[cfg(target_arch = "riscv32")]
mod target_impl {
    use crate::application::service_container::{ContainerError, ServiceContainer};
    use crate::config::{ArtisanCommand, WATCHDOG_FEED_INTERVAL_MS};
    use embassy_executor::{task, Spawner};
    use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
    use embassy_sync::channel::Channel;
    use embassy_time::{Duration, Instant, Timer};
    use heapless::String;
    use log::{info, warn};

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

            let mut safety = String::<128>::new();
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

#[cfg(target_arch = "riscv32")]
pub use target_impl::{regression_task, request_regression};

#[cfg(not(target_arch = "riscv32"))]
pub fn request_regression() {}

#[cfg(not(target_arch = "riscv32"))]
pub async fn regression_task() {}
