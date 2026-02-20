use super::{RoasterCommandHandler, RoasterError};
use crate::config::*;
use crate::control::handlers::{
    ArtisanCommandHandler, SafetyCommandHandler, SystemCommandHandler, TemperatureCommandHandler,
};
use crate::control::traits::{Fan, Heater};
use crate::control::SsrCycleGuard;
use alloc::boxed::Box;
#[cfg(not(target_arch = "riscv32"))]
use core::marker::PhantomData;
use embassy_time::{Duration, Instant};
use log::{debug, error, info, warn};

#[cfg(target_arch = "riscv32")]
use crate::hardware::max31856::{bt_spi::BtSpi, et_spi::EtSpi, Max31856};

/// RoasterControl - uses concrete Max31856 types for sensor storage
/// This enables calling async temperature methods without blocking the executor
pub struct RoasterControl {
    state: RoasterState,
    status: SystemStatus,
    ssr_guard: SsrCycleGuard,
    last_temp_read: Option<Instant>,
    last_pid_update: Option<Instant>,

    heater: Box<dyn Heater + Send>,
    fan: Box<dyn Fan + Send>,
    /// Sensors stored as concrete Max31856 types - enables async temperature reading
    #[cfg(target_arch = "riscv32")]
    bean_sensor: Max31856<BtSpi>,
    #[cfg(target_arch = "riscv32")]
    env_sensor: Max31856<EtSpi>,
    /// PhantomData for non-riscv32 targets to maintain type consistency
    /// Using fn() -> () which is Send + Sync
    #[cfg(not(target_arch = "riscv32"))]
    _bean_sensor: PhantomData<fn() -> ()>,
    #[cfg(not(target_arch = "riscv32"))]
    _env_sensor: PhantomData<fn() -> ()>,

    temp_handler: TemperatureCommandHandler,

    safety_handler: SafetyCommandHandler,
    artisan_handler: ArtisanCommandHandler,
    system_handler: SystemCommandHandler,

    /// Temperature scale preference storage
    /// Tracks UNITS command preference (Celsius/Fahrenheit) without conversion
    temp_settings: TemperatureSettings,
}

impl RoasterControl {
    #[cfg(target_arch = "riscv32")]
    pub fn new(
        heater: Box<dyn Heater + Send>,
        fan: Box<dyn Fan + Send>,
        bean_sensor: Max31856<BtSpi>,
        env_sensor: Max31856<EtSpi>,
    ) -> Result<Self, RoasterError> {
        let temp_handler = TemperatureCommandHandler::new()?;

        Ok(RoasterControl {
            state: RoasterState::Idle,
            status: SystemStatus::default(),
            ssr_guard: SsrCycleGuard::new(),
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
            temp_settings: TemperatureSettings::new(),
        })
    }

    #[cfg(not(target_arch = "riscv32"))]
    pub fn new(
        heater: Box<dyn Heater + Send>,
        fan: Box<dyn Fan + Send>,
    ) -> Result<Self, RoasterError> {
        let temp_handler = TemperatureCommandHandler::new()?;

        Ok(RoasterControl {
            state: RoasterState::Idle,
            status: SystemStatus::default(),
            ssr_guard: SsrCycleGuard::new(),
            last_temp_read: None,
            last_pid_update: None,
            heater,
            fan,
            _bean_sensor: PhantomData,
            _env_sensor: PhantomData,
            temp_handler,
            safety_handler: SafetyCommandHandler::new(),
            artisan_handler: ArtisanCommandHandler::new(),
            system_handler: SystemCommandHandler,
            temp_settings: TemperatureSettings::new(),
        })
    }

    /// Async sensor reading - uses async MAX31856 methods to avoid blocking executor
    /// This is the gap closure: storing concrete Max31856 types enables async calls
    #[cfg(target_arch = "riscv32")]
    pub async fn read_sensors(&mut self) -> Result<(), RoasterError> {
        let current_time = Instant::now();

        // Using async temperature reads - no longer blocks the async executor
        // The concrete Max31856 type gives us access to read_temperature_async()
        let raw_bt = self.bean_sensor.read_temperature_async().await?;
        let raw_et = self.env_sensor.read_temperature_async().await?;

        self.update_temperatures(raw_bt, raw_et, current_time)
    }

