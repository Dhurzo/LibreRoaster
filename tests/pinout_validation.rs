//! Static validation tests for the GPIO pin table.
//!
//! These tests verify that the pin assignments in `config::pinout` are
//! internally consistent, avoid forbidden pins, and declare all required
//! strapping-pin constraints. They run on host (x86_64) without hardware.

use libreroaster::config::constants::*;
use libreroaster::config::pinout::*;

#[test]
fn no_duplicate_gpio_assignments() {
    let mut seen = [false; 22];
    for pin in PIN_TABLE {
        let gpio = pin.gpio as usize;
        assert!(
            !seen[gpio],
            "GPIO{} is assigned to more than one function (duplicate: \"{}\")",
            gpio,
            pin.function,
        );
        seen[gpio] = true;
    }
}

#[test]
fn no_forbidden_pins_used() {
    for pin in PIN_TABLE {
        assert!(
            !FORBIDDEN_PINS.contains(&pin.gpio),
            "GPIO{} (\"{}\") is in FORBIDDEN_PINS — must not be used",
            pin.gpio,
            pin.function,
        );
    }
}

#[test]
fn all_strapping_pins_have_required_constraints() {
    for pin in PIN_TABLE {
        let Some(strapping) = pin.strapping else { continue };

        match strapping {
            StrappingWarning::Uwl => {
                assert!(
                    pin.constraints
                        .contains(&PinConstraint::ExternalPullUpRequired),
                    "GPIO{} (\"{}\") is ULW strapping but missing ExternalPullUpRequired",
                    pin.gpio,
                    pin.function,
                );
            }
            StrappingWarning::JtagEnable => {
                assert!(
                    pin.constraints.contains(&PinConstraint::PushPullOnly),
                    "GPIO{} (\"{}\") is JTAG strapping but missing PushPullOnly",
                    pin.gpio,
                    pin.function,
                );
            }
            StrappingWarning::VddSpi => {
                panic!(
                    "GPIO{} has VddSpi strapping — it should be in FORBIDDEN_PINS, not PIN_TABLE",
                    pin.gpio
                );
            }
        }
    }
}

#[test]
fn spi_miso_is_input() {
    let miso = find_pin(SPI_MISO_PIN).unwrap_or_else(|| {
        panic!("No pin found for SPI_MISO_PIN ({})", SPI_MISO_PIN)
    });
    assert_eq!(
        miso.direction,
        PinDirection::Input,
        "GPIO{} (MISO) must be Input",
        miso.gpio,
    );
}

#[test]
fn uart_rx_is_input_tx_is_output() {
    let rx = find_pin(UART_RX_PIN).unwrap_or_else(|| {
        panic!("No pin found for UART_RX_PIN ({})", UART_RX_PIN)
    });
    let tx = find_pin(UART_TX_PIN).unwrap_or_else(|| {
        panic!("No pin found for UART_TX_PIN ({})", UART_TX_PIN)
    });

    assert_eq!(
        rx.direction,
        PinDirection::Input,
        "GPIO{} (UART RX) must be Input",
        rx.gpio,
    );
    assert_eq!(
        tx.direction,
        PinDirection::Output,
        "GPIO{} (UART TX) must be Output",
        tx.gpio,
    );
}

#[test]
fn all_ledc_pwm_pins_are_output() {
    for pin in PIN_TABLE {
        if pin.peripheral == PinPeripheral::LedcPwm {
            assert_eq!(
                pin.direction,
                PinDirection::Output,
                "GPIO{} (\"{}\") is LEDC PWM but not Output",
                pin.gpio,
                pin.function,
            );
        }
    }
}

#[test]
fn heat_detection_is_input_with_pullup() {
    let det = find_pin(HEAT_DETECTION_PIN).unwrap_or_else(|| {
        panic!(
            "No pin found for HEAT_DETECTION_PIN ({})",
            HEAT_DETECTION_PIN
        )
    });
    assert_eq!(
        det.direction,
        PinDirection::Input,
        "GPIO{} (Heat Detection) must be Input",
        det.gpio,
    );
    assert!(
        det.constraints
            .contains(&PinConstraint::InternalPullUpRequired),
        "GPIO{} (Heat Detection) must declare InternalPullUpRequired",
        det.gpio,
    );
}

