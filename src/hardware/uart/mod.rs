//! UART transport module for LibreRoaster.
//!
//! Provides the ESP32-C3 UART0 driver (split TX/RX halves on host vs. embedded
//! targets), the Embassy reader task, and the multiplexer-aware response/stream
//! helpers used by the control loop to talk to Artisan over the serial port.

#[cfg(target_arch = "riscv32")]
pub mod driver;
#[cfg(not(target_arch = "riscv32"))]
#[path = "driver_host.rs"]
pub mod driver;
pub mod tasks;

// L8: `CircularBuffer` (relaxed-atomics SPSC ring; produced `Ok` even on
// partial-write/silent-loss) was removed — the production path uses
// `embassy_sync::Channel`, not a manual ring. The `pub use driver::*`
// re-exports are still needed (in particular `uart_write_bytes` for the
// transport)'s possible direct callers).
pub use driver::{init_uart, uart_read_bytes, uart_write_bytes, UartError};
// Bug #9 fix: the embedded UART driver is now split into TX and RX halves
// (UartTxDriver / UartRxDriver) so RX and TX can be guarded by independent
// mutexes. The host target keeps the historical single `UartDriver`.
#[cfg(not(target_arch = "riscv32"))]
pub use driver::UartDriver;
#[cfg(target_arch = "riscv32")]
pub use driver::{UartRxDriver, UartTxDriver};
// L8: `uart_writer_task` was removed (never spawned; would race
// `dual_output_task` for the output pipe). Keep the reader + the helper
// re-exports.
pub use tasks::{process_command_data, send_response, send_stream, uart_reader_task};

// Bug L18 (2026-08-10): `COMMAND_PIPE_SIZE` was removed together with the
// never-spawned `run_writer_task` and its pipe.
#[allow(unused_imports)]
pub use crate::hardware::transport_tasks::EVENT_QUEUE_SIZE;

/// UART line rate used for the Artisan serial session (115200 8N1).
pub const UART_BAUD_RATE: u32 = 115200;

#[cfg(target_arch = "riscv32")]
use esp_hal::gpio::interconnect::{PeripheralInput, PeripheralOutput};

/// Bind UART0 and its RX/TX pins and initialize the embedded UART driver.
#[cfg(target_arch = "riscv32")]
pub fn initialize_uart_system(
    uart0: esp_hal::peripherals::UART0<'static>,
    rx: impl PeripheralInput<'static>,
    tx: impl PeripheralOutput<'static>,
) -> Result<(), UartError> {
    init_uart(uart0, rx, tx)
}
