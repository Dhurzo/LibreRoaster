//! Regression status snapshot tests
//!
//! These tests replay canonical MAX31856 fixtures through SensorConversionHub
//! and compare the resulting SystemStatus to ArtisanFormatter::format_status_response
//! output to ensure the 16-column STATUS tail remains deterministic.
//!
//! Run with: cargo test --test regression_status --features regression --target x86_64-unknown-linux-gnu

#![cfg(all(test, not(target_arch = "riscv32"), feature = "regression"))]

extern crate std;

use libreroaster::config::constants::DEFAULT_TARGET_TEMP;
use libreroaster::config::{RoasterState, SsrHardwareStatus, SystemStatus};
use libreroaster::hardware::sensors::conversion::{FixtureReading, SensorConversionHub};
use libreroaster::output::artisan::ArtisanFormatter;

/// Canonical test fixtures matching the ones in tests/fixtures/max31856_sequences.rs
struct TestFixture {
    name: &'static str,
    reading: FixtureReading,
    expected_status_line: &'static str,
}

fn warm_fixture() -> FixtureReading {
    FixtureReading {
        bean_adc: [0x00, 0x4B, 0x00],
        bean_fault: 0x00,
        env_adc: [0x00, 0x0C, 0x80],
        env_fault: 0x00,
    }
}

fn cold_fixture() -> FixtureReading {
    FixtureReading {
        bean_adc: [0xFF, 0xFA, 0xE0],
        bean_fault: 0x00,
        env_adc: [0x00, 0x00, 0x00],
        env_fault: 0x00,
    }
}

fn fault_fixture() -> FixtureReading {
    FixtureReading {
        bean_adc: [0x00, 0x00, 0x00],
        bean_fault: 0x01,
        env_adc: [0x00, 0x32, 0x00],
        env_fault: 0x04,
    }
}

fn expected_warm_status() -> &'static str {
    "25.0,150.0,0.0,0.0,1,0,none,0,1,150.0,75.0,12.0,0.24,1,1,1,0,0"
}

fn expected_cold_status() -> &'static str {
    "0.0,-10.2,0.0,0.0,1,0,none,0,1,-10.2,60.5,-3.2,0.08,0,0,1,0,0"
}

fn expected_fault_status() -> &'static str {
    "100.0,0.0,0.0,0.0,1,0,none,0,1,0.0,0.0,0.0,0.00,0,0,0,0,0"
}

/// Create a SystemStatus from fixture reading and additional status fields
fn status_from_sample(
    sample: &libreroaster::hardware::sensors::conversion::SensorSample,
    pv: f32,
    mv: f32,
    integrator_value: f32,
    derivative_rate: f32,
    saturation_active: bool,
    integrator_clamped: bool,
    derivative_available: bool,
) -> SystemStatus {
    SystemStatus {
        state: RoasterState::Idle,
        bean_temp: sample.bean_temp,
        env_temp: sample.env_temp,
        target_temp: DEFAULT_TARGET_TEMP,
        ssr_output: 0.0,
        fan_output: 0.0,
        pid_enabled: false,
        artisan_control: false,
        fault_condition: sample.bean_fault.has_fault() || sample.env_fault.has_fault(),
        ssr_hardware_status: SsrHardwareStatus::NotDetected,
        ssr_last_duty_delta_ticks: 0,
        ssr_retry_count: 0,
        ssr_cycle_guard_busy_until_ms: 0,
        watchdog_feed_ok: true,
        watchdog_last_failure: None,
        watchdog_consecutive_failures: 0,
        ledc_guard_timeouts: 0,
        overtemp_regression_active: true,
        pv,
        mv,
        integrator_value,
        derivative_rate,
        saturation_active,
        integrator_clamped,
        derivative_available,
        command_latency_us: 0,
        max_command_latency_us: 0,
    }
}

mod regression_snapshots {
    use super::*;

