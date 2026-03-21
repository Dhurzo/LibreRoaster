//! Mock hardware tests - Fan with realistic PWM simulation
//! Simula comportamiento completo de fan con historial de velocidad

#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use libreroaster::control::{traits::Fan, RoasterError};
use std::sync::{Arc, Mutex};

/// Mock fan with detailed tracking
pub struct MockFan {
    pub current_speed: Arc<Mutex<f32>>,
    pub speed_history: Arc<Mutex<Vec<(u32, f32)>>>, // (timestamp_ms, speed)
    pub speed_change_count: Arc<Mutex<usize>>,
    pub on_speed_change: Arc<Mutex<Option<Box<dyn Fn(f32) + Send>>>>,
}

impl MockFan {
    pub fn new() -> Self {
        Self {
            current_speed: Arc::new(Mutex::new(0.0)),
            speed_history: Arc::new(Mutex::new(vec![])),
            speed_change_count: Arc::new(Mutex::new(0)),
            on_speed_change: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_callback<F: Fn(f32) + Send + 'static>(callback: F) -> Self {
        let mut fan = Self::new();
        *fan.on_speed_change.lock().unwrap() = Some(Box::new(callback));
        fan
    }

    pub fn get_speed_history(&self) -> Vec<(u32, f32)> {
        self.speed_history.lock().unwrap().clone()
    }

    pub fn get_change_count(&self) -> usize {
        *self.speed_change_count.lock().unwrap()
    }

    pub fn clear_history(&self) {
        self.speed_history.lock().unwrap().clear();
    }
}

impl Fan for MockFan {
    fn set_speed(&mut self, speed: f32) -> Result<(), RoasterError> {
        let clamped = speed.clamp(0.0, 100.0);
        let current = *self.current_speed.lock().unwrap();

        // Actualizar velocidad solo si cambió
        if clamped != current {
            *self.current_speed.lock().unwrap() = clamped;
            *self.speed_change_count.lock().unwrap() += 1;

            // Registrar en historial con timestamp
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u32;
            self.speed_history
                .lock()
                .unwrap()
                .push((timestamp, clamped));

            // Ejecutar callback si existe
            if let Some(ref callback) = *self.on_speed_change.lock().unwrap() {
                callback(clamped);
            }
        }

        Ok(())
    }

    fn get_speed(&self) -> f32 {
        *self.current_speed.lock().unwrap()
    }
}

/// Test: Basic speed setting
#[test]
fn test_mock_fan_basic_speed() {
    let mut fan = MockFan::new();

    assert_eq!(fan.get_speed(), 0.0);

    fan.set_speed(50.0).unwrap();
    assert_eq!(fan.get_speed(), 50.0);

    fan.set_speed(75.0).unwrap();
    assert_eq!(fan.get_speed(), 75.0);
}

/// Test: Clamping to 0-100%
#[test]
fn test_mock_fan_clamping() {
    let mut fan = MockFan::new();

    // Fuera de rango (arriba)
    fan.set_speed(150.0).unwrap();
    assert_eq!(fan.get_speed(), 100.0);

    // Fuera de rango (abajo)
    fan.set_speed(-20.0).unwrap();
    assert_eq!(fan.get_speed(), 0.0);

    // En rango
    fan.set_speed(50.0).unwrap();
    assert_eq!(fan.get_speed(), 50.0);
}

/// Test: Speed change counting
#[test]
fn test_mock_fan_change_count() {
    let mut fan = MockFan::new();

    assert_eq!(fan.get_change_count(), 0);

    fan.set_speed(25.0).unwrap();
    assert_eq!(fan.get_change_count(), 1);

    fan.set_speed(50.0).unwrap(); // Mismo valor, no debe contar
    assert_eq!(fan.get_change_count(), 1);

    fan.set_speed(75.0).unwrap();
    assert_eq!(fan.get_change_count(), 2);
}

/// Test: Speed history tracking
#[test]
fn test_mock_fan_speed_history() {
    let mut fan = MockFan::new();

    fan.set_speed(25.0).unwrap();
    fan.set_speed(50.0).unwrap();
    fan.set_speed(75.0).unwrap();

    let history = fan.get_speed_history();
    assert_eq!(history.len(), 2); // Solo cambios, no duplicados
    assert_eq!(history[0].1, 25.0);
    assert_eq!(history[1].1, 50.0);
}

/// Test: Callback invocation
#[test]
fn test_mock_fan_callback() {
    let callback_invoked = Arc::new(Mutex::new(vec![]));

    let callback = {
        let invoked = callback_invoked.clone();
        Box::new(move |speed: f32| {
            invoked.lock().unwrap().push(speed);
        })
    };

    let mut fan = MockFan::with_callback(callback);

    fan.set_speed(50.0).unwrap();
    fan.set_speed(75.0).unwrap();

    let invoked_calls = callback_invoked.lock().unwrap();
    assert_eq!(invoked_calls.len(), 2);
    assert_eq!(invoked_calls[0], 50.0);
    assert_eq!(invoked_calls[1], 75.0);
}

/// Test: No changes don't trigger history
#[test]
fn test_mock_fan_no_duplicate_changes() {
    let mut fan = MockFan::new();

    fan.set_speed(50.0).unwrap();
    fan.set_speed(50.0).unwrap(); // Mismo valor
    fan.set_speed(50.0).unwrap(); // Mismo valor

    let history = fan.get_speed_history();
    assert_eq!(history.len(), 1); // Solo el primero
    assert_eq!(history[0].1, 50.0);
}
