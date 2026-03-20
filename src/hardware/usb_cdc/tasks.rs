use crate::application::queue_metrics::record_queue_depth;
use crate::application::service_container::ServiceContainer;
use crate::input::multiplexer::CommChannel;
use crate::input::parser::ParseError;
use crate::input::{CommandQueue, QueueError, COMMAND_QUEUE_SIZE};
use crate::log_channel;
use crate::logging::channel::Channel;
use crate::logging::traceability::{
    trace_command_enqueue, trace_queue_dequeue, TracedCommand, TRACE_EVENT_MAX_LEN,
};
use core::cell::RefCell;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex as BlockingMutex;
use embassy_time::Duration;
use embassy_time::Timer;
use heapless::{String, Vec};
use log::debug;

use super::driver::get_usb_cdc_driver;

pub const USB_COMMAND_PIPE_SIZE: usize = 256;

/// Command queue for USB FIFO processing - reject-on-full behavior
static USB_COMMAND_QUEUE: BlockingMutex<
    CriticalSectionRawMutex,
    RefCell<Option<CommandQueue<TracedCommand, COMMAND_QUEUE_SIZE>>>,
> = BlockingMutex::new(RefCell::new(None));

#[cfg(all(test, target_arch = "riscv32"))]
pub fn init_usb_command_queue_for_test() {
    USB_COMMAND_QUEUE.lock(|cell| *cell.borrow_mut() = Some(CommandQueue::new()));
}

#[cfg(all(test, target_arch = "riscv32"))]
pub fn drain_usb_command_queue_for_test() -> Vec<TracedCommand, USB_COMMAND_PIPE_SIZE> {
    let mut drained: Vec<TracedCommand, USB_COMMAND_PIPE_SIZE> = Vec::new();

    USB_COMMAND_QUEUE.lock(|cell| {
        if let Some(queue) = cell.borrow_mut().as_mut() {
            while let Some(cmd) = queue.pop() {
                let _ = ServiceContainer::get_artisan_channel().try_send(cmd);
                let _ = drained.push(cmd);
            }
        }
    });

    drained
}

#[cfg_attr(target_arch = "riscv32", embassy_executor::task)]
pub async fn usb_reader_task() {
    let mut rbuf: [u8; 64] = [0u8; 64];

    USB_COMMAND_QUEUE.lock(|cell| *cell.borrow_mut() = Some(CommandQueue::new()));

    Timer::after(Duration::from_millis(100)).await;

    loop {
        if let Some(usb) = get_usb_cdc_driver() {
            match usb.read_bytes(&mut rbuf).await {
                Ok(len) if len > 0 => {
                    let raw_cmd = core::str::from_utf8(&rbuf[..len]).unwrap_or("[binary]");
                    log_channel!(Channel::Usb, "RX: {}", raw_cmd.trim_end());
                    process_usb_command_data(&rbuf[..len]);
                }
                _ => {}
            }
        }

        Timer::after(Duration::from_millis(10)).await;
    }
}

pub fn process_usb_command_data(data: &[u8]) {
    let mut command = Vec::<u8, 64>::new();

    for &byte in data {
        if byte == 0x0D {
            handle_complete_usb_command(&command);
            return;
        }

        if command.push(byte).is_err() {
            send_usb_parse_error(ParseError::InvalidValue);
            return;
        }
    }
}

/// Test-only version of process_usb_command_data for integration tests
/// Made pub so integration tests can call it
#[cfg(feature = "test")]
pub fn process_usb_command_data_test(data: &[u8]) {
    process_usb_command_data(data);
}

fn handle_complete_usb_command(command: &[u8]) {
    let parse_result = if command.is_empty() {
        Err(ParseError::EmptyCommand)
    } else {
        core::str::from_utf8(command)
            .map_err(|_| ParseError::InvalidValue)
            .and_then(crate::input::parse_artisan_command)
    };

    match parse_result {
        Ok(cmd) => {
            let traced = TracedCommand::new(cmd, CommChannel::Usb);
            let mut depth = 0;
            let mut should_process = true;
            let mut use_channel = false;
            let mut queued = false;

            critical_section::with(|cs| {
                let multiplexer = ServiceContainer::get_multiplexer();
                let mut guard = multiplexer.borrow(cs).borrow_mut();
                if let Some(mux) = guard.as_mut() {
                    should_process = mux.should_process_command(CommChannel::Usb);
                }

                if should_process {
                    USB_COMMAND_QUEUE.lock(|cell| {
                        if let Some(queue) = cell.borrow_mut().as_mut() {
                            match queue.try_push(traced) {
                                Ok(()) => {
                                    depth = queue.len();
                                    queued = true;
                                }
                                Err(QueueError::Full) => {
                                    debug!("USB command queue full, rejecting command");
                                    use_channel = true;
                                }
                            }
                        } else {
                            use_channel = true;
                        }
                    });
                }
            });

            if should_process {
                if queued {
                    trace_command_enqueue(&traced, depth, false);
                } else if use_channel {
                    trace_command_enqueue(&traced, depth, true);
                    let _ = ServiceContainer::get_artisan_channel().try_send(traced);
                }
            }
            record_queue_depth(depth);
        }
        Err(error) => {
            send_usb_parse_error(error);
        }
    }
}

fn send_usb_parse_error(error: ParseError) {
    let mut should_write = true;

    critical_section::with(|cs| {
        let multiplexer = ServiceContainer::get_multiplexer();
        let mut guard = multiplexer.borrow(cs).borrow_mut();
        if let Some(mux) = guard.as_mut() {
            if matches!(mux.get_active_channel(), CommChannel::None) {
                let _ = mux.on_command_received(CommChannel::Usb);
            }
            should_write = mux.should_write_to(CommChannel::Usb);
        }

        if should_write {
            let output_channel = ServiceContainer::get_output_channel();
            let mut message = String::<TRACE_EVENT_MAX_LEN>::new();
            let _ = message.push_str("ERR ");
            let _ = message.push_str(error.code());
            let _ = message.push_str(" ");
            let _ = message.push_str(error.message());
            let _ = output_channel.try_send(message);
        }
    });
}

/// USB Queue processor task - consumes commands from USB_COMMAND_QUEUE and sends to artisan_channel
/// This task bridges the command queue to the control loop, ensuring USB commands are processed
#[cfg_attr(target_arch = "riscv32", embassy_executor::task)]
pub async fn usb_queue_processor_task() {
    Timer::after(Duration::from_millis(50)).await;

    loop {
        let (cmd_opt, queue_depth) = USB_COMMAND_QUEUE.lock(|cell| {
            if let Some(queue) = cell.borrow_mut().as_mut() {
                let cmd = queue.pop();
                let depth = queue.len();
                (cmd, depth)
            } else {
                (None, 0)
            }
        });
        record_queue_depth(queue_depth);

        if let Some(cmd) = cmd_opt {
            trace_queue_dequeue(&cmd, queue_depth);
            let channel = ServiceContainer::get_artisan_channel();
            channel.send(cmd).await;
        }

        // Small delay to yield to other tasks and prevent tight looping
        Timer::after(Duration::from_millis(5)).await;
    }
}
