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

// Definición de tipos concretos para el hardware de esta placa específica
pub type ConcreteSsr =
    SsrControl<'static, Output<'static>, Input<'static>, Channel<'static, LowSpeed>>;
// El FanController ya es una struct concreta o wrapper
pub type ConcreteFan = FanController;

pub struct BoardHardware {
    pub ssr: ConcreteSsr,
    pub fan: ConcreteFan,
    // Otros periféricos que se pasan al AppBuilder
    pub uart0: UART0,
    pub ledc: LEDC,
    pub gpio9: GPIO9,
}

// Nota: Debido a cómo esp-hal maneja los tipos y ownership,
// mover la inicialización aquí requiere mover también los tipos exactos de los pines.
// Para no complicar excesivamente los generics ahora mismo,
// vamos a dejar que el main haga el 'setup' sucio y nos pase los objetos construidos.
