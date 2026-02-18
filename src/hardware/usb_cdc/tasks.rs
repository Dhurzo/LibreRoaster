use crate::application::service_container::ServiceContainer;
use crate::input::multiplexer::CommChannel;
use crate::input::parser::ParseError;
use crate::input::{CommandQueue, QueueError, COMMAND_QUEUE_SIZE};
use crate::log_channel;
use crate::logging::channel::Channel;
use embassy_time::Duration;
use embassy_time::Timer;
use heapless::{String, Vec};
use log::warn;
use log::debug;

use super::driver::{get_usb_cdc_driver, UsbCdcError};

pub const USB_COMMAND_PIPE_SIZE: usize = 256;

/// Back-pressure configuration for USB writer
const BACK_PRESSURE_INITIAL_DELAY_MS: u64 = 1;
const BACK_PRESSURE_MAX_DELAY_MS: u64 = 10;
const BACK_PRESSURE_LOG_THRESHOLD_MS: u64 = 100;

/// Command queue for USB FIFO processing - reject-on-full behavior
static mut USB_COMMAND_QUEUE: Option<CommandQueue<crate::config::ArtisanCommand, COMMAND_QUEUE_SIZE>> = None;

#[cfg_attr(target_arch = "riscv32", embassy_executor::task)]
pub async fn usb_reader_task() {
    let mut rbuf: [u8; 64] = [0u8; 64];

    // Initialize the USB command queue for FIFO processing
    critical_section::with(|_| unsafe {
        USB_COMMAND_QUEUE = Some(CommandQueue::new());
    });

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

#[cfg_attr(target_arch = "riscv32", embassy_executor::task)]
pub async fn usb_writer_task() {
    let output_channel = ServiceContainer::get_output_channel();
    let mut back_pressure_start: Option<u64> = None;
    let mut current_delay = BACK_PRESSURE_INITIAL_DELAY_MS;

    loop {
        if let Ok(data) = output_channel.try_receive() {
            if let Some(usb) = get_usb_cdc_driver() {
                let bytes = data.as_bytes().to_vec();
                log_channel!(Channel::Usb, "TX: {}", data);

                let write_result = usb.write_bytes(&bytes).await;

                match write_result {
                    Ok(()) => {
                        // Write successful - reset back-pressure state
                        back_pressure_start = None;
                        current_delay = BACK_PRESSURE_INITIAL_DELAY_MS;
                    }
                    Err(UsbCdcError::WouldBlock) => {
                        // Back-pressure detected - yield and retry with backoff
                        back_pressure_start = Some(
                            back_pressure_start.unwrap_or_else(|| {
                                // First time back-pressure detected
                                embassy_time::Instant::now().as_ticks()
                            })
                        );

                        // Log warning if prolonged back-pressure
                        let now = embassy_time::Instant::now().as_ticks();
                        if let Some(start) = back_pressure_start {
                            let duration_ms = now.saturating_sub(start);
                            if duration_ms > BACK_PRESSURE_LOG_THRESHOLD_MS {
                                warn!(
                                    "USB CDC back-pressure: {}ms congestion detected",
                                    duration_ms
                                );
                            }
                        }

                        // Yield with exponential backoff
                        Timer::after(Duration::from_millis(current_delay)).await;

                        // Increase delay for next retry (up to max)
                        current_delay = (current_delay * 2).min(BACK_PRESSURE_MAX_DELAY_MS);

                        // Put data back to output channel for retry (try_send doesn't block)
                        let _ = output_channel.try_send(data);
                    }
                    Err(e) => {
                        // Other error - log and continue
                        warn!("USB CDC write error: {:?}", e);
                        back_pressure_start = None;
                        current_delay = BACK_PRESSURE_INITIAL_DELAY_MS;
                    }
                }
            }
        } else {
            // No data to send - reset back-pressure state
            back_pressure_start = None;
            current_delay = BACK_PRESSURE_INITIAL_DELAY_MS;
        }

        // Brief yield to allow other tasks to run
        Timer::after(Duration::from_millis(1)).await;
    }
}

#[cfg(feature = "test")]
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

#[cfg(not(feature = "test"))]
pub(crate) fn process_usb_command_data(data: &[u8]) {
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
            critical_section::with(|cs| {
                let multiplexer = ServiceContainer::get_multiplexer();
                let mut guard = multiplexer.borrow(cs).borrow_mut();
                if let Some(mux) = guard.as_mut() {
                    let should_process = mux.should_process_command(CommChannel::Usb);

                    if should_process {
                        // Push to USB command queue for FIFO processing
                        // On queue full: silently drop command (no response sent - Artisan times out)
                        if let Some(queue) = unsafe { USB_COMMAND_QUEUE.as_mut() } {
                            match queue.try_push(cmd) {
                                Ok(()) => {
                                    // Command queued successfully - will be processed by queue processor
                                }
                                Err(QueueError::Full) => {
                                    // Queue full - reject silently (no response sent)
                                    debug!("USB command queue full, rejecting command");
                                }
                            }
                        }
                    }
                }
            });
        }
        Err(error) => {
            send_usb_parse_error(error);
        }
    }
}

fn send_usb_parse_error(error: ParseError) {
    critical_section::with(|cs| {
        let multiplexer = ServiceContainer::get_multiplexer();
        let mut guard = multiplexer.borrow(cs).borrow_mut();
        if let Some(mux) = guard.as_mut() {
            let should_write = mux.should_write_to(CommChannel::Usb);

            if should_write {
                let output_channel = ServiceContainer::get_output_channel();
                let mut message = String::<128>::new();
                let _ = message.push_str("ERR ");
                let _ = message.push_str(error.code());
                let _ = message.push_str(" ");
                let _ = message.push_str(error.message());
                let _ = output_channel.try_send(message);
            }
        }
    });
}

/// USB Queue processor task - consumes commands from USB_COMMAND_QUEUE and sends to artisan_channel
/// This task bridges the command queue to the control loop, ensuring USB commands are processed
#[cfg_attr(target_arch = "riscv32", embassy_executor::task)]
pub async fn usb_queue_processor_task() {
    // Small delay to allow other tasks to initialize
    Timer::after(Duration::from_millis(50)).await;

    loop {
        // Try to pop a command from the USB queue and send to artisan_channel
        let cmd_opt = critical_section::with(|_| {
            unsafe {
                USB_COMMAND_QUEUE.as_mut().and_then(|queue| queue.pop())
            }
        });

        if let Some(cmd) = cmd_opt {
            let channel = ServiceContainer::get_artisan_channel();
            if let Err(err) = channel.try_send(cmd) {
                // Log but don't block - command will be reprocessed by Artisan timeout
                debug!("USB queue processor: failed to send to artisan_channel: {:?}", err);
            }
        }

        // Small delay to yield to other tasks and prevent tight looping
        Timer::after(Duration::from_millis(5)).await;
    }
}
