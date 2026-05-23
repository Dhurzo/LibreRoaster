#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]
#![allow(clippy::expect_used, clippy::type_complexity)]

extern crate std;

use futures::executor::{block_on, ThreadPool};
use futures::future::join_all;
use futures::task::SpawnExt;
use libreroaster::application::queue_metrics::{
    queue_processor_backlog_threshold, queue_processor_metrics_snapshot,
    reset_queue_processor_metrics,
};
use libreroaster::application::service_container::ServiceContainer;
use libreroaster::control::RoasterControl;
use libreroaster::hardware::uart::tasks::{process_command_data, queue_processor_task};
use libreroaster::hardware::usb_cdc::tasks::{process_usb_command_data, usb_queue_processor_task};
use libreroaster::input::ArtisanInput;
use std::thread;
use std::time::Duration as StdDuration;
#[path = "common/mod.rs"]
mod tests_common;
use tests_common::{build_test_control, StubFan, StubHeater};

fn build_control() -> RoasterControl {
    build_test_control(Box::new(StubHeater::new()), Box::new(StubFan::new()))
}

fn init_service_container() {
    let roaster_sync = build_control();
    let roaster_async = build_control();
    let artisan_input = ArtisanInput::new().expect("ArtisanInput should build");

    critical_section::with(|cs| {
        let container = ServiceContainer::get_instance();
        container
            .roaster_sync
            .borrow(cs)
            .borrow_mut()
            .replace(roaster_sync);
        container
            .artisan_input
            .borrow(cs)
            .borrow_mut()
            .replace(artisan_input);
    });

    block_on(async {
        let mut guard = ServiceContainer::get_instance().roaster.lock().await;
        *guard = Some(roaster_async);
    });
}

fn reset_channels() {
    let cmd_channel = ServiceContainer::get_artisan_channel();
    while cmd_channel.try_receive().is_ok() {}

    let output_channel = ServiceContainer::get_output_channel();
    while output_channel.try_receive().is_ok() {}
}

#[test]
fn command_multiplexer_concurrency_test() {
    init_service_container();
    reset_channels();
    reset_queue_processor_metrics();

    let pool = ThreadPool::new().expect("Failed to start executor");
    pool.spawn_ok(queue_processor_task());
    pool.spawn_ok(usb_queue_processor_task());

    let sensor_handles = (0..5)
        .map(|_| {
            pool.spawn_with_handle(async {
                let mut ok = true;
                for _ in 0..20 {
                    if ServiceContainer::roaster_async_sensor_read().await.is_err() {
                        ok = false;
                    }
                    thread::sleep(StdDuration::from_millis(2));
                }
                ok
            })
            .expect("spawn sensor worker")
        })
        .collect::<Vec<_>>();

    let command_handles = (0..5)
        .map(|_| {
            pool.spawn_with_handle(async {
                let commands: [&[u8]; 3] = [b"READ\r", b"OT1 65\r", b"IO3 45\r"];
                for _ in 0..30 {
                    for cmd in &commands {
                        process_command_data(cmd);
                        process_usb_command_data(cmd);
                    }
                    thread::sleep(StdDuration::from_millis(1));
                }
            })
            .expect("spawn command worker")
        })
        .collect::<Vec<_>>();

    let sensor_results = block_on(join_all(sensor_handles));
    assert!(
        sensor_results.into_iter().all(|ok| ok),
        "Sensor reads returned ContainerError"
    );

    block_on(join_all(command_handles));

    for _ in 0..25 {
        thread::sleep(StdDuration::from_millis(5));
        if ServiceContainer::get_artisan_channel()
            .try_receive()
            .is_err()
        {
            break;
        }
    }

    let metrics = queue_processor_metrics_snapshot();
    assert_eq!(
        metrics.backlog_events, 0,
        "Backlog event recorded: {:?}",
        metrics
    );
    assert!(
        metrics.max_depth < queue_processor_backlog_threshold(),
        "Queue depth {} breached threshold {}",
        metrics.max_depth,
        queue_processor_backlog_threshold()
    );
}
