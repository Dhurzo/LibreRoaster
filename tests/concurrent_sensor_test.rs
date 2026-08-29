//! Concurrent sensor-read mutex stress test.
//!
//! Installs a host `critical_section` impl that serializes a `RefCell` borrow,
//! then spawns N concurrent `roaster_async_sensor_read` tasks and asserts the
//! async-lock depth never exceeds 1 (no overlapping holders).

#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]

extern crate std;

use critical_section::RawRestoreState;
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
use std::hint::spin_loop;
use std::sync::atomic::{AtomicBool, Ordering};
use tests_common::{build_test_control, StubFan, StubHeater};

// Atomic flag serializes host critical section entries so the RefCell borrow never overlaps.
static TEST_CRITICAL_SECTION_LOCK: AtomicBool = AtomicBool::new(false);

critical_section::set_impl!(TestCriticalSection);

struct TestCriticalSection;

unsafe impl critical_section::Impl for TestCriticalSection {
    unsafe fn acquire() -> RawRestoreState {
        while TEST_CRITICAL_SECTION_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            spin_loop();
        }

        true
    }

    unsafe fn release(_restore_state: RawRestoreState) {
        TEST_CRITICAL_SECTION_LOCK.store(false, Ordering::Release);
    }
}

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
