use super::{RoasterError, RoasterCommandHandler};
use crate::config::*;
use crate::control::handlers::{
    ArtisanCommandHandler, SafetyCommandHandler, SystemCommandHandler, TemperatureCommandHandler,
};
use crate::control::traits::{Fan, Heater, Thermometer};
use embassy_time::{Duration, Instant};
use log::{debug, error, info, warn};
use alloc::boxed::Box;

pub struct RoasterControl {
    state: RoasterState,
    status: SystemStatus,
    last_temp_read: Option<Instant>,
    last_pid_update: Option<Instant>,

    // Dependencias de Hardware inyectadas (Dynamic Dispatch)
    heater: Box<dyn Heater + Send>,
    fan: Box<dyn Fan + Send>,
    bean_sensor: Box<dyn Thermometer + Send>,
    env_sensor: Box<dyn Thermometer + Send>,

    temp_handler: TemperatureCommandHandler,

    safety_handler: SafetyCommandHandler,
    artisan_handler: ArtisanCommandHandler,
    system_handler: SystemCommandHandler,
}

impl RoasterControl {
    pub fn new(
        heater: Box<dyn Heater + Send>, 
        fan: Box<dyn Fan + Send>,
        bean_sensor: Box<dyn Thermometer + Send>,
        env_sensor: Box<dyn Thermometer + Send>
    ) -> Result<Self, RoasterError> {
        let temp_handler = TemperatureCommandHandler::new()?;

        Ok(RoasterControl {
            state: RoasterState::Idle,
            status: SystemStatus::default(),
            last_temp_read: None,
            last_pid_update: None,
            heater,
            fan,
            bean_sensor,
            env_sensor,
            temp_handler,
            safety_handler: SafetyCommandHandler::new(),
            artisan_handler: ArtisanCommandHandler::new(),
            system_handler: SystemCommandHandler,
        })
    }

    /// Lee las temperaturas de los sensores reales y actualiza el estado interno
    pub fn read_sensors(&mut self) -> Result<(), RoasterError> {
        let current_time = Instant::now();

        // Leer sensores
        let raw_bt = self.bean_sensor.read_temperature()?;
        let raw_et = self.env_sensor.read_temperature()?;

        // Usar update_temperatures para lógica de validación y offsets
        self.update_temperatures(raw_bt, raw_et, current_time)
    }

    pub fn get_status(&self) -> SystemStatus {
        self.status
    }

    pub fn get_state(&self) -> RoasterState {
        self.state
    }

    pub fn update_temperatures(
        &mut self,
        bean_temp: f32,
        env_temp: f32,
        current_time: Instant,
    ) -> Result<(), RoasterError> {
        // Validate temperature readings
        if !Self::is_temperature_valid(bean_temp) || !Self::is_temperature_valid(env_temp) {
            return Err(RoasterError::TemperatureOutOfRange);
        }

        self.status.bean_temp = bean_temp + BT_THERMOCOUPLE_OFFSET;
        self.status.env_temp = env_temp + ET_THERMOCOUPLE_OFFSET;
        self.last_temp_read = Some(current_time);

        // Check for emergency conditions
        if self.status.bean_temp >= OVERTEMP_THRESHOLD {
            self.emergency_shutdown("Over-temperature detected")?;
        }

        Ok(())
    }

    pub fn process_command(
        &mut self,
        command: RoasterCommand,
        current_time: Instant,
    ) -> Result<(), RoasterError> {
        let mut handlers: [&mut dyn RoasterCommandHandler; 4] = [
            &mut self.safety_handler,
            &mut self.temp_handler,
            &mut self.artisan_handler,
            &mut self.system_handler,
        ];

        for handler in &mut handlers {
            if handler.can_handle(command) {
                let result = handler.handle_command(command, current_time, &mut self.status);

                self.status.fault_condition = self.safety_handler.is_emergency_active();

                return result;
            }
        }

        warn!("No handler found for command: {:?}", command);
        Err(RoasterError::InvalidState)
    }

    pub fn is_temperature_valid(temp: f32) -> bool {
        temp >= MIN_VALID_TEMP && temp <= MAX_VALID_TEMP
    }

    pub fn emergency_shutdown(&mut self, reason: &str) -> Result<(), RoasterError> {
        error!("Emergency shutdown: {}", reason);
        self.status.state = crate::config::constants::RoasterState::Error;
        self.status.ssr_output = 0.0;
        
        // Forzar apagado de hardware directamente
        let _ = self.heater.set_power(0.0);
        let _ = self.fan.set_speed(100.0); // Cool down en emergencia? O apagado? Asumimos 100% para enfriar si es overtemp
        
        Err(RoasterError::EmergencyShutdown)
    }