#[test]
fn fan_pwm_gpio9_has_flyback_constraint() {
    let fan = find_pin(FAN_PWM_PIN).unwrap_or_else(|| {
        panic!("No pin found for FAN_PWM_PIN ({})", FAN_PWM_PIN)
    });
    assert!(
        fan.constraints
            .contains(&PinConstraint::FlybackDiodeRequired),
        "GPIO{} (Fan PWM) must declare FlybackDiodeRequired",
        fan.gpio,
    );
}

#[test]
fn pin_constants_match_pin_table() {
    let pairs: &[(u8, &str)] = &[
        (SPI_SCLK_PIN, "SPI_SCLK_PIN"),
        (SPI_MOSI_PIN, "SPI_MOSI_PIN"),
        (SPI_MISO_PIN, "SPI_MISO_PIN"),
        (THERMOCOUPLE_ET_CS_PIN, "THERMOCOUPLE_ET_CS_PIN"),
        (THERMOCOUPLE_BT_CS_PIN, "THERMOCOUPLE_BT_CS_PIN"),
        (SSR_CONTROL_PIN, "SSR_CONTROL_PIN"),
        (HEAT_DETECTION_PIN, "HEAT_DETECTION_PIN"),
        (FAN_PWM_PIN, "FAN_PWM_PIN"),
        (UART_TX_PIN, "UART_TX_PIN"),
        (UART_RX_PIN, "UART_RX_PIN"),
        (STATUS_LED_PIN, "STATUS_LED_PIN"),
    ];

    for &(pin_const, name) in pairs {
        let found = find_pin(pin_const);
        assert!(
            found.is_some(),
            "{} (= GPIO{}) has no matching entry in PIN_TABLE",
            name,
            pin_const,
        );
        assert_eq!(
            found.unwrap().gpio,
            pin_const,
            "{} constant doesn't match its PIN_TABLE entry",
            name,
        );
    }
}

#[test]
fn wiring_manifest_and_pin_table_are_bidirectionally_consistent() {
    for entry in WIRING_MANIFEST {
        let pin = find_pin(entry.gpio).unwrap_or_else(|| {
            panic!(
                "WIRING_MANIFEST references GPIO{} (\"{}\") but no PIN_TABLE entry exists",
                entry.gpio, entry.component,
            )
        });
        assert!(
            !pin.function.is_empty(),
            "GPIO{} in manifest maps to unnamed pin",
            entry.gpio,
        );
    }

    let manifest_gpios: Vec<u8> = WIRING_MANIFEST.iter().map(|e| e.gpio).collect();
    for pin in PIN_TABLE {
        assert!(
            manifest_gpios.contains(&pin.gpio),
            "PIN_TABLE GPIO{} (\"{}\") has no WIRING_MANIFEST entry",
            pin.gpio,
            pin.function,
        );
    }
}

#[test]
fn gpio_numbers_within_valid_range() {
    for pin in PIN_TABLE {
        assert!(
            pin.gpio <= 21,
            "GPIO{} (\"{}\") exceeds ESP32-C3 max GPIO21",
            pin.gpio,
            pin.function,
        );
    }
}

#[test]
fn init_rs_uses_pins_from_constants() {
    // Verify that init.rs uses the SAME pin numbers as constants.rs
    // by checking that the constants match what the hardware init expects.
    // If init.rs is refactored to use constants (GPIO9 -> FAN_PWM_PIN),
    // this test ensures the mapping stays correct.
    let expected = [
        (1, HEAT_DETECTION_PIN, "Heat Detection (GPIO1)"),
        (3, THERMOCOUPLE_ET_CS_PIN, "ET CS (GPIO3)"),
        (4, THERMOCOUPLE_BT_CS_PIN, "BT CS (GPIO4)"),
        (5, SPI_MISO_PIN, "SPI MISO (GPIO5)"),
        (6, SPI_SCLK_PIN, "SPI SCLK (GPIO6)"),
        (7, SPI_MOSI_PIN, "SPI MOSI (GPIO7)"),
        (9, FAN_PWM_PIN, "Fan PWM (GPIO9)"),
        (10, SSR_CONTROL_PIN, "SSR PWM (GPIO10)"),
        (8, STATUS_LED_PIN, "Status LED (GPIO8)"),
        (20, UART_RX_PIN, "UART RX (GPIO20)"),
        (21, UART_TX_PIN, "UART TX (GPIO21)"),
    ];

    for (expected_gpio, constant, label) in expected {
        assert_eq!(
            constant,
            expected_gpio,
            "{}: constant value {} != expected GPIO{}",
            label,
            constant,
            expected_gpio,
        );
    }
}