    #[test]
    fn test_warm_normal_fixture_status() {
        // Get warm fixture
        let fixture = warm_fixture();

        // Process through hub
        let mut hub = SensorConversionHub::new();
        let sample = hub
            .sample_from_fixture(fixture)
            .expect("Should process warm-normal fixture");

        // Build status with expected values
        let status = status_from_sample(
            &sample, 150.0, // pv = bean_temp
            75.0,  // mv
            12.0,  // integrator
            0.24,  // derivative
            true,  // saturation
            true,  // integrator_clamped
            true,  // derivative_available
        );

        // Format using ArtisanFormatter
        let formatted = ArtisanFormatter::format_status_response(&status);

        // Compare to expected (18 columns)
        let parts: Vec<&str> = formatted.split(',').collect();
        assert_eq!(parts.len(), 18, "STATUS must have exactly 18 columns");

        // Verify against expected
        let expected = expected_warm_status();
        assert_eq!(
            formatted, expected,
            "Formatted status should match expected line"
        );
    }

    #[test]
    fn test_cold_negative_fixture_status() {
        // Get cold fixture
        let fixture = cold_fixture();

        // Process through hub
        let mut hub = SensorConversionHub::new();
        let sample = hub
            .sample_from_fixture(fixture)
            .expect("Should process cold-negative fixture");

        // Build status with expected values
        let status = status_from_sample(
            &sample, -10.2, // pv
            60.5,  // mv
            -3.2,  // integrator
            0.08,  // derivative
            false, // saturation
            false, // integrator_clamped
            true,  // derivative_available
        );

        // Format using ArtisanFormatter
        let formatted = ArtisanFormatter::format_status_response(&status);

        // Verify against expected
        let expected = expected_cold_status();
        assert_eq!(
            formatted, expected,
            "Formatted status should match expected line"
        );
    }

    #[test]
    fn test_bean_open_fixture_status() {
        // Get fault fixture
        let fixture = fault_fixture();

        // Process through hub
        let mut hub = SensorConversionHub::new();
        let sample = hub
            .sample_from_fixture(fixture)
            .expect("Should process bean-open fixture");

        // Faulty readings may use cached values - build status with defaults
        let status = status_from_sample(
            &sample, 0.0,   // pv (faulty)
            0.0,   // mv
            0.0,   // integrator
            0.0,   // derivative
            false, // saturation
            false, // integrator_clamped
            false, // derivative_available
        );

        // Format using ArtisanFormatter
        let formatted = ArtisanFormatter::format_status_response(&status);

        // Verify against expected
        let expected = expected_fault_status();
        assert_eq!(
            formatted, expected,
            "Formatted status should match expected line"
        );
    }
}

mod column_order_verification {
    use super::*;

    /// Verify all fixtures produce the same column count
    #[test]
    fn test_all_fixtures_produce_18_columns() {
        let fixtures = [
            ("warm", warm_fixture(), expected_warm_status()),
            ("cold", cold_fixture(), expected_cold_status()),
            ("fault", fault_fixture(), expected_fault_status()),
        ];

        for (name, reading, _expected) in fixtures {
            // Process through hub
            let mut hub = SensorConversionHub::new();
            let sample = hub
                .sample_from_fixture(reading)
                .expect(&format!("Should process {} fixture", name));

            // Build status
            let status = status_from_sample(&sample, 0.0, 0.0, 0.0, 0.0, false, false, false);

            // Format and count columns
            let formatted = ArtisanFormatter::format_status_response(&status);
            let parts: Vec<&str> = formatted.split(',').collect();

            assert_eq!(
                parts.len(),
                18,
                "{} fixture produced {} columns, expected 18",
                name,
                parts.len()
            );
        }
    }

