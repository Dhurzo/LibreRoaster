extern crate alloc;

use crate::config::constants::DEFAULT_TARGET_TEMP;
use crate::config::{RoasterState, SsrHardwareStatus, SystemStatus};
use crate::hardware::sensors::conversion::FixtureReading;
use alloc::vec::Vec;
use embedded_hal_mock::eh1::spi::Transaction;

type StatusBuilder = fn() -> SystemStatus;
type TransactionBuilder = fn() -> Vec<Transaction<'static>>;

pub struct RegressionFixture {
    pub name: &'static str,
    pub reading: FixtureReading,
    pub status_builder: StatusBuilder,
    pub expected_status_line: &'static str,
    pub bean_transactions: TransactionBuilder,
    pub env_transactions: TransactionBuilder,
}

pub fn canonical_fixtures() -> &'static [RegressionFixture] {
    &FIXTURES
}

fn base_status() -> SystemStatus {
    SystemStatus {
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
    }
}

fn status_normal() -> SystemStatus {
    let mut status = base_status();
    status.mv = 75.0;
    status.integrator_value = 12.0;
    status.derivative_rate = 0.24;
    status.saturation_active = true;
    status.integrator_clamped = true;
    status.derivative_available = true;
    status
}

fn status_cold() -> SystemStatus {
    let mut status = base_status();
    status.mv = 60.5;
    status.integrator_value = -3.2;
    status.derivative_rate = 0.08;
    status.saturation_active = false;
    status.integrator_clamped = false;
    status.derivative_available = true;
    status
}

fn status_faulty() -> SystemStatus {
    let mut status = base_status();
    status.mv = 0.0;
    status.integrator_value = 0.0;
    status.derivative_rate = 0.0;
    status.saturation_active = false;
    status.integrator_clamped = false;
    status.derivative_available = false;
    status
}

fn write_transaction(data: &[u8]) -> Transaction<'static> {
    Transaction::<u8>::write_vec(Vec::from(data))
}

fn read_transaction(data: &[u8]) -> Transaction<'static> {
    Transaction::<u8>::read_vec(Vec::from(data))
}

fn sensor_sequence(adc: [u8; 3], fault: u8) -> Vec<Transaction<'static>> {
    let mut sequence = Vec::new();
    sequence.push(Transaction::<u8>::transaction_start());
    sequence.push(write_transaction(&[0x80, 0x80]));
    sequence.push(Transaction::<u8>::transaction_end());

    sequence.push(Transaction::<u8>::transaction_start());
    sequence.push(write_transaction(&[0x8C, 0x8C, 0x8C]));
    sequence.push(read_transaction(&adc));
    sequence.push(Transaction::<u8>::transaction_end());

    sequence.push(Transaction::<u8>::transaction_start());
    sequence.push(write_transaction(&[0x8F, 0x00]));
    sequence.push(Transaction::<u8>::read(fault));
    sequence.push(Transaction::<u8>::transaction_end());
    sequence
}

fn warm_bean_transactions() -> Vec<Transaction<'static>> {
    sensor_sequence([0x00, 0x4B, 0x00], 0x00)
}

fn warm_env_transactions() -> Vec<Transaction<'static>> {
    sensor_sequence([0x00, 0x0C, 0x80], 0x00)
}

fn cold_bean_transactions() -> Vec<Transaction<'static>> {
    sensor_sequence([0xFF, 0xFA, 0xE0], 0x00)
}

fn cold_env_transactions() -> Vec<Transaction<'static>> {
    sensor_sequence([0x00, 0x00, 0x00], 0x00)
}

fn fault_bean_transactions() -> Vec<Transaction<'static>> {
    sensor_sequence([0x00, 0x00, 0x00], 0x01)
}

fn fault_env_transactions() -> Vec<Transaction<'static>> {
    sensor_sequence([0x00, 0x32, 0x00], 0x04)
}

static FIXTURES: [RegressionFixture; 3] = [
    RegressionFixture {
        name: "warm-normal",
        reading: FixtureReading {
            bean_adc: [0x00, 0x4B, 0x00],
            bean_fault: 0x00,
            env_adc: [0x00, 0x0C, 0x80],
            env_fault: 0x00,
        },
        status_builder: status_normal,
        expected_status_line: "25.0,150.0,0.0,0.0,1,0,none,0,1,150.0,75.0,12.0,0.24,1,1,1",
        bean_transactions: warm_bean_transactions,
        env_transactions: warm_env_transactions,
    },
    RegressionFixture {
        name: "cold-negative",
        reading: FixtureReading {
            bean_adc: [0xFF, 0xFA, 0xE0],
            bean_fault: 0x00,
            env_adc: [0x00, 0x00, 0x00],
            env_fault: 0x00,
        },
        status_builder: status_cold,
        expected_status_line: "0.0,-10.2,0.0,0.0,1,0,none,0,1,-10.2,60.5,-3.2,0.08,0,0,1",
        bean_transactions: cold_bean_transactions,
        env_transactions: cold_env_transactions,
    },
    RegressionFixture {
        name: "bean-open",
        reading: FixtureReading {
            bean_adc: [0x00, 0x00, 0x00],
            bean_fault: 0x01,
            env_adc: [0x00, 0x32, 0x00],
            env_fault: 0x04,
        },
        status_builder: status_faulty,
        expected_status_line: "100.0,0.0,0.0,0.0,1,0,none,0,1,0.0,0.0,0.0,0.00,0,0,0",
        bean_transactions: fault_bean_transactions,
        env_transactions: fault_env_transactions,
    },
];
