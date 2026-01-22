// GPIO Pin Assignments for LibreRoaster ESP32-C3
// These pins are optimized for the ESP32-C3 capabilities and coffee roaster application

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