    pub fn update_control(&mut self, current_time: Instant) -> Result<f32, RoasterError> {
        if let Some(last_read) = self.last_temp_read {
            if current_time.duration_since(last_read)
                > Duration::from_millis(TEMP_VALIDITY_TIMEOUT_MS as u64)
            {
                warn!("Temperature sensor timeout detected");
                self.emergency_shutdown("Temperature sensor timeout")?;
            }
        }

        // Obtener estado real del hardware calefactor
        self.status.ssr_hardware_status = self.heater.get_status();

        let output = if self.safety_handler.is_emergency_active() {
            debug!("Emergency active - forcing SSR output to 0%");
            0.0
        } else {
            if self.status.artisan_control {
                let manual_output = self.artisan_handler.get_manual_heater();
                debug!(
                    "Artisan+ control - manual heater output: {:.1}%",
                    manual_output
                );
                manual_output
            } else if self.status.pid_enabled {
                if self.status.ssr_hardware_status
                    == crate::config::constants::SsrHardwareStatus::Available
                {
                    self.update_pid_control(current_time)
                } else {
                    warn!("PID enabled but SSR not available - output: 0%");
                    0.0
                }
            } else {
                0.0
            }
        };

        // Aplicar salida al hardware calefactor
        self.heater.set_power(output)
            .map_err(|_| RoasterError::HardwareError)?;

        self.status.ssr_output = output.clamp(0.0, 100.0);
        
        // Gestionar ventilador (Fan)
        // Nota: En la versión anterior, el ventilador se controlaba indirectamente o era "manual"
        // Ahora lo controlamos explícitamente desde aquí basado en el handler de Artisan
        let fan_output = self.artisan_handler.get_manual_fan();
        self.fan.set_speed(fan_output)
             .map_err(|_| RoasterError::HardwareError)?;
             
        self.status.fan_output = fan_output;

        self.status.state = self.state;

        if output > 0.0
            && self.status.ssr_hardware_status
                != crate::config::constants::SsrHardwareStatus::Available
        {
            debug!(
                "SSR output {:.1}% applied but no heat source detected",
                output
            );
        }

        Ok(self.status.ssr_output)
    }

    pub async fn process_output(&mut self) -> Result<(), RoasterError> {
        if let Err(e) = self.temp_handler.get_output_manager_mut().process_status(&self.status).await {
            warn!("Output error: {:?}", e);
        }
        Ok(())
    }

    pub fn get_output_manager(&self) -> &crate::control::OutputController {
        self.temp_handler.get_output_manager()
    }

    pub fn get_output_manager_mut(&mut self) -> &mut crate::control::OutputController {
        self.temp_handler.get_output_manager_mut()
    }

    pub fn process_artisan_command(&mut self, command: crate::config::ArtisanCommand) -> Result<(), RoasterError> {
        use crate::config::constants::DEFAULT_TARGET_TEMP;
        let current_time = embassy_time::Instant::now();

        match command {
            crate::config::ArtisanCommand::StartRoast => {
                self.status.artisan_control = true;
                self.enable_pid_control(DEFAULT_TARGET_TEMP)?;
                self.temp_handler.get_output_manager_mut().enable_continuous_output();

                // Actualizar estado hardware
                self.status.ssr_hardware_status = self.heater.get_status();

                info!("Artisan+ roast started with target {:.1}°C - SSR: {:?}", 
                      DEFAULT_TARGET_TEMP, self.status.ssr_hardware_status);
            }

            crate::config::ArtisanCommand::SetHeater(value) => {
                let heater_command = crate::config::RoasterCommand::SetHeaterManual(value);
                self.process_command(heater_command, current_time)?;
                info!("Artisan+ heater command processed: {}%", value);
            }

            crate::config::ArtisanCommand::SetFan(value) => {
                let fan_command = crate::config::RoasterCommand::SetFanManual(value);
                self.process_command(fan_command, current_time)?;

                info!("Artisan+ fan command processed: {}%", value);
            }

            crate::config::ArtisanCommand::EmergencyStop => {
                self.temp_handler.get_output_manager_mut().disable_continuous_output();
                self.safety_handler.trigger_emergency("Artisan+ emergency stop")?;
                self.status.fault_condition = true;
                self.status.ssr_hardware_status = crate::config::constants::SsrHardwareStatus::Error;

                info!("Artisan+ emergency stop triggered");
            }

            crate::config::ArtisanCommand::ReadStatus => {
                self.status.ssr_hardware_status = self.heater.get_status();
                debug!("READ command - SSR status: {:?}", self.status.ssr_hardware_status);
            }
        }

        Ok(())
    }

    pub fn enable_pid_control(&mut self, target_temp: f32) -> Result<(), RoasterError> {
        self.status.artisan_control = false;
        self.temp_handler.set_pid_target(target_temp)?;
        self.temp_handler.enable_pid();
        self.status.pid_enabled = true;
        self.status.target_temp = target_temp;

        info!("PID control re-enabled with target: {:.1}°C", target_temp);

        Ok(())
    }

    pub fn get_fan_speed(&self) -> f32 {
        self.status.fan_output
    }

    fn update_pid_control(&mut self, current_time: embassy_time::Instant) -> f32 {
        use crate::config::constants::SsrHardwareStatus;
        
        let should_update = if let Some(last_update) = self.last_pid_update {
            current_time.duration_since(last_update)
                >= embassy_time::Duration::from_millis(crate::config::PID_SAMPLE_TIME_MS as u64)
        } else {
            true
        };

        if should_update {
            if self.status.ssr_hardware_status != SsrHardwareStatus::Available {
                warn!("PID update requested but SSR not available - skipping");
                return 0.0;
            }

            let output = self.temp_handler.get_pid_output(self.status.bean_temp, current_time);

            self.last_pid_update = Some(current_time);

            if self.state == crate::config::constants::RoasterState::Heating {
                let temp_error = (self.status.bean_temp - self.status.target_temp).abs();
                if temp_error < 2.0 {
                    self.state = crate::config::constants::RoasterState::Stable;
                    info!("Target temperature reached, entering stable state");
                }
            }

            debug!("PID update: bean_temp={:.1}°C, target={:.1}°C, output={:.1}%", 
                   self.status.bean_temp, self.status.target_temp, output);

            output
        } else {
            self.status.ssr_output
        }
    }


}
