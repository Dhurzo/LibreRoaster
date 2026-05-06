//! Single source of truth for every GPIO pin on the ESP32-C3.
//!
//! This module defines a structured pin table (`PIN_TABLE`) with metadata about
//! direction, peripheral, strapping-pin constraints, and wiring connections.
//! All hardware init code should reference this (or `constants.rs` which is
//! cross-checked against it) — never hardcode pin numbers.
//!
//! # ESP32-C3 GPIO Notes
//!
//! - 22 GPIOs (0–21), but GPIO0–5 have specific restrictions.
//! - GPIO2 is a strapping pin (VDD_SPI voltage selection) — must NOT be used.
//! - GPIO8 is a strapping pin (JTAG enable) — push-pull output only.
//! - GPIO9 is a strapping pin (ULW / boot mode) — external pull-up required.
//! - All GPIOs are 3.3 V; absolute maximum input is 3.6 V.

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Logical direction of a GPIO pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinDirection {
    Input,
    Output,
    Bidirectional,
}

/// Hardware peripheral a pin is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinPeripheral {
    /// Fast SPI (FSPI) bus — shared between both MAX31856 thermocouples.
    Spi,
    /// LEDC PWM peripheral.
    LedcPwm,
    /// UART0 serial (Artisan communication).
    Uart,
    /// Direct GPIO (no peripheral multiplexing needed).
    GpioDirect,
    /// Native USB (internal, not routed to external pins).
    Usb,
    /// Pin is intentionally unused / left floating.
    Unused,
}

/// Strapping-pin warnings for ESP32-C3.
///
/// Strapping pins are sampled during reset to determine boot behaviour.
/// Misconfiguring them prevents the chip from booting or damages the flash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrappingWarning {
    /// GPIO2 — selects VDD_SPI voltage (1.8 V vs 3.3 V). Must be left floating.
    VddSpi,
    /// GPIO8 — enables JTAG / boot mode when pulled LOW. Push-pull output only.
    JtagEnable,
    /// GPIO9 — ULW (Upload/Log Wait). Pulled LOW → chip enters download mode.
    /// Requires external pull-up to 3.3 V.
    Uwl,
}

/// Constraints that must be documented (and verified externally) for safe
/// operation. These are *software-documented* constraints — the tests ensure
/// they are declared; the actual hardware must implement them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinConstraint {
    /// External 10 kOhm pull-up resistor to 3.3 V is mandatory.
    ExternalPullUpRequired,
    /// Pin must be configured as push-pull output only (never open-drain).
    PushPullOnly,
    /// Do not connect anything to this pin.
    DoNotConnect,
    /// Maximum continuous output current is 20 mA per GPIO.
    MaxCurrent20mA,
    /// A series resistor is required between this pin and the external device
    /// (typically 10 kOhm for signal protection).
    SeriesResistorRequired,
    /// A flyback / freewheel diode is required across the inductive load
    /// connected to this pin.
    FlybackDiodeRequired,
    /// Internal pull-up must be enabled on this input pin.
    InternalPullUpRequired,
    /// Pin drives a high-voltage load through an SSR — output is logic-level
    /// only (never exceeds 3.3 V).
    LogicLevelOnly,
}

// ---------------------------------------------------------------------------
// PinSpec — one entry per GPIO
// ---------------------------------------------------------------------------

/// Describes a single GPIO pin assignment on the ESP32-C3.
#[derive(Debug, Clone, Copy)]
pub struct PinSpec {
    /// GPIO number (0–21 for ESP32-C3).
    pub gpio: u8,
    /// Human-readable function name (e.g. "MAX31856 ET CS").
    pub function: &'static str,
    /// Logical direction of the pin.
    pub direction: PinDirection,
    /// Peripheral the pin is routed to.
    pub peripheral: PinPeripheral,
    /// Strapping-pin warning, if applicable.
    pub strapping: Option<StrappingWarning>,
    /// Safety / electrical constraints that must be satisfied externally.
    pub constraints: &'static [PinConstraint],
}

