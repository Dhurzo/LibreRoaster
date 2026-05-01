// GPIO Pin Assignments for LibreRoaster ESP32-C3
// These pins are optimized for the ESP32-C3 capabilities and coffee roaster application
// Note: ESP32-C3 has strapping pins (e.g. GPIO2/GPIO8/GPIO9). We avoid GPIO2/GPIO8/GPIO9.
//
// SPI2 (FSPI) IO MUX functions on ESP32-C3:
//   FSPICLK (SCK)  : GPIO6
//   FSPID   (MOSI) : GPIO7
//   FSPIQ   (MISO) : GPIO2 (strapping, avoid) → use GPIO5 via GPIO Matrix

pub const SPI_SCLK_PIN: u8 = 6;
pub const SPI_MOSI_PIN: u8 = 7;
pub const SPI_MISO_PIN: u8 = 5;
pub const THERMOCOUPLE_BT_CS_PIN: u8 = 4;
pub const THERMOCOUPLE_ET_CS_PIN: u8 = 3;
pub const SSR_CONTROL_PIN: u8 = 10;
pub const HEAT_DETECTION_PIN: u8 = 1;
pub const FAN_PWM_PIN: u8 = 0;
pub const UART_TX_PIN: u8 = 21;
pub const UART_RX_PIN: u8 = 20;

pub const FAN_PWM_FREQUENCY_HZ: u32 = 25000;
pub const SSR_PWM_FREQUENCY_HZ: u32 = 1;
pub const FAN_LEDC_CHANNEL: u8 = 0;
pub const SSR_LEDC_CHANNEL: u8 = 1;
pub const SSR_LEDC_TIMER: u8 = 0; // Timer0 for SSR (~1Hz zero-crossing)
pub const FAN_LEDC_TIMER: u8 = 1; // Timer1 for Fan (25kHz silent operation)
pub const SSR_PWM_RESOLUTION: u8 = 8;
pub const SSR_CYCLE_GUARD_MS: u32 = 100;
pub const SSR_DUTY_TOLERANCE_TICKS: u8 = 2;

pub const PWM_FREQUENCY: u32 = 1000;

pub const DEFAULT_TARGET_TEMP: f32 = 225.0;
pub const MAX_SAFE_TEMP: f32 = 250.0;
pub const MIN_TEMP: f32 = 0.0;
pub const MAX_TEMP: f32 = 300.0;
pub const MIN_VALID_TEMP: f32 = -50.0;
pub const MAX_VALID_TEMP: f32 = 350.0;

/// PID control loop sample time in milliseconds.
/// Sensor reads (~160ms) may exceed this interval, see stale-data guard in update_control().
pub const PID_SAMPLE_TIME_MS: u32 = 100;
/// MAX31856 thermocouple read time in milliseconds (SPI + conversion latency).
/// Exceeds PID_SAMPLE_TIME_MS; stale-data guard prevents PID from using old readings.
pub const TEMPERATURE_READ_INTERVAL_MS: u32 = 160;

pub const OVERTEMP_THRESHOLD: f32 = 260.0;
pub const TEMP_VALIDITY_TIMEOUT_MS: u32 = 1000;
pub const SSR_DETECTION_TIMEOUT_MS: u32 = 100;
pub const HEAT_SOURCE_CHECK_INTERVAL_MS: u32 = 5000;

pub const BT_THERMOCOUPLE_OFFSET: f32 = 0.0;
pub const ET_THERMOCOUPLE_OFFSET: f32 = 0.0;

pub const DEFAULT_OUTPUT_INTERVAL_MS: u64 = 1000;

