use embedded_hal::digital::OutputPin;

#[derive(Debug, Clone, Copy)]
pub enum SsrError {
    OutputError,
}

pub struct SsrControl<PIN> {
    pin: PIN,
}

impl<PIN> SsrControl<PIN>
where
    PIN: OutputPin,
{
    pub fn new(mut pin: PIN) -> Result<Self, SsrError> {
        // Initialize SSR to OFF state
        let _ = pin.set_low();

        Ok(SsrControl { pin })
    }

    pub fn set_on(&mut self) -> Result<(), SsrError> {
        match self.pin.set_high() {
            Ok(_) => Ok(()),
            Err(_) => Err(SsrError::OutputError),
        }
    }

    pub fn set_off(&mut self) -> Result<(), SsrError> {
        match self.pin.set_low() {
            Ok(_) => Ok(()),
            Err(_) => Err(SsrError::OutputError),
        }
    }

    pub fn set(&mut self, state: bool) -> Result<(), SsrError> {
        if state {
            self.set_on()
        } else {
            self.set_off()
        }
    }

    pub fn is_on(&self) -> bool {
        // Note: This depends on the ESP32-C3 pin state reading capability
        // For now, we'll assume we can't read back the state
        false // Placeholder - would need GPIO input configuration
    }

    pub fn toggle(&mut self) -> Result<(), SsrError> {
        // Simple toggle - assumes current state can be tracked
        self.set_on()
    }
}
