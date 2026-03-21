#![cfg(all(test, target_arch = "riscv32"))]

extern crate std;

use std::vec::Vec;

use critical_section;
use libreroaster::application::service_container::ServiceContainer;
use libreroaster::config::ArtisanCommand;
use libreroaster::hardware::usb_cdc::tasks::{
    drain_usb_command_queue_for_test, init_usb_command_queue_for_test,
    process_usb_command_data_test,
};
use libreroaster::input::multiplexer::CommChannel;

fn reset_channels() {
    let artisan_channel = ServiceContainer::get_artisan_channel();
    while artisan_channel.try_receive().is_ok() {}

    let output_channel = ServiceContainer::get_output_channel();
    while output_channel.try_receive().is_ok() {}
}

fn collect_artisan_commands() -> Vec<ArtisanCommand> {
    let channel = ServiceContainer::get_artisan_channel();
    let mut commands = Vec::new();

    while let Ok(command) = channel.try_receive() {
        commands.push(command.command);
    }

    commands
}

fn enable_usb_channel() {
    critical_section::with(|cs| {
        let multiplexer = ServiceContainer::get_multiplexer();
        let mut guard = multiplexer.borrow(cs).borrow_mut();
        let mux = guard
            .as_mut()
            .expect("USB multiplexer must be initialized for instrumentation");

        assert!(
            mux.should_process_command(CommChannel::Usb),
            "USB instrumentation should activate the USB channel"
        );
    });
}

#[test]
fn usb_instrumentation_runner_exercises_process_helper() {
    ServiceContainer::init_multiplexer();
    reset_channels();
    init_usb_command_queue_for_test();
    enable_usb_channel();

    process_usb_command_data_test(b"READ\r");

    let drained = drain_usb_command_queue_for_test();
    assert_eq!(
        drained.len(),
        1,
        "Instrumentation queue run should emit exactly one command"
    );
    assert!(
        matches!(drained[0], ArtisanCommand::ReadStatus),
        "READ instrumentation should parse into ReadStatus"
    );

    let artisan_commands = collect_artisan_commands();
    assert_eq!(
        artisan_commands.len(),
        1,
        "Artisan channel receives command"
    );
    assert!(
        matches!(artisan_commands[0], ArtisanCommand::ReadStatus),
        "Artisan channel read should observe ReadStatus"
    );

    let output_channel = ServiceContainer::get_output_channel();
    while let Ok(line) = output_channel.try_receive() {
        assert!(
            line.as_str().starts_with("TRACE,"),
            "Unexpected non-TRACE output: {}",
            line.as_str()
        );
    }
}
