// GPIO Pin Assignments for LibreRoaster ESP32-C3
// These pins are optimized for the ESP32-C3 capabilities and coffee roaster application

// SPI Pins for MAX31856 Thermocouples (shared bus)
pub const SPI_SCLK_PIN: u8 = 7; // GPIO7 - SPI Clock
pub const SPI_MISO_PIN: u8 = 6; // GPIO6 - MISO (Master In Slave Out)
pub const SPI_MOSI_PIN: u8 = 5; // GPIO5 - MOSI (Master Out Slave In)

// Chip Select pins for individual thermocouples
pub const BT_CS_PIN: u8 = 4; // GPIO4 - Bean Temperature Thermocouple CS
pub const ET_CS_PIN: u8 = 3; // GPIO3 - Environment Temperature Thermocouple CS

// SSR Control Pin
pub const SSR_PIN: u8 = 2; // GPIO2 - SSR PWM Output

// Hardware Configuration
pub const SPI_FREQUENCY: u32 = 1_000_000; // 1MHz SPI frequency for MAX31856
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

// Network Settings
pub const HTTP_SERVER_PORT: u16 = 80;
pub const WIFI_CONNECT_TIMEOUT_MS: u32 = 30000; // 30 seconds to connect to WiFi

// Calibration Constants (can be adjusted per thermocouple)
pub const BT_THERMOCOUPLE_OFFSET: f32 = 0.0; // Bean temperature calibration offset
pub const ET_THERMOCOUPLE_OFFSET: f32 = 0.0; // Environment temperature calibration offset

// Thermocouple Types
pub const BT_THERMOCOUPLE_TYPE: u8 = 0x03; // Type K thermocouple
pub const ET_THERMOCOUPLE_TYPE: u8 = 0x03; // Type K thermocouple

// MAX31856 Register Addresses
pub const MAX31856_CR0_REG: u8 = 0x80; // Configuration Register 0
pub const MAX31856_CR1_REG: u8 = 0x81; // Configuration Register 1
pub const MAX31856_MASK_REG: u8 = 0x82; // Fault Mask Register
pub const MAX31856_CJHF_REG: u8 = 0x83; // Cold Junction High Fault
pub const MAX31856_CJLF_REG: u8 = 0x84; // Cold Junction Low Fault
pub const MAX31856_LTHFTH_REG: u8 = 0x85; // Linearized Temperature High Fault Threshold
pub const MAX31856_LTHFTL_REG: u8 = 0x86; // Linearized Temperature High Fault Threshold
pub const MAX31856_LTLFTH_REG: u8 = 0x87; // Linearized Temperature Low Fault Threshold
pub const MAX31856_LTLFTL_REG: u8 = 0x88; // Linearized Temperature Low Fault Threshold
pub const MAX31856_CJTO_REG: u8 = 0x89; // Cold Junction Temperature Offset
pub const MAX31856_CJTH_REG: u8 = 0x8A; // Cold Junction Temperature High Byte
pub const MAX31856_CJTL_REG: u8 = 0x8B; // Cold Junction Temperature Low Byte
pub const MAX31856_LTCBH_REG: u8 = 0x8C; // Linearized Temperature Cold-Junction Compensated High Byte
pub const MAX31856_LTCBM_REG: u8 = 0x8D; // Linearized Temperature Cold-Junction Compensated Mid Byte
pub const MAX31856_LTCBL_REG: u8 = 0x8E; // Linearized Temperature Cold-Junction Compensated Low Byte
pub const MAX31856_SR_REG: u8 = 0x8F; // Status Register

// Status Register Bit Masks
pub const MAX31856_FAULT_CRIT: u8 = 0x80; // Critical Fault
pub const MAX31856_FAULT_SCV: u8 = 0x40; // Thermocouple Input Shorted to VCC
pub const MAX31856_FAULT_SCG: u8 = 0x20; // Thermocouple Input Shorted to GND
pub const MAX31856_FAULT_OPEN: u8 = 0x10; // Thermocouple Input Open
pub const MAX31856_FAULT_OVUV: u8 = 0x08; // Over/Under Voltage
pub const MAX31856_FAULT_CJRANGE: u8 = 0x04; // Cold-Junction Range
pub const MAX31856_FAULT_TCRANGE: u8 = 0x02; // Thermocouple Range
pub const MAX31856_FAULT_CNVRANGE: u8 = 0x01; // Converter Range

// Error Messages
pub const WAKE_MESSAGE: &str = "Wake the f*** up samurai we have beans to burn!";

// Output/Serial Configuration
pub const DEFAULT_OUTPUT_INTERVAL_MS: u64 = 1000; // 1Hz default output frequency
pub const ARTISAN_BAUD_RATE: u32 = 115200; // Artisan+ compatible baud rate
pub const OUTPUT_BUFFER_SIZE: usize = 256; // Buffer size for output formatting

// Artisan+ Protocol Configuration
pub const ARTISAN_FORMAT: &str = "#time,ET,BT,ROR,Power,DeltaBT";
pub const ARTISAN_DECIMAL_PLACES: usize = 1; // Decimal places for temperature
pub const ARTISAN_ROR_DECIMAL_PLACES: usize = 2; // Decimal places for rate of rise
pub const ROR_HISTORY_SIZE: usize = 5; // Number of samples for ROR calculation

// PID Controller Gains (Coffee Roaster Optimized)
pub const PID_KP: f32 = 2.0; // Proportional gain
pub const PID_KI: f32 = 0.01; // Integral gain (small to prevent overshoot)
pub const PID_KD: f32 = 0.5; // Derivative gain (moderate for damping)

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

// Temperature Sensor Type
#[derive(Debug, Clone, Copy)]
pub enum TemperatureSensor {
    BeanTemp, // BT - Bean Temperature
    EnvTemp,  // ET - Environment Temperature
}

// Roaster Control Commands
#[derive(Debug, Clone, Copy)]
pub enum RoasterCommand {
    StartRoast(f32),     // Start roasting with target temperature
    StopRoast,           // Stop current roast
    SetTemperature(f32), // Set target temperature
    EmergencyStop,       // Immediate shutdown
    Reset,               // Reset system
}

// System Status Information
#[derive(Debug, Clone, Copy)]
pub struct SystemStatus {
    pub state: RoasterState,
    pub bean_temp: f32,
    pub env_temp: f32,
    pub target_temp: f32,
    pub ssr_output: f32,
    pub pid_enabled: bool,
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
            pid_enabled: false,
            fault_condition: false,
        }
    }
}
