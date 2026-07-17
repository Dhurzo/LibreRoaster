#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]
#![allow(clippy::expect_used, clippy::type_complexity)]

extern crate std;

use futures::executor::{block_on, ThreadPool};
use futures::future::join_all;
use futures::task::SpawnExt;
use libreroaster::application::service_container::ServiceContainer;
use libreroaster::control::RoasterControl;
use libreroaster::hardware::uart::tasks::process_command_data;
use libreroaster::hardware::usb_cdc::tasks::process_usb_command_data;
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
    let roaster = build_control();
    let artisan_input = ArtisanInput::new().expect("ArtisanInput should build");
    ServiceContainer::init_roaster(roaster);
    ServiceContainer::init_artisan_input(artisan_input);
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

    let pool = ThreadPool::new().expect("Failed to start executor");

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

    // Drain all commands from artisan channel
    let artisan_channel = ServiceContainer::get_artisan_channel();
    for _ in 0..25 {
        thread::sleep(StdDuration::from_millis(5));
        if artisan_channel.try_receive().is_err() {
            break;
        }
    }

    // Verify artisan channel is drained (no backlog in direct-push model)
    assert!(
        artisan_channel.try_receive().is_err(),
        "Artisan channel should be empty after drain"
    );
}
