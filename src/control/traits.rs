use crate::config::constants::SsrHardwareStatus;
use crate::control::RoasterError;

/// Trait para leer temperatura (Termómetro)
pub trait Thermometer: Send {
    /// Lee la temperatura en grados Celsius
    fn read_temperature(&mut self) -> Result<f32, RoasterError>;
}

/// Trait para controlar un elemento calefactor (SSR)
pub trait Heater: Send {
    /// Establece el nivel de potencia (0.0 a 100.0)
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError>;

    /// Obtiene el estado físico del hardware
    fn get_status(&self) -> SsrHardwareStatus;
}

/// Trait para controlar un ventilador
pub trait Fan: Send {
    /// Establece la velocidad (0.0 a 100.0)
    fn set_speed(&mut self, duty: f32) -> Result<(), RoasterError>;
}

// Blanket implementation for mutable references
impl<T: Heater + ?Sized> Heater for &mut T {
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError> {
        (**self).set_power(duty)
    }

    fn get_status(&self) -> SsrHardwareStatus {
        (**self).get_status()
    }
}

impl<T: Fan + ?Sized> Fan for &mut T {
    fn set_speed(&mut self, duty: f32) -> Result<(), RoasterError> {
        (**self).set_speed(duty)
    }
}

impl<T: Thermometer + ?Sized> Thermometer for &mut T {
    fn read_temperature(&mut self) -> Result<f32, RoasterError> {
        (**self).read_temperature()
    }
}
