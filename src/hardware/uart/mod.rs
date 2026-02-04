pub mod buffer;
#[cfg(target_arch = "riscv32")]
pub mod driver;
#[cfg(not(target_arch = "riscv32"))]
#[path = "driver_host.rs"]
pub mod driver;
pub mod tasks;

pub use buffer::CircularBuffer;
pub use driver::{get_uart_driver, init_uart, UartDriver, UartError};
pub use tasks::{
    send_response, send_stream, uart_reader_task, uart_writer_task, COMMAND_PIPE_SIZE,
};

pub const UART_BAUD_RATE: u32 = 115200;

#[cfg(target_arch = "riscv32")]
pub fn initialize_uart_system(uart0: esp_hal::peripherals::UART0) -> Result<(), UartError> {
    init_uart(uart0)
}