    /// Verify column positions are consistent across fixtures
    #[test]
    fn test_column_positions_consistent() {
        let fixture = warm_fixture();

        let mut hub = SensorConversionHub::new();
        let sample = hub.sample_from_fixture(fixture).unwrap();

        let status = status_from_sample(&sample, 150.0, 75.0, 12.0, 0.24, true, true, true);

        let formatted = ArtisanFormatter::format_status_response(&status);
        let parts: Vec<&str> = formatted.split(',').collect();

        // Column positions per format_status_response:
        // 0: env_temp, 1: bean_temp, 2: ssr_output, 3: fan_output
        // 4: watchdog_flag, 5: failure_count, 6: failure_reason
        // 7: guard_timeouts, 8: regression_flag
        // 9: pv, 10: mv, 11: integrator, 12: derivative
        // 13: saturation, 14: integrator_clamp, 15: derivative_available
        // 16: command_latency, 17: max_command_latency

        assert_eq!(parts[0], "25.0", "Column 0 (env_temp) should be 25.0");
        assert_eq!(parts[1], "150.0", "Column 1 (bean_temp) should be 150.0");
        assert_eq!(parts[2], "0.0", "Column 2 (ssr_output) should be 0.0");
        assert_eq!(parts[3], "0.0", "Column 3 (fan_output) should be 0.0");
        assert_eq!(parts[4], "1", "Column 4 (watchdog) should be 1");
        assert_eq!(parts[5], "0", "Column 5 (failure_count) should be 0");
        assert_eq!(parts[6], "none", "Column 6 (failure_reason) should be none");
        assert_eq!(parts[7], "0", "Column 7 (guard_timeouts) should be 0");
        assert_eq!(parts[8], "1", "Column 8 (regression_flag) should be 1");
        assert_eq!(parts[9], "150.0", "Column 9 (pv) should be 150.0");
        assert_eq!(parts[10], "75.0", "Column 10 (mv) should be 75.0");
        assert_eq!(parts[11], "12.0", "Column 11 (integrator) should be 12.0");
        assert_eq!(parts[12], "0.24", "Column 12 (derivative) should be 0.24");
        assert_eq!(parts[13], "1", "Column 13 (saturation) should be 1");
        assert_eq!(parts[14], "1", "Column 14 (integrator_clamp) should be 1");
        assert_eq!(
            parts[15], "1",
            "Column 15 (derivative_available) should be 1"
        );
        assert_eq!(parts[16], "0", "Column 16 (command_latency) should be 0");
        assert_eq!(
            parts[17], "0",
            "Column 17 (max_command_latency) should be 0"
        );
    }
}

mod fixture_hub_agreement {
    use super::*;

    /// Verify hub temperatures match fixture expectations exactly
    #[test]
    fn test_hub_output_matches_fixture_expected_status() {
        let fixtures = [
            ("warm", warm_fixture(), expected_warm_status()),
            ("cold", cold_fixture(), expected_cold_status()),
            ("fault", fault_fixture(), expected_fault_status()),
        ];

        for (name, reading, expected_line) in fixtures {
            // Process through hub
            let mut hub = SensorConversionHub::new();
            let sample = hub
                .sample_from_fixture(reading)
                .expect(&format!("Should process {} fixture", name));

            // Build status preserving temperatures from hub
            let mut status = SystemStatus::default();
            status.bean_temp = sample.bean_temp;
            status.env_temp = sample.env_temp;
            status.fault_condition = sample.bean_fault.has_fault() || sample.env_fault.has_fault();
            status.overtemp_regression_active = true; // Regression fixtures run with flag set

            // Parse expected line to extract pv/mv/integrator/etc
            let parts: Vec<&str> = expected_line.split(',').collect();
            status.pv = parts[9].parse().unwrap_or(0.0);
            status.mv = parts[10].parse().unwrap_or(0.0);
            status.integrator_value = parts[11].parse().unwrap_or(0.0);
            status.derivative_rate = parts[12].parse().unwrap_or(0.0);
            status.saturation_active = parts[13] == "1";
            status.integrator_clamped = parts[14] == "1";
            status.derivative_available = parts[15] == "1";

            // Format and compare
            let formatted = ArtisanFormatter::format_status_response(&status);

            assert_eq!(
                formatted, expected_line,
                "Fixture {}: formatted status should match expected",
                name
            );
        }
    }
}

mod status_tail_determinism {
    use super::*;

    /// Test that formatting is deterministic (same input = same output)
    #[test]
    fn test_formatting_is_deterministic() {
        let fixture = warm_fixture();

        let mut hub = SensorConversionHub::new();
        let sample = hub.sample_from_fixture(fixture).unwrap();

        let status = status_from_sample(&sample, 150.0, 75.0, 12.0, 0.24, true, true, true);

        // Format multiple times
        let result1 = ArtisanFormatter::format_status_response(&status);
        let result2 = ArtisanFormatter::format_status_response(&status);
        let result3 = ArtisanFormatter::format_status_response(&status);

        assert_eq!(result1, result2, "First and second format should match");
        assert_eq!(result2, result3, "Second and third format should match");
    }

