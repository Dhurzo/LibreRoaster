pub mod buffer;
#[cfg(target_arch = "riscv32")]
pub mod driver;
#[cfg(not(target_arch = "riscv32"))]
#[path = "driver_host.rs"]
pub mod driver;
pub mod tasks;

pub use buffer::CircularBuffer;
pub use driver::{init_uart, uart_read_bytes, uart_write_bytes, UartError};
// Bug #9 fix: the embedded UART driver is now split into TX and RX halves
// (UartTxDriver / UartRxDriver) so RX and TX can be guarded by independent
// mutexes. The host target keeps the historical single `UartDriver`.
#[cfg(not(target_arch = "riscv32"))]
pub use driver::UartDriver;
#[cfg(target_arch = "riscv32")]
pub use driver::{UartRxDriver, UartTxDriver};
pub use tasks::{
    process_command_data, send_response, send_stream, uart_reader_task, uart_writer_task,
    COMMAND_PIPE_SIZE,
};

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