// ---------------------------------------------------------------------------
// Forbidden pins — must never appear in PIN_TABLE
// ---------------------------------------------------------------------------

/// GPIOs that are forbidden to use on the ESP32-C3 in this project.
pub const FORBIDDEN_PINS: &[u8] = &[2]; // GPIO2 = VDD_SPI strapping

// ---------------------------------------------------------------------------
// The pin table — single source of truth
// ---------------------------------------------------------------------------

/// Complete GPIO allocation for LibreRoaster on ESP32-C3.
///
/// Every used pin has exactly one entry. Tests validate:
/// - No duplicate GPIO numbers
/// - No forbidden pins used
/// - Strapping pins have required constraints declared
/// - Directions match expected peripheral usage
pub const PIN_TABLE: &[PinSpec] = &[
    // ── Feedback / Detection ───────────────────────────────────────────
    PinSpec {
        gpio: 1,
        function: "SSR Heat Detection",
        direction: PinDirection::Input,
        peripheral: PinPeripheral::GpioDirect,
        strapping: None,
        constraints: &[
            PinConstraint::InternalPullUpRequired,
            PinConstraint::LogicLevelOnly,
        ],
    },
    // ── SPI Bus (shared between two MAX31856 thermocouple amplifiers) ──
    PinSpec {
        gpio: 3,
        function: "MAX31856 ET Chip Select",
        direction: PinDirection::Output,
        peripheral: PinPeripheral::Spi,
        strapping: None,
        constraints: &[PinConstraint::MaxCurrent20mA],
    },
    PinSpec {
        gpio: 4,
        function: "MAX31856 BT Chip Select",
        direction: PinDirection::Output,
        peripheral: PinPeripheral::Spi,
        strapping: None,
        constraints: &[PinConstraint::MaxCurrent20mA],
    },
    PinSpec {
        gpio: 5,
        function: "SPI MISO (GPIO Matrix)",
        direction: PinDirection::Input,
        peripheral: PinPeripheral::Spi,
        strapping: None,
        constraints: &[],
    },
    PinSpec {
        gpio: 6,
        function: "SPI SCLK (FSPICLK)",
        direction: PinDirection::Output,
        peripheral: PinPeripheral::Spi,
        strapping: None,
        constraints: &[],
    },
    PinSpec {
        gpio: 7,
        function: "SPI MOSI (FSPID)",
        direction: PinDirection::Output,
        peripheral: PinPeripheral::Spi,
        strapping: None,
        constraints: &[],
    },
    // ── PWM Outputs (LEDC) ─────────────────────────────────────────────
    PinSpec {
        gpio: 9,
        function: "Fan PWM (25 kHz)",
        direction: PinDirection::Output,
        peripheral: PinPeripheral::LedcPwm,
        strapping: Some(StrappingWarning::Uwl),
        constraints: &[
            PinConstraint::ExternalPullUpRequired,
            PinConstraint::FlybackDiodeRequired,
            PinConstraint::MaxCurrent20mA,
        ],
    },
    PinSpec {
        gpio: 10,
        function: "SSR PWM (1 Hz)",
        direction: PinDirection::Output,
        peripheral: PinPeripheral::LedcPwm,
        strapping: None,
        constraints: &[PinConstraint::LogicLevelOnly],
    },
    // ── Status ──────────────────────────────────────────────────────────
    PinSpec {
        gpio: 8,
        function: "Status LED",
        direction: PinDirection::Output,
        peripheral: PinPeripheral::GpioDirect,
        strapping: Some(StrappingWarning::JtagEnable),
        constraints: &[PinConstraint::PushPullOnly],
    },
    // ── UART0 (Artisan communication) ──────────────────────────────────
    PinSpec {
        gpio: 20,
        function: "UART0 RX (from CH341 TX)",
        direction: PinDirection::Input,
        peripheral: PinPeripheral::Uart,
        strapping: None,
        constraints: &[PinConstraint::SeriesResistorRequired],
    },
    PinSpec {
        gpio: 21,
        function: "UART0 TX (to CH341 RX)",
        direction: PinDirection::Output,
        peripheral: PinPeripheral::Uart,
        strapping: None,
        constraints: &[],
    },
];