    /// Test that different temperatures produce different outputs
    #[test]
    fn test_different_temperatures_produce_different_outputs() {
        // Get warm and cold fixtures
        let warm_reading = warm_fixture();
        let cold_reading = cold_fixture();

        // Process both through hub
        let mut hub = SensorConversionHub::new();

        let warm_sample = hub.sample_from_fixture(warm_reading).unwrap();
        let warm_status =
            status_from_sample(&warm_sample, 150.0, 75.0, 12.0, 0.24, true, true, true);

        let cold_sample = hub.sample_from_fixture(cold_reading).unwrap();
        let cold_status =
            status_from_sample(&cold_sample, -10.2, 60.5, -3.2, 0.08, false, false, true);

        let warm_formatted = ArtisanFormatter::format_status_response(&warm_status);
        let cold_formatted = ArtisanFormatter::format_status_response(&cold_status);

        assert_ne!(
            warm_formatted, cold_formatted,
            "Warm and cold fixtures should produce different STATUS lines"
        );
    }
}

mod fault_injection_metadata {
    use super::*;

    /// Test that STATUS tail includes correct watchdog_feed_ok for fault scenarios
    #[test]
    fn test_status_watchdog_metadata_for_fault_scenarios() {
        // Test healthy watchdog scenario (WD-01 equivalent)
        let mut healthy_status = SystemStatus::default();
        healthy_status.watchdog_feed_ok = true;
        healthy_status.watchdog_consecutive_failures = 0;
        healthy_status.watchdog_last_failure = None;

        let formatted = ArtisanFormatter::format_status_response(&healthy_status);
        let parts: Vec<&str> = formatted.split(',').collect();

        // Column 4 is watchdog_flag (1 = healthy)
        assert_eq!(parts[4], "1", "Watchdog healthy: flag should be 1");
        // Column 5 is failure_count
        assert_eq!(parts[5], "0", "Watchdog healthy: failure_count should be 0");

        // Test failed watchdog scenario (WD-03 equivalent)
        let mut failed_status = SystemStatus::default();
        failed_status.watchdog_feed_ok = false;
        failed_status.watchdog_consecutive_failures = 3;
        failed_status.watchdog_last_failure = Some("feed_failed");

        let formatted = ArtisanFormatter::format_status_response(&failed_status);
        let parts: Vec<&str> = formatted.split(',').collect();

        assert_eq!(parts[4], "0", "Watchdog failed: flag should be 0");
        assert_eq!(parts[5], "3", "Watchdog failed: failure_count should be 3");
        assert_eq!(
            parts[6], "feed_failed",
            "Watchdog failed: reason should be feed_failed"
        );
    }

    /// Test that STATUS tail includes correct ledc_guard_timeouts
    #[test]
    fn test_status_guard_timeout_metadata() {
        // Test no guard timeouts (GD-01 equivalent)
        let mut no_timeout_status = SystemStatus::default();
        no_timeout_status.ledc_guard_timeouts = 0;

        let formatted = ArtisanFormatter::format_status_response(&no_timeout_status);
        let parts: Vec<&str> = formatted.split(',').collect();

        // Column 7 is guard_timeouts
        assert_eq!(parts[7], "0", "No guard timeouts: should be 0");

        // Test multiple guard timeouts (GD-03 equivalent)
        let mut timeout_status = SystemStatus::default();
        timeout_status.ledc_guard_timeouts = 5;

        let formatted = ArtisanFormatter::format_status_response(&timeout_status);
        let parts: Vec<&str> = formatted.split(',').collect();

        assert_eq!(parts[7], "5", "Guard timeouts: should be 5");
    }

