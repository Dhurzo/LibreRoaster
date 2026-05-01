use esp_hal::gpio::{Gpio1, Gpio10, Gpio3, Gpio4, Gpio9, Input, Output};
use esp_hal::ledc::{
    channel::{Channel, ChannelIFace},
    timer::Timer,
    LowSpeed,
};
use esp_hal::peripherals::{GPIO1, GPIO10, GPIO3, GPIO4, GPIO9, LEDC, UART0};
use esp_hal::uart::Uart;

use crate::hardware::fan::FanController;
use crate::hardware::ssr::SsrControl;

pub type ConcreteSsr =
    SsrControl<'static, Output<'static>, Input<'static>, Channel<'static, LowSpeed>>;
pub type ConcreteFan = FanController;

pub struct BoardHardware {
    pub ssr: ConcreteSsr,
    pub fan: ConcreteFan,
    pub uart0: UART0,
    pub ledc: LEDC,
    pub gpio9: GPIO9,
}
