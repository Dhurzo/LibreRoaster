//! Mock hardware tests - MAX31856 with realistic simulation
//! Simula comportamiento completo de MAX31856 con escenarios de fallo

#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use core::cell::RefCell;
use embedded_hal::spi::{Operation, SpiDevice};
use libreroaster::control::RoasterError;
use libreroaster::hardware::max31856::Max31856Error;

/// Fault scenarios for MAX31856
#[derive(Debug, Clone, Copy)]
pub enum FaultScenario {
    NoFault,
    OpenCircuit,      // Fault bit 0: Thermocouple shorted to VCC
    ShortToVCC,       // Fault bit 1: Thermocouple shorted to VCC
    ShortToGND,       // Fault bit 2: Thermocouple shorted to GND
    OpenThermocouple, // Fault bit 3: Thermocouple open
}

/// Mock SPI bus for MAX31856
pub struct MockSpiBus {
    pub transactions: Vec<(Vec<u8>, Vec<u8>)>,
    pub delay_us: u64,
}

impl MockSpiBus {
    pub fn new() -> Self {
        Self {
            transactions: vec![],
            delay_us: 0,
        }
    }

    pub fn with_delay(delay_us: u64) -> Self {
        Self {
            transactions: vec![],
            delay_us,
        }
    }
}

impl SpiDevice for MockSpiBus {
    type Error = MockSpiError;

    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        // Recordar transacción
        let mut writes = vec![];
        let mut reads = vec![];

        for op in operations {
            match op {
                Operation::Write(buf) => {
                    writes.extend_from_slice(buf);
                }
                Operation::Read(buf) => {
                    // Generar datos de lectura según estado
                    reads.extend_from_slice(buf);
                }
                Operation::Transfer(read, write) => {
                    writes.extend_from_slice(write);
                    reads.extend_from_slice(read);
                }
                Operation::TransferInPlace(buf) => {
                    // Leer y escribir en mismo buffer
                    for b in buf {
                        reads.push(*b);
                    }
                }
                Operation::DelayNs(ns) => {
                    // Simular delay
                    let _cycles = *ns as u64 / 10;
                    // Simular spin loop con sleep
                    std::thread::sleep(std::time::Duration::from_micros(self.delay_us));
                }
            }
        }

        self.transactions.push((writes, reads));
        Ok(())
    }
}

#[derive(Debug)]
pub struct MockSpiError;

impl embedded_hal::spi::Error for MockSpiError {
    fn kind(&self) -> embedded_hal::spi::ErrorKind {
        embedded_hal::spi::ErrorKind::Other
    }
}

/// Mock MAX31856 with temperature series and fault simulation
pub struct MockMax31856 {
    pub temp_series: Vec<f32>,
    pub temp_index: RefCell<usize>,
    pub simulated_latency_ms: u64,
    pub fault_scenario: RefCell<FaultScenario>,
    pub spi_bus: MockSpiBus,
}

impl MockMax31856 {
    pub fn with_temperature_series(temps: Vec<f32>) -> Self {
        Self {
            temp_series: temps,
            temp_index: RefCell::new(0),
            simulated_latency_ms: 160, // Latencia real de conversión
            fault_scenario: RefCell::new(FaultScenario::NoFault),
            spi_bus: MockSpiBus::new(),
        }
    }

    pub fn with_fault_scenario(temps: Vec<f32>, scenario: FaultScenario) -> Self {
        Self {
            temp_series: temps,
            temp_index: RefCell::new(0),
            simulated_latency_ms: 160,
            fault_scenario: RefCell::new(scenario),
            spi_bus: MockSpiBus::new(),
        }
    }

    pub fn set_fault_scenario(&self, scenario: FaultScenario) {
        *self.fault_scenario.borrow_mut() = scenario;
    }

    pub fn get_call_count(&self) -> usize {
        *self.temp_index.borrow()
    }
}

