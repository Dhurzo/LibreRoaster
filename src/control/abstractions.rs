use embassy_time::Instant;

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



/// Versión mejorada de TemperatureCommandHandler usando abstracciones
pub struct TemperatureCommandHandlerV2<T: PidController, U: OutputManager> {
    pid_controller: T,
    output_manager: U,
}

impl<T: PidController, U: OutputManager> TemperatureCommandHandlerV2<T, U> {
    pub fn new(pid_controller: T, output_manager: U) -> Self {
        Self {
            pid_controller,
            output_manager,
        }
    }

    pub fn get_pid_output(&mut self, bean_temp: f32, current_time: Instant) -> f32 {
        self.pid_controller.compute_output(bean_temp, current_time.as_millis() as u32)
    }

    pub fn set_pid_target(&mut self, target_temp: f32) -> Result<(), T::Error> {
        self.pid_controller.set_target(target_temp)
    }

    pub fn enable_pid(&mut self) {
        self.pid_controller.enable();
    }

    pub fn disable_pid(&mut self) {
        self.pid_controller.disable();
    }

    pub fn is_pid_enabled(&self) -> bool {
        self.pid_controller.is_enabled()
    }

    pub fn get_pid_target(&self) -> f32 {
        self.pid_controller.get_target()
    }

    pub fn get_output_manager(&self) -> &U {
        &self.output_manager
    }

    pub fn get_output_manager_mut(&mut self) -> &mut U {
        &mut self.output_manager
    }
}

/// Implementación genérica del command handler usando las abstracciones
impl<T: PidController, U: OutputManager> crate::control::command_handler::RoasterCommandHandler 
    for TemperatureCommandHandlerV2<T, U> 
{
    fn handle_command(
        &mut self,
        command: crate::config::RoasterCommand,
        _current_time: embassy_time::Instant,
        status: &mut crate::config::SystemStatus,
    ) -> Result<(), super::RoasterError> {
        use crate::config::RoasterCommand;
        use log::info;

        match command {
            RoasterCommand::StartRoast(target_temp) => {
                self.set_pid_target(target_temp).map_err(|_| super::RoasterError::PidError)?;
                self.enable_pid();
                status.target_temp = target_temp;
                status.pid_enabled = true;

                // Reset output manager for new roast
                self.output_manager.reset();

                info!(
                    "Artisan+ control started with target temperature: {:.1}°C",
                    target_temp
                );
                Ok(())
            }

            RoasterCommand::StopRoast => {
                self.disable_pid();
                status.ssr_output = 0.0;
                status.pid_enabled = false;

                info!("Artisan+ control stopped - heating disabled");
                Ok(())
            }

            RoasterCommand::SetTemperature(target_temp) => {
                self.set_pid_target(target_temp).map_err(|_| super::RoasterError::PidError)?;
                status.target_temp = target_temp;

                info!("Artisan+ target temperature set to: {:.1}°C", target_temp);
                Ok(())
            }

            RoasterCommand::SetHeaterManual(value) => {
                // Artisan+ takes control, disable PID
                status.artisan_control = true;
                status.pid_enabled = false;
                self.disable_pid();

                info!("Artisan+ manual heater set to: {}%", value);
                Ok(())
            }

            _ => Err(super::RoasterError::InvalidState),
        }
    }

    fn can_handle(&self, command: crate::config::RoasterCommand) -> bool {
        use crate::config::RoasterCommand;
        matches!(
            command,
            RoasterCommand::StartRoast(_) |
            RoasterCommand::StopRoast |
            RoasterCommand::SetTemperature(_) |
            RoasterCommand::SetHeaterManual(_)
        )
    }
}