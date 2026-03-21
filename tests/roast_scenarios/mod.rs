//! Roast scenario simulation tests
//! Simula fases reales de tueste con curvas de temperatura realistas

#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use embassy_time::Instant;
use libreroaster::common::{StubFan, StubHeater, StubThermometer};
use libreroaster::config::RoasterState;
use libreroaster::control::RoasterControl;
use libreroaster::hardware::sensors::SensorConversionHub;

/// Simula una curva de calentamiento realista (25°C → 150°C)
pub struct HeatingCurve {
    temps: Vec<f32>,
    intervals_ms: u64,
}

impl HeatingCurve {
    pub fn new() -> Self {
        // Curva exponencial típica de calentamiento
        // Modela inercia térmica del sistema
        let temps = vec![
            25.0, 30.0, 38.0, 48.0, 60.0, // 0-4s: rampa rápida inicial
            74.0, 88.0, 104.0, 120.0, 138.0, // 5-9s: rampa se desacelera
            150.0, // 10s: target alcanzado
        ];
        Self {
            temps,
            intervals_ms: 100,
        }
    }

    pub fn get_temp_at_second(&self, second: usize) -> f32 {
        if second < self.temps.len() {
            self.temps[second]
        } else {
            *self.temps.last().unwrap()
        }
    }
}

/// Simula una curva de tueste realista (150°C → 220°C)
pub struct RoastingCurve {
    temps: Vec<f32>,
    intervals_ms: u64,
}

impl RoastingCurve {
    pub fn new() -> Self {
        // Curva con estabilización progresiva
        // Modela la dinámica de tueste real con fluctuaciones
        let temps = vec![
            150.0, // 0s: inicio de tueste
            165.0, // 30s: rampa rápida
            180.0, // 60s: aproximación a target
            195.0, // 90s: cerca de target
            210.0, // 120s: muy cerca de target
            218.0, // 150s: target casi alcanzado
            220.0, // 180s: target alcanzado
            219.0, // 210s: estabilización
            221.0, // 240s: oscilación mínima
            220.0, // 270s: estabilización
            220.0, // 300s: estabilidad
        ];
        Self {
            temps,
            intervals_ms: 30,
        }
    }

    pub fn get_temp_at_second(&self, second: usize) -> f32 {
        let index = (second / 30).min(self.temps.len() - 1);
        self.temps[index]
    }
}

/// Simula una curva de enfriamiento realista (220°C → 50°C)
pub struct CoolingCurve {
    temps: Vec<f32>,
    intervals_ms: u64,
}

impl CoolingCurve {
    pub fn new() -> Self {
        // Curva exponencial de enfriamiento
        // Modela la velocidad de enfriamiento del sistema
        let temps = vec![
            220.0, // 0s: STOP inmediato
            180.0, // 30s: enfriamiento rápido
            140.0, // 60s: enfriamiento medio
            100.0, // 90s: enfriamiento continúa
            70.0,  // 120s: temperatura baja
            50.0,  // 150s: temperatura ambiente
        ];
        Self {
            temps,
            intervals_ms: 30,
        }
    }

    pub fn get_temp_at_second(&self, second: usize) -> f32 {
        let index = (second / 30).min(self.temps.len() - 1);
        self.temps[index]
    }
}

/// Helper para crear RoasterControl con mocks configurados
pub fn create_test_roaster() -> RoasterControl {
    let heater = Box::new(StubHeater::new());
    let fan = Box::new(StubFan::new());
    let sensor_hub = SensorConversionHub::new();

    RoasterControl::new(heater, fan, sensor_hub).expect("RoasterControl creation should succeed")
}
