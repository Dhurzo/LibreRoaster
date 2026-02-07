// GPIO Pin Assignments for LibreRoaster ESP32-C3
// These pins are optimized for the ESP32-C3 capabilities and coffee roaster application
// Note: GPIO2, GPIO8, GPIO9 are strapping pins and are NOT used to avoid boot issues

pub const SPI_SCLK_PIN: u8 = 7;
pub const SPI_MOSI_PIN: u8 = 5;
pub const SPI_MISO_PIN: u8 = 6;
pub const THERMOCOUPLE_BT_CS_PIN: u8 = 4;
pub const THERMOCOUPLE_ET_CS_PIN: u8 = 3;
pub const SSR_CONTROL_PIN: u8 = 10;
pub const HEAT_DETECTION_PIN: u8 = 1;
pub const FAN_PWM_PIN: u8 = 9;
pub const UART_TX_PIN: u8 = 20;
pub const UART_RX_PIN: u8 = 21;

pub const FAN_PWM_FREQUENCY_HZ: u32 = 25000;
pub const SSR_PWM_FREQUENCY_HZ: u32 = 1;
pub const FAN_LEDC_CHANNEL: u8 = 0;
pub const SSR_LEDC_CHANNEL: u8 = 1;
pub const SSR_PWM_RESOLUTION: u8 = 8;

pub const PWM_FREQUENCY: u32 = 1000;

pub const DEFAULT_TARGET_TEMP: f32 = 225.0;
pub const MAX_SAFE_TEMP: f32 = 250.0;
pub const MIN_TEMP: f32 = 0.0;
pub const MAX_TEMP: f32 = 300.0;
pub const MIN_VALID_TEMP: f32 = 0.0;
pub const MAX_VALID_TEMP: f32 = 300.0;

pub const PID_SAMPLE_TIME_MS: u32 = 100;
pub const TEMPERATURE_READ_INTERVAL_MS: u32 = 160;

pub const OVERTEMP_THRESHOLD: f32 = 260.0;
pub const TEMP_VALIDITY_TIMEOUT_MS: u32 = 1000;
pub const SSR_DETECTION_TIMEOUT_MS: u32 = 100;
pub const HEAT_SOURCE_CHECK_INTERVAL_MS: u32 = 5000;

pub const BT_THERMOCOUPLE_OFFSET: f32 = 0.0;
pub const ET_THERMOCOUPLE_OFFSET: f32 = 0.0;

pub const DEFAULT_OUTPUT_INTERVAL_MS: u64 = 1000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RoasterState {
    Idle,
    Heating,
    Stable,
    Cooling,
    Fault,
    EmergencyStop,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtisanCommand {
    ReadStatus,
    StartRoast,
    SetHeater(u8),
    SetFan(u8),
    /// OT2 command with fan speed (0-100, decimals rounded, clamped silently)
    /// bool indicates if original value was out of range (triggers heater stop)
    SetFanSpeed(u8, bool),
    EmergencyStop,
    IncreaseHeater,
    DecreaseHeater,
    Chan(u16),
    Units(bool),
    Filt(u8),
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
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SsrHardwareStatus {
    Available,
    NotDetected,
    Error,
}

/// Temperature scale preference for Artisan protocol
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TemperatureScale {
    Celsius,
    Fahrenheit,
}

impl Default for TemperatureScale {
    fn default() -> Self {
        TemperatureScale::Celsius
    }
}

/// Temperature settings storage
/// Tracks temperature scale preference without applying conversion
#[derive(Debug, Clone, Copy)]
pub struct TemperatureSettings {
    scale: TemperatureScale,
}

impl TemperatureSettings {
    pub fn new() -> Self {
        Self {
            scale: TemperatureScale::default(),
        }
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
}

#[derive(Debug, Clone, Copy)]
pub struct SystemStatus {
    pub state: RoasterState,
    pub bean_temp: f32,
    pub env_temp: f32,
    pub target_temp: f32,
    pub ssr_output: f32,
    pub fan_output: f32,
    pub pid_enabled: bool,
    pub artisan_control: bool,
    pub fault_condition: bool,
    pub ssr_hardware_status: SsrHardwareStatus,
}

impl Default for SystemStatus {
    fn default() -> Self {
        Self {
            state: RoasterState::Idle,
            bean_temp: 0.0,
            env_temp: 0.0,
            target_temp: DEFAULT_TARGET_TEMP,
            ssr_output: 0.0,
            fan_output: 0.0,
            pid_enabled: false,
            artisan_control: false,
            fault_condition: false,
            ssr_hardware_status: SsrHardwareStatus::NotDetected,
        }
    }
}
