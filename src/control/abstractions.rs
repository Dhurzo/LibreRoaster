#[derive(Debug, Clone, PartialEq)]
pub enum RoasterError {
    TemperatureOutOfRange,
    SensorFault,
    InvalidState,
    PidError,
    HardwareError,
    EmergencyShutdown,
}

impl core::fmt::Display for RoasterError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RoasterError::TemperatureOutOfRange => write!(f, "Temperature out of range"),
            RoasterError::SensorFault => write!(f, "Sensor fault"),
            RoasterError::InvalidState => write!(f, "Invalid state"),
            RoasterError::PidError => write!(f, "PID error"),
            RoasterError::HardwareError => write!(f, "Hardware error"),
            RoasterError::EmergencyShutdown => write!(f, "Emergency shutdown"),
        }
    }
}

/// Trait para abstracción del controlador PID - Dependency Inversion Principle
pub trait PidController {
    type Error;
    
    /// Establece la temperatura objetivo
    fn set_target(&mut self, target: f32) -> Result<(), Self::Error>;
    
    /// Habilita el controlador PID
    fn enable(&mut self);
    
    /// Deshabilita el controlador PID
    fn disable(&mut self);
    
    /// Calcula la salida del PID basado en la temperatura actual y tiempo
    fn compute_output(&mut self, current_temp: f32, current_time: u32) -> f32;
    
    /// Verifica si el PID está habilitado
    fn is_enabled(&self) -> bool;
    
    /// Obtiene la temperatura objetivo actual
    fn get_target(&self) -> f32;
}

/// Trait para abstracción del gestor de salida - Dependency Inversion Principle  
pub trait OutputManager {
    type Error;
    
    /// Procesa el estado del sistema y lo envía a la salida
    fn process_status(&mut self, status: &crate::config::SystemStatus) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send;
    
    /// Reinicia el estado del gestor de salida
    fn reset(&mut self);
    
    /// Habilita la salida continua
    fn enable_continuous_output(&mut self);
    
    /// Deshabilita la salida continua
    fn disable_continuous_output(&mut self);
    
    /// Verifica si la salida continua está habilitada
    fn is_continuous_enabled(&self) -> bool;
}

/// Implementación del trait PidController para el PID existente
impl PidController for crate::control::pid::CoffeeRoasterPid {
    type Error = crate::control::pid::PidError;
    
    fn set_target(&mut self, target: f32) -> Result<(), Self::Error> {
        self.set_target(target)
    }
    
    fn enable(&mut self) {
        self.enable()
    }
    
    fn disable(&mut self) {
        self.disable()
    }
    
    fn compute_output(&mut self, current_temp: f32, current_time: u32) -> f32 {
        self.compute_output(current_temp, current_time)
    }
    
    fn is_enabled(&self) -> bool {
        self.is_enabled()
    }
    
    fn get_target(&self) -> f32 {
        // Implementación por defecto - sobrescribir en la implementación concreta
        0.0
    }
}

/// Implementación del trait OutputManager para el OutputManager existente
impl OutputManager for crate::output::OutputManager {
    type Error = crate::output::OutputError;
    
    async fn process_status(&mut self, status: &crate::config::SystemStatus) -> Result<(), Self::Error> {
        self.process_status(status).await
    }
    
    fn reset(&mut self) {
        self.reset()
    }
    
    fn enable_continuous_output(&mut self) {
        self.enable_continuous_output()
    }
    
    fn disable_continuous_output(&mut self) {
        self.disable_continuous_output()
    }
    
    fn is_continuous_enabled(&self) -> bool {
        // OutputManager doesn't expose this directly, so we'll use a default implementation
        // In a real implementation, this would need to be added to OutputManager trait
        true
    }
}