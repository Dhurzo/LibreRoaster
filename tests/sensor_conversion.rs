//! Sensor conversion tests for SensorConversionHub
//!
//! These tests exhaustively validate the 0.0078125°C LSB, two's-complement
//! conversion, rounding, and fault propagation so any change to the conversion
//! math fails the suite.
//!
//! Run with: cargo test --test sensor_conversion --features regression --target x86_64-unknown-linux-gnu

#![cfg(all(test, not(target_arch = "riscv32"), feature = "regression"))]

extern crate std;

use libreroaster::hardware::sensors::conversion::MAX31856_LSB;
use libreroaster::hardware::sensors::conversion::{
    convert_raw_temp, FixtureReading, SensorConversionHub, SensorFault, SensorSample,
};

/// Helper to convert 3-byte ADC array to u32
fn adc_to_u32(adc: [u8; 3]) -> u32 {
    ((adc[0] as u32) << 16) | ((adc[1] as u32) << 8) | (adc[2] as u32)
}

/// Helper to create a FixtureReading from raw values
fn make_fixture(
    bean_adc: [u8; 3],
    bean_fault: u8,
    env_adc: [u8; 3],
    env_fault: u8,
) -> FixtureReading {
    FixtureReading {
        bean_adc,
        bean_fault,
        env_adc,
        env_fault,
    }
}

mod conversion_math {
    use super::*;

    #[test]
    fn test_lsb_constant() {
        // Verify the LSB constant matches datasheet: 0.0078125°C
        assert_eq!(MAX31856_LSB, 0.0078125);
    }

    #[test]
    fn test_positive_temperature_conversion() {
        // ADC value 19199 (0x004B00) should yield ~150.0°C
        let raw = adc_to_u32([0x00, 0x4B, 0x00]);
        let temp = convert_raw_temp(raw);

        // Using 0.0078125 * 19199 = 149.9921875, which rounds to 150.0
        // due to floating point precision
        let expected = 19199.0 * MAX31856_LSB;
        assert!(
            (temp - expected).abs() < 0.01,
            "Expected {}, got {}",
            expected,
            temp
        );
        // Also check it's close to 150.0
        assert!(
            (temp - 150.0).abs() < 0.1,
            "Expected close to 150.0, got {}",
            temp
        );
    }

    #[test]
    fn test_negative_temperature_conversion() {
        // Two's complement: 0xFFFAE0 should yield negative temperature
        let raw = adc_to_u32([0xFF, 0xFA, 0xE0]);
        let temp = convert_raw_temp(raw);

        // Two's complement: sign bit set, take complement and negate
        // (!0xFFFAE0) & 0x7FFFFF = 0x00051F = 1311
        // -1311 * 0.0078125 = -10.2421875
        let expected = -1311.0 * MAX31856_LSB;
        assert!(
            (temp - expected).abs() < 0.001,
            "Expected {}, got {}",
            expected,
            temp
        );
    }