/// Control loop is expected to feed the Task Watchdog at this cadence.
pub const WATCHDOG_FEED_INTERVAL_MS: u64 = 100;
pub const HW_WATCHDOG_TIMEOUT_SECS: u32 = 2;
pub const LEDC_GUARD_TIMEOUT_MS: u64 = 40;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RoasterState {
    Idle,
    Preheating,
    Heating,
    Stable,
    Cooling,
    Fault,
    EmergencyStop,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArtisanCommand {
    ReadStatus,
    StatusReport,
    StartRoast,
    SetHeater(u8),
    SetFan(u8),
    SetFanSpeed(u8, bool),
    EmergencyStop,
    IncreaseHeater,
    DecreaseHeater,
    Chan(u16),
    Units(bool),
    Filt(u8),
    RunRegression,
    SetPidGain(f32, f32, f32),
    SetTargetTemp(f32),
    SetProfile,
    DumpLog,
    Preheat(f32),
    SetFanProfile,
    SetPidChannel(u8),           // PID;CHAN;2 — 1=ET, 2=BT (default)
    SetPidCycleTime(u32),        // PID;CT;1000 — cycle time in ms
    SetPidOutputLimits(f32, f32), // PID;LIMIT;0;100 — min/max output %
}

pub const MAX_PROFILE_SETPOINTS: usize = 16;
pub const MAX_COMMANDS_PER_TICK: usize = 8;
pub const CHARGE_DROP_THRESHOLD_C: f32 = 20.0;
pub const CHARGE_DETECTION_WINDOW_S: u32 = 3;
pub const PREHEAT_HOLD_TOLERANCE_C: f32 = 2.0;

/// A single setpoint in a roast profile: at time_secs → target temperature °C.
#[derive(Debug, Clone, Copy)]
pub struct ProfileSetpoint {
    pub time_secs: u32,
    pub temperature: f32,
}

/// Fan profile setpoint: at time_secs → fan speed %. Same format as temperature profile.
#[derive(Debug, Clone, Copy)]
pub struct FanSetpoint {
    pub time_secs: u32,
    pub fan_speed: u8,
}

pub struct FanProfile {
    pub setpoints: heapless::Vec<FanSetpoint, MAX_PROFILE_SETPOINTS>,
}

impl FanProfile {
    pub fn new() -> Self {
        Self { setpoints: heapless::Vec::new() }
    }
    pub fn target_at(&self, elapsed_secs: u32) -> Option<u8> {
        if self.setpoints.is_empty() { return None; }
        if elapsed_secs <= self.setpoints[0].time_secs {
            return Some(self.setpoints[0].fan_speed);
        }
        for i in 1..self.setpoints.len() {
            let prev = self.setpoints[i - 1];
            let curr = self.setpoints[i];
            if elapsed_secs <= curr.time_secs {
                let range = curr.time_secs - prev.time_secs;
                if range == 0 { return Some(curr.fan_speed); }
                let frac = (elapsed_secs - prev.time_secs) as f32 / range as f32;
                let interp = prev.fan_speed as f32 + (curr.fan_speed as f32 - prev.fan_speed as f32) * frac;
                return Some((interp + 0.5) as u8);
            }
        }
        Some(self.setpoints[self.setpoints.len() - 1].fan_speed)
    }
}

/// Roast profile received from Artisan as a sequence of time-temperature setpoints.
/// Artisan sends: `PROFILE;0,50;120,150;300,200;480,225` meaning
/// at 0s→50°C, 120s→150°C, 300s→200°C, 480s→225°C.
/// The firmware interpolates linearly between setpoints during the roast.
pub struct RoastProfile {
    pub setpoints: heapless::Vec<ProfileSetpoint, MAX_PROFILE_SETPOINTS>,
}

impl RoastProfile {
    pub fn new() -> Self {
        Self { setpoints: heapless::Vec::new() }
    }

    /// Compute target temperature at elapsed_secs using linear interpolation.
    /// Returns None if profile is empty.
    pub fn target_at(&self, elapsed_secs: u32) -> Option<f32> {
        if self.setpoints.is_empty() {
            return None;
        }
        // Before first setpoint: return first setpoint temp
        if elapsed_secs <= self.setpoints[0].time_secs {
            return Some(self.setpoints[0].temperature);
        }
        // Find bracketing setpoints
        for i in 1..self.setpoints.len() {
            let prev = self.setpoints[i - 1];
            let curr = self.setpoints[i];
            if elapsed_secs <= curr.time_secs {
                let range = curr.time_secs - prev.time_secs;
                if range == 0 {
                    return Some(curr.temperature);
                }
                let frac = (elapsed_secs - prev.time_secs) as f32 / range as f32;
                return Some(prev.temperature + (curr.temperature - prev.temperature) * frac);
            }
        }
        // After last setpoint: hold at final temperature
        Some(self.setpoints[self.setpoints.len() - 1].temperature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Fan profile interpolation ──────────────

    fn make_fan_profile(pairs: &[(u32, u8)]) -> FanProfile {
        let mut p = FanProfile::new();
        for &(t, s) in pairs {
            p.setpoints.push(FanSetpoint { time_secs: t, fan_speed: s }).unwrap();
        }
        p
    }

    #[test]
    fn fan_profile_empty_returns_none() {
        let p = FanProfile::new();
        assert_eq!(p.target_at(0), None);
        assert_eq!(p.target_at(100), None);
    }

    #[test]
    fn fan_profile_before_first() {
        let p = make_fan_profile(&[(10, 30), (20, 80)]);
        assert_eq!(p.target_at(0), Some(30));
        assert_eq!(p.target_at(5), Some(30));
    }

    #[test]
    fn fan_profile_exact_setpoint() {
        let p = make_fan_profile(&[(0, 20), (60, 60), (120, 100)]);
        assert_eq!(p.target_at(0), Some(20));
        assert_eq!(p.target_at(60), Some(60));
        assert_eq!(p.target_at(120), Some(100));
    }

    #[test]
    fn fan_profile_interpolation_midpoint() {
        let p = make_fan_profile(&[(0, 0), (100, 100)]);
        assert_eq!(p.target_at(50), Some(50));
    }

    #[test]
    fn fan_profile_after_last_holds() {
        let p = make_fan_profile(&[(0, 20), (30, 80)]);
        assert_eq!(p.target_at(60), Some(80));
        assert_eq!(p.target_at(999), Some(80));
    }

    #[test]
    fn fan_profile_single_setpoint() {
        let p = make_fan_profile(&[(0, 50)]);
        assert_eq!(p.target_at(0), Some(50));
        assert_eq!(p.target_at(100), Some(50));
    }

    #[test]
    fn fan_profile_max_setpoints() {
        let mut p = FanProfile::new();
        for i in 0..MAX_PROFILE_SETPOINTS {
            p.setpoints.push(FanSetpoint { time_secs: i as u32 * 10, fan_speed: i as u8 * 6 }).unwrap();
        }
        assert_eq!(p.setpoints.len(), MAX_PROFILE_SETPOINTS);
        assert_eq!(p.target_at(0), Some(0));
        assert_eq!(p.target_at((MAX_PROFILE_SETPOINTS - 1) as u32 * 10), Some((MAX_PROFILE_SETPOINTS - 1) as u8 * 6));
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RoasterCommand {
    StartRoast(f32),
    StopRoast,
    SetTemperature(f32),
    EmergencyStop,
    Reset,
    SetHeaterManual(u8),
    SetFanManual(u8),
    ArtisanEmergencyStop,
    IncreaseHeater,
    DecreaseHeater,
    SetUnits(bool), // true = Fahrenheit, false = Celsius
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SsrHardwareStatus {
    Available,
    #[default]
    NotDetected,
    Error,
}

/// Temperature scale preference for Artisan protocol
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TemperatureScale {
    #[default]
    Celsius,
    Fahrenheit,
}

/// Temperature settings storage
/// Tracks temperature scale preference without applying conversion
#[derive(Debug, Clone, Copy, Default)]
pub struct TemperatureSettings {
    scale: TemperatureScale,
}

impl TemperatureSettings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_scale(&self) -> TemperatureScale {
        self.scale
    }

    pub fn set_scale(&mut self, scale: TemperatureScale) {
        self.scale = scale;
    }

    /// Check if scale is Fahrenheit
    pub fn is_fahrenheit(&self) -> bool {
        matches!(self.scale, TemperatureScale::Fahrenheit)
    }

    /// Convert temperature from Celsius to display units.
    /// If scale is Fahrenheit: °F = °C × 9.0/5.0 + 32.0
    /// If scale is Celsius: return temp_c unchanged
    pub fn convert_to_display(&self, temp_c: f32) -> f32 {
        match self.scale {
            TemperatureScale::Fahrenheit => temp_c * 9.0 / 5.0 + 32.0,
            TemperatureScale::Celsius => temp_c,
        }
    }
}

/// Current roast state and instrumentation telemetry.
///
/// **Future consideration**: Split into `CoreRoastStatus` (state, temps, outputs —
/// needed every tick) and `InstrumentationSnapshot` (watchdog, PID internals,
/// latency metrics — polled on demand) to reduce stack pressure in the control loop.
#[derive(Debug, Clone, Copy)]
pub struct SystemStatus {
    pub state: RoasterState,
    pub bean_temp: f32,
    pub env_temp: f32,
    pub ambient_temp: f32,
    pub target_temp: f32,
    pub ssr_output: f32,
    pub fan_output: f32,
    pub pid_enabled: bool,
    pub artisan_control: bool,
    pub fault_condition: bool,
    pub ssr_hardware_status: SsrHardwareStatus,
    pub ssr_last_duty_delta_ticks: i16,
    pub ssr_retry_count: u8,
    pub ssr_cycle_guard_busy_until_ms: u64,
    pub watchdog_feed_ok: bool,
    pub watchdog_last_failure: Option<&'static str>,
    pub watchdog_consecutive_failures: u8,
    pub ledc_guard_timeouts: u16,
    pub overtemp_regression_active: bool,
    pub pv: f32,
    pub mv: f32,
    pub integrator_value: f32,
    pub derivative_rate: f32,
    pub saturation_active: bool,
    pub integrator_clamped: bool,
    pub derivative_available: bool,
    pub command_latency_us: u32,
    pub max_command_latency_us: u32,
    pub temperature_settings: TemperatureSettings,
    pub charge_detected: bool,
    pub pid_channel: u8,           // 1=ET, 2=BT, default=2
    pub pid_cycle_time_ms: u32,    // default=100 (PID_SAMPLE_TIME_MS)
    pub pid_output_min: f32,       // default=0.0
    pub pid_output_max: f32,       // default=100.0
}

impl Default for SystemStatus {
    fn default() -> Self {
        Self {
            state: RoasterState::Idle,
            bean_temp: 0.0,
            env_temp: 0.0,
            ambient_temp: 0.0,
            target_temp: DEFAULT_TARGET_TEMP,
            ssr_output: 0.0,
            fan_output: 0.0,
            pid_enabled: false,
            artisan_control: false,
            fault_condition: false,
            ssr_hardware_status: SsrHardwareStatus::NotDetected,
            ssr_last_duty_delta_ticks: 0,
            ssr_retry_count: 0,
            ssr_cycle_guard_busy_until_ms: 0,
            watchdog_feed_ok: true,
            watchdog_last_failure: None,
            watchdog_consecutive_failures: 0,
            ledc_guard_timeouts: 0,
            overtemp_regression_active: false,
            pv: 0.0,
            mv: 0.0,
            integrator_value: 0.0,
            derivative_rate: 0.0,
            saturation_active: false,
            integrator_clamped: false,
            derivative_available: false,
            command_latency_us: 0,
            max_command_latency_us: 0,
            temperature_settings: TemperatureSettings::new(),
            charge_detected: false,
            pid_channel: 2,
            pid_cycle_time_ms: PID_SAMPLE_TIME_MS,
            pid_output_min: 0.0,
            pid_output_max: 100.0,
        }
    }
}
