//! Concurrent sensor-read mutex stress test.
//!
//! Spawns N concurrent `roaster_async_sensor_read` tasks and asserts the
//! async-lock depth never exceeds 1 (no overlapping holders).
//!
//! The host `critical_section` implementation comes from the
//! `critical-section` crate's `std` feature (dev-dependencies): defining a
//! second one here via `set_impl!` would duplicate the
//! `_critical_section_1_0_acquire`/`_release` symbols and break the link on
//! strict linkers (CI/lld, 2026-10-04). The crate's std implementation
//! serializes entries with a global mutex, which is all this test needs.

#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]

extern crate std;

use futures::executor::{block_on, ThreadPool};
use futures::future::join_all;
use futures::task::SpawnExt;
use libreroaster::application::service_container::{
    async_lock_depth_max_for_tests, reset_async_lock_metrics_for_tests, ContainerError,
    ServiceContainer,
};
#[path = "common/mod.rs"]
mod tests_common;

use libreroaster::control::RoasterControl;
use std::boxed::Box;
use tests_common::{build_test_control, StubFan, StubHeater};

/// Number of concurrent sensor readers (and executor pool size).
const CONCURRENT_READS: usize = 10;

/// Build a stub `RoasterControl` for the sensor-read workers.
fn build_control() -> RoasterControl {
    build_test_control(Box::new(StubHeater::new()), Box::new(StubFan::new()))
}

/// Register a fresh stub roaster in the global container.
fn init_service_container() {
    let roaster = build_control();
    ServiceContainer::init_roaster(roaster);
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