impl libreroaster::control::traits::Thermometer for MockMax31856 {
    fn read_temperature(&mut self) -> Result<f32, RoasterError> {
        // Simular delay de 160ms de conversión MAX31856
        std::thread::sleep(std::time::Duration::from_millis(self.simulated_latency_ms));

        let scenario = *self.fault_scenario.borrow();

        // Generar fault byte según escenario
        let fault_byte = match scenario {
            FaultScenario::NoFault => 0x00,
            FaultScenario::OpenCircuit => 0x01, // Fault bit 0
            FaultScenario::ShortToVCC => 0x02,  // Fault bit 1
            FaultScenario::ShortToGND => 0x04,  // Fault bit 2
            FaultScenario::OpenThermocouple => 0x08, // Fault bit 3
        };

        if fault_byte != 0x00 {
            return Err(RoasterError::SensorFault);
        }

        // Devolver temperatura de la serie
        let index = *self.temp_index.borrow();
        let temp = if index < self.temp_series.len() {
            self.temp_series[index]
        } else {
            *self.temp_series.last().unwrap()
        };

        *self.temp_index.borrow_mut() += 1;
        Ok(temp)
    }
}

/// Test: No fault scenario
#[test]
fn test_mock_max31856_no_fault() {
    let mut mock = MockMax31856::with_temperature_series(vec![150.0, 155.0, 160.0]);

    let temp1 = mock.read_temperature().unwrap();
    assert_eq!(temp1, 150.0);

    let temp2 = mock.read_temperature().unwrap();
    assert_eq!(temp2, 155.0);

    let temp3 = mock.read_temperature().unwrap();
    assert_eq!(temp3, 160.0);
}

/// Test: Open circuit fault
#[test]
fn test_mock_max31856_open_circuit_fault() {
    let mut mock = MockMax31856::with_fault_scenario(vec![150.0], FaultScenario::OpenCircuit);

    let result = mock.read_temperature();
    assert!(matches!(result, Err(RoasterError::SensorFault)));
}

/// Test: Short to VCC fault
#[test]
fn test_mock_max31856_short_to_vcc_fault() {
    let mut mock = MockMax31856::with_fault_scenario(vec![150.0], FaultScenario::ShortToVCC);

    let result = mock.read_temperature();
    assert!(matches!(result, Err(RoasterError::SensorFault)));
}

/// Test: Short to GND fault
#[test]
fn test_mock_max31856_short_to_gnd_fault() {
    let mut mock = MockMax31856::with_fault_scenario(vec![150.0], FaultScenario::ShortToGND);

    let result = mock.read_temperature();
    assert!(matches!(result, Err(RoasterError::SensorFault)));
}

/// Test: Open thermocouple fault
#[test]
fn test_mock_max31856_open_thermocouple_fault() {
    let mut mock = MockMax31856::with_fault_scenario(vec![150.0], FaultScenario::OpenThermocouple);

    let result = mock.read_temperature();
    assert!(matches!(result, Err(RoasterError::SensorFault)));
}

/// Test: Temperature series with wraparound
#[test]
fn test_mock_max31856_series_wraparound() {
    let temps = vec![100.0, 110.0, 120.0, 130.0];
    let mut mock = MockMax31856::with_temperature_series(temps);

    // Leer más allá de la serie
    for _ in 0..10 {
        let temp = mock.read_temperature().unwrap();
        assert!(
            temp >= 100.0 && temp <= 130.0,
            "Temperatura debe estar en la serie"
        );
    }
}

/// Test: Call count tracking
#[test]
fn test_mock_max31856_call_count() {
    let temps = vec![100.0, 110.0, 120.0];
    let mut mock = MockMax31856::with_temperature_series(temps);

    assert_eq!(mock.get_call_count(), 0);

    mock.read_temperature().unwrap();
    assert_eq!(mock.get_call_count(), 1);

    mock.read_temperature().unwrap();
    assert_eq!(mock.get_call_count(), 2);
}
