//! Mock hardware tests - SSR with realistic PWM simulation
//! Simula comportamiento completo de SSR con heat detection y callbacks

#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use libreroaster::config::SsrHardwareStatus;
use libreroaster::control::{traits::Heater, RoasterError};
use std::sync::{Arc, Mutex};

/// Mock SSR with detailed tracking
pub struct MockSsr {
    pub current_duty: Arc<Mutex<f32>>,
    pub power_on_count: Arc<Mutex<u32>>,
    pub power_off_count: Arc<Mutex<u32>>,
    pub duty_history: Arc<Mutex<Vec<(u32, f32)>>>, // (timestamp_ms, duty)
    pub heat_detected: Arc<Mutex<bool>>,
    pub on_power_change: Arc<Mutex<Option<Box<dyn Fn(f32) + Send>>>>,
}

impl MockSsr {
    pub fn new() -> Self {
        Self {
            current_duty: Arc::new(Mutex::new(0.0)),
            power_on_count: Arc::new(Mutex::new(0)),
            power_off_count: Arc::new(Mutex::new(0)),
            duty_history: Arc::new(Mutex::new(vec![])),
            heat_detected: Arc::new(Mutex::new(false)),
            on_power_change: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_heat_detected(&self, detected: bool) {
        *self.heat_detected.lock().unwrap() = detected;
    }

    pub fn with_callback<F: Fn(f32) + Send + 'static>(callback: F) -> Self {
        let mut ssr = Self::new();
        *ssr.on_power_change.lock().unwrap() = Some(Box::new(callback));
        ssr
    }

    pub fn get_duty_history(&self) -> Vec<(u32, f32)> {
        self.duty_history.lock().unwrap().clone()
    }

    pub fn clear_history(&self) {
        self.duty_history.lock().unwrap().clear();
    }
}

impl Heater for MockSsr {
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError> {
        let clamped = duty.clamp(0.0, 100.0);
        let prev_duty = *self.current_duty.lock().unwrap();

        // Actualizar duty
        *self.current_duty.lock().unwrap() = clamped;

        // Registrar en historial con timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u32;
        self.duty_history.lock().unwrap().push((timestamp, clamped));

        // Contar transiciones
        if clamped > 0.0 && prev_duty == 0.0 {
            *self.power_on_count.lock().unwrap() += 1;
        } else if clamped == 0.0 && prev_duty > 0.0 {
            *self.power_off_count.lock().unwrap() += 1;
        }

        // Ejecutar callback si existe
        if let Some(ref callback) = *self.on_power_change.lock().unwrap() {
            callback(clamped);
        }

        Ok(())
    }

    fn get_status(&self) -> SsrHardwareStatus {
        let detected = *self.heat_detected.lock().unwrap();
        if detected {
            SsrHardwareStatus::Available
        } else {
            SsrHardwareStatus::NotDetected
        }
    }

    fn last_duty_delta_ticks(&self) -> i16 {
        0
    }

    fn last_retry_count(&self) -> u8 {
        0
    }
}

/// Test: Power transitions
#[test]
fn test_mock_ssr_power_transitions() {
    let mut ssr = MockSsr::new();

    // Encender
    ssr.set_power(50.0).unwrap();
    assert_eq!(*ssr.current_duty.lock().unwrap(), 50.0);
    assert_eq!(*ssr.power_on_count.lock().unwrap(), 1);

    // Apagar
    ssr.set_power(0.0).unwrap();
    assert_eq!(*ssr.current_duty.lock().unwrap(), 0.0);
    assert_eq!(*ssr.power_off_count.lock().unwrap(), 1);
}

/// Test: Clamping a 0-100%
#[test]
fn test_mock_ssr_clamping() {
    let mut ssr = MockSsr::new();

    // Valor fuera de rango (arriba)
    ssr.set_power(150.0).unwrap();
    assert_eq!(*ssr.current_duty.lock().unwrap(), 100.0);

    // Valor fuera de rango (abajo)
    ssr.set_power(-20.0).unwrap();
    assert_eq!(*ssr.current_duty.lock().unwrap(), 0.0);

    // Valor en rango
    ssr.set_power(50.0).unwrap();
    assert_eq!(*ssr.current_duty.lock().unwrap(), 50.0);
}

/// Test: Heat detection status
#[test]
fn test_mock_ssr_heat_detection() {
    let mut ssr = MockSsr::new();

    // Sin detección
    ssr.set_heat_detected(false);
    assert_eq!(ssr.get_status(), SsrHardwareStatus::NotDetected);

    // Con detección
    ssr.set_heat_detected(true);
    assert_eq!(ssr.get_status(), SsrHardwareStatus::Available);
}

/// Test: Duty history tracking
#[test]
fn test_mock_ssr_duty_history() {
    let mut ssr = MockSsr::new();

    ssr.set_power(25.0).unwrap();
    ssr.set_power(50.0).unwrap();
    ssr.set_power(75.0).unwrap();

    let history = ssr.get_duty_history();
    assert_eq!(history.len(), 3);
    assert_eq!(history[0].1, 25.0);
    assert_eq!(history[1].1, 50.0);
    assert_eq!(history[2].1, 75.0);
}

/// Test: Callback invocation
#[test]
fn test_mock_ssr_callback() {
    let callback_invoked = Arc::new(Mutex::new(vec![]));

    let callback = {
        let invoked = callback_invoked.clone();
        Box::new(move |duty: f32| {
            invoked.lock().unwrap().push(duty);
        })
    };

    let mut ssr = MockSsr::with_callback(callback);

    ssr.set_power(50.0).unwrap();
    ssr.set_power(75.0).unwrap();

    let invoked_calls = callback_invoked.lock().unwrap();
    assert_eq!(invoked_calls.len(), 2);
    assert_eq!(invoked_calls[0], 50.0);
    assert_eq!(invoked_calls[1], 75.0);
}

/// Test: No callbacks by default
#[test]
fn test_mock_ssr_no_callback_default() {
    let ssr = MockSsr::new();

    // Debe funcionar sin callback (no panic)
    ssr.set_power(50.0).unwrap();
    assert_eq!(*ssr.current_duty.lock().unwrap(), 50.0);
}
