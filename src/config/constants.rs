// GPIO Pin Assignments for LibreRoaster ESP32-C3
// These pins are optimized for the ESP32-C3 capabilities and coffee roaster application

// GPIO Pin Assignments for ESP32-C3 Coffee Roaster
pub const SPI_SCLK_PIN: u8 = 7; // SPI Clock for MAX31856
pub const SPI_MOSI_PIN: u8 = 5; // SPI MOSI for MAX31856
pub const SPI_MISO_PIN: u8 = 6; // SPI MISO for MAX31856
pub const THERMOCOUPLE_BT_CS_PIN: u8 = 4; // Bean Temperature Chip Select
pub const THERMOCOUPLE_ET_CS_PIN: u8 = 3; // Environment Temperature Chip Select
pub const SSR_CONTROL_PIN: u8 = 2; // Solid State Relay Control
pub const FAN_PWM_PIN: u8 = 8; // Fan PWM Control
pub const UART_TX_PIN: u8 = 21; // UART Transmit to Artisan+
pub const UART_RX_PIN: u8 = 22; // UART Receive from Artisan+

// PWM Configuration
pub const FAN_PWM_FREQUENCY_HZ: u32 = 25000; // 25kHz for DC fan motor
pub const SSR_PWM_FREQUENCY_HZ: u32 = 1; // 1Hz for heating element (slow PWM)

// Hardware Configuration
pub const PWM_FREQUENCY: u32 = 1000; // 1Hz PWM frequency for SSR (suitable for heating elements)

// Temperature Settings
pub const DEFAULT_TARGET_TEMP: f32 = 225.0; // Default roasting temperature in Celsius
pub const MAX_SAFE_TEMP: f32 = 250.0; // Maximum safe temperature limit
pub const MIN_TEMP: f32 = 0.0; // Minimum temperature reading
pub const MAX_TEMP: f32 = 300.0; // Maximum temperature reading range

// Control Settings
pub const PID_SAMPLE_TIME_MS: u32 = 100; // 10Hz sampling frequency (100ms)
pub const TEMPERATURE_READ_INTERVAL_MS: u32 = 160; // MAX31856 conversion time + margin

// Safety Settings
pub const OVERTEMP_THRESHOLD: f32 = 260.0; // Emergency shutdown temperature
pub const TEMP_VALIDITY_TIMEOUT_MS: u32 = 1000; // Timeout for temperature sensor validity

// Calibration Constants (can be adjusted per thermocouple)
pub const BT_THERMOCOUPLE_OFFSET: f32 = 0.0; // Bean temperature calibration offset
pub const ET_THERMOCOUPLE_OFFSET: f32 = 0.0; // Environment temperature calibration offset

// Output/Serial Configuration
pub const DEFAULT_OUTPUT_INTERVAL_MS: u64 = 1000; // 1Hz default output frequency

// Roaster State Machine
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RoasterState {
    Idle,          // System ready, heating off
    Heating,       // Temperature ramping to target
    Stable,        // Temperature at target, roasting
    Cooling,       // Cooling down after roast
    Fault,         // System in fault state
    EmergencyStop, // Emergency shutdown
}

// Artisan+ Input Commands (from Artisan software)
#[derive(Debug, Clone, Copy)]
pub enum ArtisanCommand {
    ReadStatus,    // READ -> ET,BT,Power,Fan
    StartRoast,    // START -> Begin roasting and continuous output
    SetHeater(u8), // OT1 x (0-100%)
    SetFan(u8),    // IO3 x (0-100%)
    EmergencyStop, // STOP
}

// Roaster Control Commands (internal)
#[derive(Debug, Clone, Copy)]
pub enum RoasterCommand {
    StartRoast(f32),      // Start roasting with target temperature
    StopRoast,            // Stop current roast
    SetTemperature(f32),  // Set target temperature
    EmergencyStop,        // Immediate shutdown
    Reset,                // Reset system
    SetHeaterManual(u8),  // Manual heater control (overrides PID)
    SetFanManual(u8),     // Manual fan control
    ArtisanEmergencyStop, // Artisan+ specific stop
}

// System Status Information
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
        }
    }
}