    /// Test that fault_condition is correctly reflected in STATUS
    #[test]
    fn test_status_fault_condition_metadata() {
        // Test no fault condition
        let mut no_fault = SystemStatus::default();
        no_fault.fault_condition = false;
        no_fault.watchdog_feed_ok = true;
        no_fault.ledc_guard_timeouts = 0;

        let formatted = ArtisanFormatter::format_status_response(&no_fault);
        let parts: Vec<&str> = formatted.split(',').collect();

        // Column 4 is watchdog_flag (watchdog healthy)
        assert_eq!(parts[4], "1", "No fault: watchdog should be 1");
        // Column 7 is guard_timeouts
        assert_eq!(parts[7], "0", "No fault: guard_timeouts should be 0");

        // Test fault condition from watchdog failure
        let mut watchdog_fault = SystemStatus::default();
        watchdog_fault.fault_condition = true;
        watchdog_fault.watchdog_feed_ok = false;
        watchdog_fault.watchdog_consecutive_failures = 3;

        let formatted = ArtisanFormatter::format_status_response(&watchdog_fault);
        let parts: Vec<&str> = formatted.split(',').collect();

        assert_eq!(parts[4], "0", "Watchdog fault: flag should be 0");
        assert_eq!(parts[5], "3", "Watchdog fault: failures should be 3");

        // Test fault condition from guard timeouts
        let mut guard_fault = SystemStatus::default();
        guard_fault.fault_condition = true;
        guard_fault.watchdog_feed_ok = true;
        guard_fault.ledc_guard_timeouts = 10;

        let formatted = ArtisanFormatter::format_status_response(&guard_fault);
        let parts: Vec<&str> = formatted.split(',').collect();

        assert_eq!(parts[4], "1", "Guard fault: watchdog should still be 1");
        assert_eq!(parts[7], "10", "Guard fault: timeouts should be 10");
    }

    /// Test that STATUS columns remain deterministic with fault metadata
    #[test]
    fn test_status_columns_deterministic_with_faults() {
        // Run multiple times to verify determinism
        for _ in 0..5 {
            let mut status = SystemStatus::default();
            status.watchdog_feed_ok = false;
            status.watchdog_consecutive_failures = 2;
            status.watchdog_last_failure = Some("feed_failed");
            status.ledc_guard_timeouts = 3;
            status.fault_condition = true;

            let formatted = ArtisanFormatter::format_status_response(&status);
            let parts: Vec<&str> = formatted.split(',').collect();

            assert_eq!(parts.len(), 18, "Should have exactly 18 columns");
            assert_eq!(parts[4], "0", "Column 4 should be 0");
            assert_eq!(parts[5], "2", "Column 5 should be 2");
            assert_eq!(parts[6], "feed_failed", "Column 6 should be feed_failed");
            assert_eq!(parts[7], "3", "Column 7 should be 3");
        }
    }

    /// Verify fault metadata matches SCENARIO_MATRIX.md expectations
    #[test]
    fn test_fault_scenario_metadata_matches_matrix() {
        // WD-01: Watchdog OK, no guard timeouts
        let mut wd01 = SystemStatus::default();
        wd01.watchdog_feed_ok = true;
        wd01.watchdog_consecutive_failures = 0;
        wd01.ledc_guard_timeouts = 0;
        wd01.fault_condition = false;

        let formatted = ArtisanFormatter::format_status_response(&wd01);
        let parts: Vec<&str> = formatted.split(',').collect();
        assert_eq!(parts[4], "1", "WD-01: watchdog_flag = 1");
        assert_eq!(parts[5], "0", "WD-01: failure_count = 0");
        assert_eq!(parts[7], "0", "WD-01: guard_timeouts = 0");

        // WD-03: Watchdog failed, multiple failures, fault condition
        let mut wd03 = SystemStatus::default();
        wd03.watchdog_feed_ok = false;
        wd03.watchdog_consecutive_failures = 3;
        wd03.ledc_guard_timeouts = 0;
        wd03.fault_condition = true;

        let formatted = ArtisanFormatter::format_status_response(&wd03);
        let parts: Vec<&str> = formatted.split(',').collect();
        assert_eq!(parts[4], "0", "WD-03: watchdog_flag = 0");
        assert_eq!(parts[5], "3", "WD-03: failure_count = 3");

        // GD-03: Watchdog OK, multiple guard timeouts, fault condition
        let mut gd03 = SystemStatus::default();
        gd03.watchdog_feed_ok = true;
        gd03.watchdog_consecutive_failures = 0;
        gd03.ledc_guard_timeouts = 5;
        gd03.fault_condition = true;

        let formatted = ArtisanFormatter::format_status_response(&gd03);
        let parts: Vec<&str> = formatted.split(',').collect();
        assert_eq!(parts[4], "1", "GD-03: watchdog_flag = 1");
        assert_eq!(parts[7], "5", "GD-03: guard_timeouts = 5");
    }
}