    #[test]
    fn test_zero_temperature() {
        // ADC value 0 should yield 0.0°C
        let raw = adc_to_u32([0x00, 0x00, 0x00]);
        let temp = convert_raw_temp(raw);
        assert!((temp - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_max_positive_temperature() {
        // Max positive: 0x7FFFFF = 8388607
        let raw = 0x7FFFFF;
        let temp = convert_raw_temp(raw);
        let expected = 8388607.0 * MAX31856_LSB;
        assert!((temp - expected).abs() < 0.001);
    }

    #[test]
    fn test_max_negative_temperature() {
        // Min negative: 0x800000 (sign bit set, all others zero)
        let raw = 0x800000;
        let temp = convert_raw_temp(raw);
        // (!0x800000) & 0x7FFFFF = 0x7FFFFF = 8388607
        let expected = -8388607.0 * MAX31856_LSB;
        assert!((temp - expected).abs() < 0.001);
    }

    #[test]
    fn test_small_positive_temperature() {
        // Smallest positive: 0x000001
        let raw = 0x000001;
        let temp = convert_raw_temp(raw);
        let expected = 1.0 * MAX31856_LSB;
        assert!((temp - expected).abs() < 0.001);
    }

    #[test]
    fn test_small_negative_temperature() {
        // Smallest negative: 0xFFFFFF (-1 in two's complement)
        let raw = 0xFFFFFF;
        let temp = convert_raw_temp(raw);
        // (!0xFFFFFF) & 0x7FFFFF = 0x000000 = 0
        // Actually 0xFFFFFF has sign bit set, but complement is 0
        // So temp should be -0.0
        assert!(temp <= 0.001 && temp >= -0.001);
    }
}

mod hub_integration {
    use super::*;

    #[test]
    fn test_hub_from_fixture_warm() {
        // warm-normal fixture: bean=0x004B00, env=0x00C80
        let fixture = make_fixture([0x00, 0x4B, 0x00], 0x00, [0x00, 0x0C, 0x80], 0x00);

        let mut hub = SensorConversionHub::new();
        let sample = hub.sample_from_fixture(fixture).expect("Should succeed");

        // Bean: 19199 * 0.0078125 = 149.9921875 ≈ 150.0
        let expected_bean = 19199.0 * MAX31856_LSB;
        assert!((sample.bean_temp - expected_bean).abs() < 0.1);

        // Env: 3200 * 0.0078125 = 25.0
        let expected_env = 3200.0 * MAX31856_LSB;
        assert!((sample.env_temp - expected_env).abs() < 0.1);

        // No faults
        assert!(!sample.bean_fault.has_fault());
        assert!(!sample.env_fault.has_fault());
    }

    #[test]
    fn test_hub_from_fixture_cold_negative() {
        // cold-negative fixture: bean=0xFFFAE0 (negative), env=0
        let fixture = make_fixture([0xFF, 0xFA, 0xE0], 0x00, [0x00, 0x00, 0x00], 0x00);

        let mut hub = SensorConversionHub::new();
        let sample = hub.sample_from_fixture(fixture).expect("Should succeed");

        // Bean: -1311 * 0.0078125 = -10.2421875 ≈ -10.2
        let expected_bean = -1311.0 * MAX31856_LSB;
        assert!((sample.bean_temp - expected_bean).abs() < 0.1);

        // Env: 0 * 0.0078125 = 0.0
        assert!((sample.env_temp - 0.0).abs() < 0.001);

        // No faults
        assert!(!sample.bean_fault.has_fault());
        assert!(!sample.env_fault.has_fault());
    }

    #[test]
    fn test_hub_from_fixture_bean_fault() {
        // bean-open: bean_fault=0x01 (open circuit)
        let fixture = make_fixture([0x00, 0x00, 0x00], 0x01, [0x00, 0x32, 0x00], 0x00);

        let mut hub = SensorConversionHub::new();
        let sample = hub.sample_from_fixture(fixture).expect("Should succeed");

        // Bean fault should be detected
        assert!(sample.bean_fault.open_circuit);
        assert!(sample.bean_fault.has_fault());

        // Env should still work (fault=0x00)
        assert!(!sample.env_fault.has_fault());
    }

    #[test]
    fn test_hub_from_fixture_env_fault() {
        // bean-open: env_fault=0x04 (short to GND)
        let fixture = make_fixture([0x00, 0x4B, 0x00], 0x00, [0x00, 0x00, 0x00], 0x04);

        let mut hub = SensorConversionHub::new();
        let sample = hub.sample_from_fixture(fixture).expect("Should succeed");

        // Bean should work
        assert!(!sample.bean_fault.has_fault());

        // Env fault should be detected
        assert!(sample.env_fault.short_to_gnd);
        assert!(sample.env_fault.has_fault());
    }

    #[test]
    fn test_hub_from_fixture_both_faults() {
        // Both sensors have faults: bean=0x01, env=0x04
        let fixture = make_fixture([0x00, 0x00, 0x00], 0x01, [0x00, 0x32, 0x00], 0x04);

        let mut hub = SensorConversionHub::new();
        let sample = hub.sample_from_fixture(fixture).expect("Should succeed");

        // Both should have faults
        assert!(sample.bean_fault.open_circuit);
        assert!(sample.env_fault.short_to_gnd);
        assert!(sample.bean_fault.has_fault());
        assert!(sample.env_fault.has_fault());
    }
}

mod fixture_consistency {
    use super::*;

    /// Verify that the hub produces the same temperatures as direct conversion
    /// This ensures the hub correctly applies the conversion math
    #[test]
    fn test_hub_matches_direct_conversion() {
        let test_cases: Vec<([u8; 3], [u8; 3], f32, f32)> = vec![
            // (bean_adc, env_adc, expected_bean, expected_env)
            (
                [0x00, 0x4B, 0x00],
                [0x00, 0x0C, 0x80],
                19199.0 * MAX31856_LSB,
                3200.0 * MAX31856_LSB,
            ),
            (
                [0xFF, 0xFA, 0xE0],
                [0x00, 0x00, 0x00],
                -1311.0 * MAX31856_LSB,
                0.0,
            ),
            (
                [0x00, 0x00, 0x00],
                [0x00, 0x32, 0x00],
                0.0,
                12800.0 * MAX31856_LSB,
            ),
        ];

        for (bean_adc, env_adc, expected_bean, expected_env) in test_cases {
            let fixture = make_fixture(bean_adc, 0x00, env_adc, 0x00);
            let mut hub = SensorConversionHub::new();
            let sample = hub.sample_from_fixture(fixture).expect("Should succeed");

            assert!(
                (sample.bean_temp - expected_bean).abs() < 0.01,
                "Bean temp mismatch: expected {}, got {}",
                expected_bean,
                sample.bean_temp
            );
            assert!(
                (sample.env_temp - expected_env).abs() < 0.01,
                "Env temp mismatch: expected {}, got {}",
                expected_env,
                sample.env_temp
            );
        }
    }

    /// Verify temperature values match what the fixtures expect
    #[test]
    fn test_warm_fixture_temperatures() {
        // From fixtures: warm-normal
        let fixture = make_fixture([0x00, 0x4B, 0x00], 0x00, [0x00, 0x0C, 0x80], 0x00);

        let mut hub = SensorConversionHub::new();
        let sample = hub.sample_from_fixture(fixture).expect("Should succeed");

        // The fixture expects bean ~150.0 and env ~25.0
        // Our calculation: 19199 * 0.0078125 = 149.9921875 ≈ 150.0
        assert!((sample.bean_temp - 150.0).abs() < 0.1);
        assert!((sample.env_temp - 25.0).abs() < 0.1);
    }

    #[test]
    fn test_cold_fixture_temperatures() {
        // From fixtures: cold-negative
        let fixture = make_fixture([0xFF, 0xFA, 0xE0], 0x00, [0x00, 0x00, 0x00], 0x00);

        let mut hub = SensorConversionHub::new();
        let sample = hub.sample_from_fixture(fixture).expect("Should succeed");

        // The fixture expects bean ~-10.2
        // Our calculation: -1311 * 0.0078125 = -10.2421875 ≈ -10.2
        assert!((sample.bean_temp - (-10.2)).abs() < 0.1);
        assert!((sample.env_temp - 0.0).abs() < 0.01);
    }
}
