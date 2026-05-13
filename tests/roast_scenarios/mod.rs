//! Roast scenario simulation tests
//! Simulates real roasting phases with realistic temperature curves

#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use embassy_time::Instant;
use libreroaster::common::{StubFan, StubHeater, StubThermometer};
use libreroaster::config::RoasterState;
use libreroaster::control::RoasterControl;
use libreroaster::hardware::sensors::SensorConversionHub;

/// Simulates a realistic heating curve (25°C → 150°C)
pub struct HeatingCurve {
    temps: Vec<f32>,
    intervals_ms: u64,
}

impl HeatingCurve {
    pub fn new() -> Self {
        // Typical exponential heating curve
        // Models system thermal inertia
        let temps = vec![
            25.0, 30.0, 38.0, 48.0, 60.0, // 0-4s: initial fast ramp
            74.0, 88.0, 104.0, 120.0, 138.0, // 5-9s: ramp decelerates
            150.0, // 10s: target reached
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

/// Simulates a realistic roasting curve (150°C → 220°C)
pub struct RoastingCurve {
    temps: Vec<f32>,
    intervals_ms: u64,
}

impl RoastingCurve {
    pub fn new() -> Self {
        // Curve with progressive stabilization
        // Models real roast dynamics with fluctuations
        let temps = vec![
            150.0, // 0s: roast start
            165.0, // 30s: fast ramp
            180.0, // 60s: approaching target
            195.0, // 90s: close to target
            210.0, // 120s: very close to target
            218.0, // 150s: target nearly reached
            220.0, // 180s: target reached
            219.0, // 210s: stabilization
            221.0, // 240s: minimal oscillation
            220.0, // 270s: stabilization
            220.0, // 300s: stability
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

/// Simulates a realistic cooling curve (220°C → 50°C)
pub struct CoolingCurve {
    temps: Vec<f32>,
    intervals_ms: u64,
}

impl CoolingCurve {
    pub fn new() -> Self {
        // Exponential cooling curve
        // Models system cooling rate
        let temps = vec![
            220.0, // 0s: immediate STOP
            180.0, // 30s: fast cooling
            140.0, // 60s: medium cooling
            100.0, // 90s: cooling continues
            70.0,  // 120s: low temperature
            50.0,  // 150s: ambient temperature
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

/// Helper to create RoasterControl with configured mocks
pub fn create_test_roaster() -> RoasterControl {
    let heater = Box::new(StubHeater::new());
    let fan = Box::new(StubFan::new());
    let sensor_hub = SensorConversionHub::new();

    RoasterControl::new(heater, fan, sensor_hub).expect("RoasterControl creation should succeed")
}