// ---------------------------------------------------------------------------
// Wiring manifest — GPIO → external component mapping
// ---------------------------------------------------------------------------

/// Expected physical wiring: each entry maps a GPIO to the external component
/// pin it connects to, with a brief description of the connection.
///
/// This is the bridge between firmware pin constants and the physical wiring
/// documented in `pinout.md`. Tests cross-check this against both.
pub const WIRING_MANIFEST: &[WiringEntry] = &[
    WiringEntry {
        gpio: 1,
        component: "SSR feedback optocoupler collector",
        note: "Pull-up to 3.3 V internal; reads LOW when SSR conducts",
    },
    WiringEntry {
        gpio: 3,
        component: "MAX31856 #1 CS pin",
        note: "Active-low chip select for ET (Environment Temperature)",
    },
    WiringEntry {
        gpio: 4,
        component: "MAX31856 #2 CS pin",
        note: "Active-low chip select for BT (Bean Temperature)",
    },
    WiringEntry {
        gpio: 5,
        component: "MAX31856 SDO (MISO)",
        note: "Shared SPI data in — GPIO Matrix (not native FSPIQ on GPIO2)",
    },
    WiringEntry {
        gpio: 6,
        component: "MAX31856 SCK (SCLK)",
        note: "Shared SPI clock (FSPICLK native)",
    },
    WiringEntry {
        gpio: 7,
        component: "MAX31856 SDI (MOSI)",
        note: "Shared SPI data out (FSPID native)",
    },
    WiringEntry {
        gpio: 8,
        component: "Status LED anode",
        note: "Push-pull output, 330 Ohm series resistor to LED → GND",
    },
    WiringEntry {
        gpio: 9,
        component: "Fan MOSFET gate",
        note: "1 kOhm series resistor to gate; 10 kOhm pull-up to 3.3 V; 10 kOhm pull-down on gate side",
    },
    WiringEntry {
        gpio: 10,
        component: "SSR control input",
        note: "Logic-level PWM to SSR + input (3.3 V compatible SSR required)",
    },
    WiringEntry {
        gpio: 20,
        component: "CH341 TX → ESP RX",
        note: "USB-UART adapter TX through 1 kOhm + 3.3 V Zener clamp",
    },
    WiringEntry {
        gpio: 21,
        component: "ESP TX → CH341 RX",
        note: "Direct connection (ESP32-C3 TX at 3.3 V)",
    },
];

/// A single entry in the wiring manifest linking a GPIO to an external component.
#[derive(Debug, Clone, Copy)]
pub struct WiringEntry {
    pub gpio: u8,
    pub component: &'static str,
    pub note: &'static str,
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Find a pin spec by GPIO number.
pub fn find_pin(gpio: u8) -> Option<&'static PinSpec> {
    PIN_TABLE.iter().find(|p| p.gpio == gpio)
}

/// Get all output pins.
pub fn output_pins() -> impl Iterator<Item = &'static PinSpec> {
    PIN_TABLE.iter().filter(|p| p.direction == PinDirection::Output)
}

/// Get all input pins.
pub fn input_pins() -> impl Iterator<Item = &'static PinSpec> {
    PIN_TABLE.iter().filter(|p| p.direction == PinDirection::Input)
}

/// Get all strapping pins (those with a strapping warning).
pub fn strapping_pins() -> impl Iterator<Item = &'static PinSpec> {
    PIN_TABLE.iter().filter(|p| p.strapping.is_some())
}

/// Total number of GPIOs used by the firmware.
pub const fn used_pin_count() -> usize {
    PIN_TABLE.len()
}
