#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use critical_section;
use critical_section::RawRestoreState;
use futures::executor::{block_on, ThreadPool};
use futures::future::join_all;
use futures::task::SpawnExt;
use libreroaster::application::service_container::{
    async_lock_depth_max_for_tests, reset_async_lock_metrics_for_tests, ContainerError,
    ServiceContainer,
};
use libreroaster::config::constants::SsrHardwareStatus;
use libreroaster::control::traits::{Fan, Heater};
use libreroaster::control::RoasterControl;
use libreroaster::control::RoasterError;
use std::boxed::Box;

critical_section::set_impl!(TestCriticalSection);

struct TestCriticalSection;

unsafe impl critical_section::Impl for TestCriticalSection {
    unsafe fn acquire() -> RawRestoreState {
        false
    }

    unsafe fn release(_restore_state: RawRestoreState) {}
}

#[derive(Default)]
struct StubHeater {
    power: f32,
    status: SsrHardwareStatus,
}

impl Heater for StubHeater {
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError> {
        self.power = duty;
        Ok(())
    }

    fn get_status(&self) -> SsrHardwareStatus {
        self.status
    }
}

#[derive(Default)]
struct StubFan {
    speed: f32,
}

impl Fan for StubFan {
    fn set_speed(&mut self, duty: f32) -> Result<(), RoasterError> {
        self.speed = duty;
        Ok(())
    }
}

const CONCURRENT_READS: usize = 10;

fn build_control() -> RoasterControl {
    RoasterControl::new(
        Box::new(StubHeater {
            power: 0.0,
            status: SsrHardwareStatus::Available,
        }),
        Box::new(StubFan { speed: 0.0 }),
    )
    .expect("RoasterControl should initialize successfully")
}

fn init_service_container() {
    let async_roaster = build_control();
    block_on(async {
        let mut guard = ServiceContainer::get_instance().roaster.lock().await;
        guard.replace(async_roaster);
    });

    let sync_roaster = build_control();
    critical_section::with(|cs| {
        ServiceContainer::get_instance()
            .roaster_sync
            .borrow(cs)
            .borrow_mut()
            .replace(sync_roaster);
    });
}

#[test]
fn concurrent_sensor_reads_verify_async_mutex() {
    init_service_container();

    reset_async_lock_metrics_for_tests();

    let pool = ThreadPool::builder()
        .pool_size(CONCURRENT_READS)
        .create()
        .expect("failed to build executor pool");
    let handles = (0..CONCURRENT_READS)
        .map(|_| {
            pool.spawn_with_handle(async { ServiceContainer::roaster_async_sensor_read().await })
                .expect("failed to spawn concurrent sensor read")
        })
        .collect::<Vec<_>>();

    let results = block_on(async { join_all(handles).await });

    for result in results {
        let read_result: Result<(), ContainerError> = result;
        read_result.expect("Expected concurrent sensor read to succeed");
    }

    let max_depth = async_lock_depth_max_for_tests();
    assert!(
        max_depth <= 1,
        "Async lock depth recorded {} concurrent holders",
        max_depth
    );

    reset_async_lock_metrics_for_tests();
    assert_eq!(
        async_lock_depth_max_for_tests(),
        0,
        "Async lock metrics should reset before the next run"
    );
}