    /// Async sensor reading - stub for non-riscv32 targets
    #[cfg(not(target_arch = "riscv32"))]
    pub async fn read_sensors(&mut self) -> Result<(), RoasterError> {
        // Stub for host target - actual sensor reading not available
        Ok(())
    }

    /// Sync sensor reading - kept for backwards compatibility
    /// Note: This now uses the concrete Max31856 types but calls sync methods
    #[cfg(target_arch = "riscv32")]
    pub fn read_sensors_sync(&mut self) -> Result<(), RoasterError> {
        let current_time = Instant::now();
        let raw_bt = self.bean_sensor.read_temperature()?;
        let raw_et = self.env_sensor.read_temperature()?;
        self.update_temperatures(raw_bt, raw_et, current_time)
    }

    /// Sync sensor reading - stub for non-riscv32 targets
    #[cfg(not(target_arch = "riscv32"))]
    pub fn read_sensors_sync(&mut self) -> Result<(), RoasterError> {
        // Stub for host target - actual sensor reading not available
        Ok(())
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
        if matches!(command, RoasterCommand::StopRoast) {
            return self.stop_streaming();
        }

        if let RoasterCommand::SetHeaterManual(value) = command {
            return self.apply_manual_heater(value, current_time);
        }

        if let RoasterCommand::SetFanManual(value) = command {
            return self.apply_manual_fan(value);
        }

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

    fn is_streaming(&self) -> bool {
        self.temp_handler
            .get_output_manager()
            .is_continuous_enabled()
            || self.status.pid_enabled
            || self.status.artisan_control
    }

    fn stop_streaming(&mut self) -> Result<(), RoasterError> {
        self.temp_handler
            .get_output_manager_mut()
            .disable_continuous_output();
        self.temp_handler.disable_pid();
        self.status.pid_enabled = false;
        self.status.artisan_control = false;
        self.artisan_handler.clear_manual();
        self.status.ssr_output = 0.0;
        self.status.fan_output = 0.0;
        self.state = crate::config::constants::RoasterState::Idle;
        self.status.state = self.state;
        self.status.ssr_cycle_guard_busy_until_ms = 0;

        if !self.safety_handler.is_emergency_active() {
            self.status.fault_condition = false;
        }

        let heater_result = self.heater.set_power(0.0);
        self.capture_ssr_monitor_metrics();
        heater_result.map_err(|_| RoasterError::HardwareError)?;
        self.fan
            .set_speed(0.0)
            .map_err(|_| RoasterError::HardwareError)?;

        self.status.ssr_hardware_status = self.heater.get_status();

        Ok(())
    }

    fn apply_manual_heater(
        &mut self,
        value: u8,
        current_time: Instant,
    ) -> Result<(), RoasterError> {
        if value > 100 {
            return Err(RoasterError::InvalidState);
        }

        let manual_value = (value as f32).clamp(0.0, 100.0);

        self.temp_handler.disable_pid();
        self.status.pid_enabled = false;
        self.status.artisan_control = true;
        self.artisan_handler.set_manual_heater(manual_value);
        self.temp_handler
            .get_output_manager_mut()
            .enable_continuous_output();

        self.apply_guarded_heater(manual_value, current_time, true)?;

        self.status.ssr_hardware_status = self.heater.get_status();

        info!(
            "Artisan+ manual heater set to {:.1}% (manual mode enabled)",
            manual_value
        );

        Ok(())
    }

    fn apply_manual_fan(&mut self, value: u8) -> Result<(), RoasterError> {
        if value > 100 {
            return Err(RoasterError::InvalidState);
        }

        let fan_value = (value as f32).clamp(0.0, 100.0);

        self.artisan_handler.set_manual_fan(fan_value);
        self.status.artisan_control = true;
        self.status.pid_enabled = false;

        self.temp_handler
            .get_output_manager_mut()
            .enable_continuous_output();

        self.fan
            .set_speed(fan_value)
            .map_err(|_| RoasterError::HardwareError)?;

        // Read the actual applied speed from the fan controller (via bus)
        self.status.fan_output = self.fan.get_speed();

        self.status.ssr_hardware_status = self.heater.get_status();

        info!(
            "Artisan+ manual fan set to {:.1}% (manual mode enabled)",
            self.status.fan_output
        );

        Ok(())
    }

    pub fn is_temperature_valid(temp: f32) -> bool {
        temp >= MIN_VALID_TEMP && temp <= MAX_VALID_TEMP
    }

    pub fn emergency_shutdown(&mut self, reason: &str) -> Result<(), RoasterError> {
        error!("Emergency shutdown: {}", reason);
        self.status.state = crate::config::constants::RoasterState::Error;
        self.status.ssr_output = 0.0;
        self.status.ssr_cycle_guard_busy_until_ms = 0;

        let _ = self.heater.set_power(0.0);
        self.capture_ssr_monitor_metrics();
        let _ = self.fan.set_speed(100.0);

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

        self.status.ssr_hardware_status = self.heater.get_status();

        let desired_output = if self.safety_handler.is_emergency_active() {
            debug!("Emergency active - forcing SSR output to 0%");
            0.0
        } else if self.status.artisan_control {
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
        };

        let applied_output = self.apply_guarded_heater(desired_output, current_time, false)?;

        let fan_output = self.artisan_handler.get_manual_fan();
        self.fan
            .set_speed(fan_output)
            .map_err(|_| RoasterError::HardwareError)?;

        // Read the actual applied speed from the fan controller (via bus)
        self.status.fan_output = self.fan.get_speed();

        self.status.state = self.state;

        if applied_output > 0.0
            && self.status.ssr_hardware_status
                != crate::config::constants::SsrHardwareStatus::Available
        {
            debug!(
                "SSR output {:.1}% applied but no heat source detected",
                applied_output
            );
        }

        Ok(applied_output)
    }

    fn busy_window_ms(now: Instant, busy_until: Instant) -> u64 {
        if busy_until > now {
            busy_until.duration_since(now).as_millis()
        } else {
            0
        }
    }

    fn update_guard_busy_ms(&mut self, now: Instant) {
        let busy_until = self.ssr_guard.busy_until();
        self.status.ssr_cycle_guard_busy_until_ms = Self::busy_window_ms(now, busy_until);
    }

    fn capture_ssr_monitor_metrics(&mut self) {
        self.status.ssr_last_duty_delta_ticks = self.heater.last_duty_delta_ticks();
        self.status.ssr_retry_count = self.heater.last_retry_count();

        if self.status.ssr_last_duty_delta_ticks != 0 || self.status.ssr_retry_count != 0 {
            info!(
                "SSR monitor delta {} ticks, retries {}",
                self.status.ssr_last_duty_delta_ticks, self.status.ssr_retry_count
            );
        }
    }

    fn apply_guarded_heater(
        &mut self,
        desired: f32,
        now: Instant,
        reject_on_busy: bool,
    ) -> Result<f32, RoasterError> {
        let clamped = desired.clamp(0.0, 100.0);
        self.update_guard_busy_ms(now);

        if clamped <= 0.0 {
            let power_result = self.heater.set_power(0.0);
            self.capture_ssr_monitor_metrics();
            power_result.map_err(|_| RoasterError::HardwareError)?;
            self.status.ssr_output = 0.0;
            self.update_guard_busy_ms(now);
            return Ok(0.0);
        }

        match self.ssr_guard.next_cycle_allowed(now) {
            Ok(_) => {
                self.ssr_guard.mark_cycle(now);
                let power_result = self.heater.set_power(clamped);
                self.capture_ssr_monitor_metrics();
                power_result.map_err(|_| RoasterError::HardwareError)?;
                self.status.ssr_output = clamped;
                self.update_guard_busy_ms(now);
                Ok(clamped)
            }
            Err(busy_until) => {
                self.status.ssr_cycle_guard_busy_until_ms = Self::busy_window_ms(now, busy_until);
                warn!("SSR cycle busy until {:?}", busy_until);
                if reject_on_busy {
                    Err(RoasterError::InvalidState)
                } else {
                    Ok(self.status.ssr_output)
                }
            }
        }
    }

    pub async fn process_output(&mut self) -> Result<(), RoasterError> {
        if let Err(e) = self
            .temp_handler
            .get_output_manager_mut()
            .process_status(&self.status)
            .await
        {
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

    pub fn process_artisan_command(
        &mut self,
        command: crate::config::ArtisanCommand,
    ) -> Result<(), RoasterError> {
        use crate::config::constants::DEFAULT_TARGET_TEMP;
        let current_time = embassy_time::Instant::now();

        match command {
            crate::config::ArtisanCommand::StartRoast => {
                if self.is_streaming() {
                    info!("Artisan+ START ignored - streaming already active");
                    self.status.ssr_hardware_status = self.heater.get_status();
                } else {
                    self.status.artisan_control = true;
                    self.enable_pid_control(DEFAULT_TARGET_TEMP)?;
                    self.temp_handler
                        .get_output_manager_mut()
                        .enable_continuous_output();

                    // Actualizar estado hardware
                    self.status.ssr_hardware_status = self.heater.get_status();
                    self.state = crate::config::constants::RoasterState::Heating;
                    self.status.state = self.state;

                    info!(
                        "Artisan+ roast started with target {:.1}°C - SSR: {:?}",
                        DEFAULT_TARGET_TEMP, self.status.ssr_hardware_status
                    );
                }
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

            crate::config::ArtisanCommand::SetFanSpeed(value, was_clamped) => {
                let fan_command = crate::config::RoasterCommand::SetFanManual(value);
                self.process_command(fan_command, current_time)?;

                if was_clamped {
                    // Out of range value - stop heater as safety measure
                    let _ = self.heater.set_power(0.0);
                    self.capture_ssr_monitor_metrics();
                    info!(
                        "Artisan+ OT2 out of range - heater stopped, fan set to {}%",
                        value
                    );
                } else {
                    info!("Artisan+ OT2 fan command processed: {}%", value);
                }
            }

            crate::config::ArtisanCommand::EmergencyStop => {
                self.stop_streaming()?;
                info!("Artisan+ stop requested - streaming disabled and outputs cleared");
            }

            crate::config::ArtisanCommand::IncreaseHeater => {
                let up_command = crate::config::RoasterCommand::IncreaseHeater;
                self.process_command(up_command, current_time)?;
                info!("Artisan+ UP command processed");
            }

            crate::config::ArtisanCommand::DecreaseHeater => {
                let down_command = crate::config::RoasterCommand::DecreaseHeater;
                self.process_command(down_command, current_time)?;
                info!("Artisan+ DOWN command processed");
            }

            crate::config::ArtisanCommand::ReadStatus => {
                self.status.ssr_hardware_status = self.heater.get_status();

                let response = crate::output::artisan::ArtisanFormatter::format_read_response_full(
                    &self.status,
                );

                // Validate response has 4 comma-separated values
                let parts: alloc::vec::Vec<&str> = response.split(',').collect();
                if response.trim().is_empty() || parts.len() != 4 {
                    error!(
                        "Malformed READ response from ArtisanFormatter: expected 4 values, got {}",
                        parts.len()
                    );
                }

                debug!(
                    "READ command - SSR status: {:?}, response generated",
                    self.status.ssr_hardware_status
                );
            }

            crate::config::ArtisanCommand::Chan(_) => {
                debug!("Chan command received - initialization handled by multiplexer");
            }
            crate::config::ArtisanCommand::Units(is_fahrenheit) => {
                let scale = if is_fahrenheit {
                    TemperatureScale::Fahrenheit
                } else {
                    TemperatureScale::Celsius
                };
                self.temp_settings.set_scale(scale);
                debug!("Units command received - scale set to {:?}", scale);
            }
            crate::config::ArtisanCommand::Filt(_) => {
                debug!("Filt command received - initialization handled by multiplexer");
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

            let output = self
                .temp_handler
                .get_pid_output(self.status.bean_temp, current_time);

            self.last_pid_update = Some(current_time);

            if self.state == crate::config::constants::RoasterState::Heating {
                let temp_error = (self.status.bean_temp - self.status.target_temp).abs();
                if temp_error < 2.0 {
                    self.state = crate::config::constants::RoasterState::Stable;
                    info!("Target temperature reached, entering stable state");
                }
            }

            debug!(
                "PID update: bean_temp={:.1}°C, target={:.1}°C, output={:.1}%",
                self.status.bean_temp, self.status.target_temp, output
            );

            output
        } else {
            self.status.ssr_output
        }
    }
}
