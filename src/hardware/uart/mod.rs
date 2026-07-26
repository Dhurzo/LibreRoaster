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

pub use crate::hardware::transport_tasks::{COMMAND_PIPE_SIZE, EVENT_QUEUE_SIZE};

pub const UART_BAUD_RATE: u32 = 115200;

#[cfg(target_arch = "riscv32")]
use esp_hal::gpio::interconnect::{PeripheralInput, PeripheralOutput};

#[cfg(target_arch = "riscv32")]
pub fn initialize_uart_system(
    uart0: esp_hal::peripherals::UART0<'static>,
    rx: impl PeripheralInput<'static>,
    tx: impl PeripheralOutput<'static>,
) -> Result<(), UartError> {
    init_uart(uart0, rx, tx)
}
