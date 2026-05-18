//! Synthetic temperature curve generator for testing without real thermocouples.
//!
//! When the `simulated-sensors` feature is enabled, this module provides
//! realistic roast temperature curves that drive the entire control stack
//! (PID, safety, telemetry) without real MAX31856 hardware.
//!
//! ## Usage
//!
//! ```bash
//! cargo build --release --target riscv32imc-unknown-none-elf \
//!   --features "embedded,simulated-sensors"
//! ```
//!
//! ## Curve model
//!
//! A roast curve is modelled as a sequence of (time_secs, bean_temp, env_temp)
//! waypoints. The generator linearly interpolates between consecutive waypoints
//! to produce smooth temperature trajectories at the control-loop cadence (~10 Hz).

use embassy_time::Instant;

/// Maximum number of waypoints in a synthetic curve.
pub const MAX_CURVE_POINTS: usize = 32;

/// A single waypoint in a synthetic roast curve.
#[derive(Debug, Clone, Copy)]
pub struct CurvePoint {
    /// Elapsed time in seconds since roast start.
    pub time_secs: u32,
    /// Bean temperature in °C at this waypoint.
    pub bean_temp: f32,
    /// Environment/exhaust temperature in °C at this waypoint.
    pub env_temp: f32,
}

/// A synthetic roast temperature curve built from interpolated waypoints.
#[derive(Debug, Clone)]
pub struct RoastCurve {
    points: heapless::Vec<CurvePoint, MAX_CURVE_POINTS>,
}

impl Default for RoastCurve {
    fn default() -> Self {
        Self::new()
    }
}

impl RoastCurve {
    /// Create an empty curve. Use [`Self::add_point`] to build it up.
    pub fn new() -> Self {
        Self {
            points: heapless::Vec::new(),
        }
    }

    /// Create the built-in default roast curve: a typical medium roast profile.
    ///
    /// Approximate profile:
    /// - 0–60s:   Charge at 150°C BT / 180°C ET
    /// - 60–240s: Ramp to 190°C BT / 220°C ET (drying phase)
    /// - 240–420s: Ramp to 215°C BT / 240°C ET (Maillard phase)
    /// - 420–540s: Ramp to 225°C BT / 250°C ET (first crack)
    /// - 540–600s: Hold at 225°C BT / 250°C ET (development)
    pub fn default_medium_roast() -> Self {
        let mut curve = Self::new();
        // Initial charge temperatures (ambient ~25°C ramping up)
        curve.add_point(CurvePoint {
            time_secs: 0,
            bean_temp: 25.0,
            env_temp: 25.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 30,
            bean_temp: 80.0,
            env_temp: 100.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 60,
            bean_temp: 120.0,
            env_temp: 150.0,
        });
        // Drying phase
        curve.add_point(CurvePoint {
            time_secs: 120,
            bean_temp: 150.0,
            env_temp: 180.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 180,
            bean_temp: 170.0,
            env_temp: 200.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 240,
            bean_temp: 190.0,
            env_temp: 220.0,
        });
        // Maillard phase
        curve.add_point(CurvePoint {
            time_secs: 300,
            bean_temp: 200.0,
            env_temp: 230.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 360,
            bean_temp: 210.0,
            env_temp: 238.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 420,
            bean_temp: 215.0,
            env_temp: 240.0,
        });
        // First crack
        curve.add_point(CurvePoint {
            time_secs: 480,
            bean_temp: 220.0,
            env_temp: 245.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 540,
            bean_temp: 225.0,
            env_temp: 250.0,
        });
        // Development / hold
        curve.add_point(CurvePoint {
            time_secs: 600,
            bean_temp: 225.0,
            env_temp: 250.0,
        });
        curve
    }

    /// Add a waypoint to the curve. Points **must** be added in ascending
    /// `time_secs` order. Returns `false` if the curve is full.
    pub fn add_point(&mut self, point: CurvePoint) -> bool {
        self.points.push(point).is_ok()
    }

    /// Returns the number of waypoints in this curve.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Returns `true` if the curve has no waypoints.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Compute interpolated (bean_temp, env_temp) at a given elapsed time.
    ///
    /// - Before the first waypoint → returns the first waypoint's temperatures.
    /// - After the last waypoint → returns the last waypoint's temperatures.
    /// - Between two waypoints → linearly interpolates both temperatures.
    ///
    /// Returns `(0.0, 0.0)` if the curve is empty.
    pub fn temperatures_at(&self, elapsed_secs: u32) -> (f32, f32) {
        if self.points.is_empty() {
            return (0.0, 0.0);
        }

        if elapsed_secs <= self.points[0].time_secs {
            return (self.points[0].bean_temp, self.points[0].env_temp);
        }

        for i in 1..self.points.len() {
            let prev = self.points[i - 1];
            let curr = self.points[i];
            if elapsed_secs <= curr.time_secs {
                let range = curr.time_secs - prev.time_secs;
                if range == 0 {
                    return (curr.bean_temp, curr.env_temp);
                }
                let frac = (elapsed_secs - prev.time_secs) as f32 / range as f32;
                let bean = prev.bean_temp + (curr.bean_temp - prev.bean_temp) * frac;
                let env = prev.env_temp + (curr.env_temp - prev.env_temp) * frac;
                return (bean, env);
            }
        }

        let last = self.points[self.points.len() - 1];
        (last.bean_temp, last.env_temp)
    }
}

/// Runtime state for the synthetic temperature generator.
///
/// Tracks elapsed time and produces temperatures from a [`RoastCurve`]
/// at the control-loop cadence.
pub struct SimulatedSensorSource {
    curve: RoastCurve,
    start_instant: Instant,
    /// Optional: add a small sinusoidal noise to make the signal more realistic.
    /// Amplitude in °C. Set to 0.0 for a perfectly clean signal.
    noise_amplitude: f32,
    /// Running counter for noise phase.
    tick_count: u32,
}

impl SimulatedSensorSource {
    /// Create a new simulated source from a roast curve.
    pub fn new(curve: RoastCurve) -> Self {
        Self {
            curve,
            start_instant: Instant::now(),
            noise_amplitude: 0.0,
            tick_count: 0,
        }
    }

    /// Create a simulated source using the built-in medium roast curve.
    pub fn default_curve() -> Self {
        Self::new(RoastCurve::default_medium_roast())
    }

    /// Set noise amplitude in °C. A value of 0.5 adds ±0.5°C noise.
    pub fn with_noise_amplitude(mut self, amplitude: f32) -> Self {
        self.noise_amplitude = amplitude;
        self
    }

    /// Reset the simulation to t=0.
    pub fn reset(&mut self) {
        self.start_instant = Instant::now();
        self.tick_count = 0;
    }

    /// Compute the current (bean_temp, env_temp) from the curve at the
    /// current elapsed time, with optional noise applied.
    pub fn current_temperatures(&mut self) -> (f32, f32) {
        let elapsed_ms = self.start_instant.elapsed().as_millis();
        let elapsed_secs = (elapsed_ms / 1000) as u32;
        let (mut bean, mut env) = self.curve.temperatures_at(elapsed_secs);

        if self.noise_amplitude > 0.0 {
            bean += self.noise(self.tick_count, 0);
            env += self.noise(self.tick_count, 1);
        }

        self.tick_count = self.tick_count.wrapping_add(1);
        (bean, env)
    }

    /// Simple deterministic noise based on a triangle wave.
    /// No `libm` dependency — just arithmetic.
    /// `seed` differentiates bean vs env noise.
    fn noise(&self, tick: u32, seed: u32) -> f32 {
        // Period = 20 ticks (~2s at 100ms cadence)
        let phase = (tick.wrapping_add(seed * 7)) % 20;
        // Triangle wave: 0→10→0 mapped to -1→+1→-1
        let normalized = if phase < 10 {
            (phase as f32) / 10.0
        } else {
            (20 - phase) as f32 / 10.0
        };
        let wave = normalized * 2.0 - 1.0;
        wave * self.noise_amplitude
    }

    /// Returns elapsed seconds since simulation start.
    pub fn elapsed_secs(&self) -> u32 {
        (self.start_instant.elapsed().as_millis() / 1000) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_curve_returns_zeros() {
        let curve = RoastCurve::new();
        assert_eq!(curve.temperatures_at(0), (0.0, 0.0));
        assert_eq!(curve.temperatures_at(999), (0.0, 0.0));
    }

    #[test]
    fn single_point_returns_constant() {
        let mut curve = RoastCurve::new();
        curve.add_point(CurvePoint {
            time_secs: 0,
            bean_temp: 100.0,
            env_temp: 150.0,
        });
        assert_eq!(curve.temperatures_at(0), (100.0, 150.0));
        assert_eq!(curve.temperatures_at(999), (100.0, 150.0));
    }

    #[test]
    fn interpolation_midpoint() {
        let mut curve = RoastCurve::new();
        curve.add_point(CurvePoint {
            time_secs: 0,
            bean_temp: 0.0,
            env_temp: 0.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 100,
            bean_temp: 200.0,
            env_temp: 250.0,
        });
        assert_eq!(curve.temperatures_at(50), (100.0, 125.0));
    }

    #[test]
    fn before_first_returns_first() {
        let mut curve = RoastCurve::new();
        curve.add_point(CurvePoint {
            time_secs: 10,
            bean_temp: 50.0,
            env_temp: 80.0,
        });
        assert_eq!(curve.temperatures_at(0), (50.0, 80.0));
        assert_eq!(curve.temperatures_at(5), (50.0, 80.0));
    }

    #[test]
    fn after_last_holds() {
        let mut curve = RoastCurve::new();
        curve.add_point(CurvePoint {
            time_secs: 0,
            bean_temp: 20.0,
            env_temp: 25.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 60,
            bean_temp: 200.0,
            env_temp: 230.0,
        });
        assert_eq!(curve.temperatures_at(60), (200.0, 230.0));
        assert_eq!(curve.temperatures_at(999), (200.0, 230.0));
    }

    #[test]
    fn zero_range_uses_current() {
        let mut curve = RoastCurve::new();
        curve.add_point(CurvePoint {
            time_secs: 0,
            bean_temp: 100.0,
            env_temp: 120.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 0,
            bean_temp: 150.0,
            env_temp: 180.0,
        });
        // Both at time 0 → second point's values (curr)
        assert_eq!(curve.temperatures_at(0), (150.0, 180.0));
    }

    #[test]
    fn default_medium_roast_has_points() {
        let curve = RoastCurve::default_medium_roast();
        assert!(!curve.is_empty());
        assert!(curve.len() > 5);
    }

    #[test]
    fn default_medium_roast_interpolates_reasonably() {
        let curve = RoastCurve::default_medium_roast();
        let (bean, env) = curve.temperatures_at(0);
        assert!(
            bean > 20.0 && bean < 30.0,
            "BT at t=0 should be ~25, got {bean}"
        );
        assert!(
            env > 20.0 && env < 30.0,
            "ET at t=0 should be ~25, got {env}"
        );

        let (bean_mid, _) = curve.temperatures_at(180);
        assert!(
            bean_mid > 150.0 && bean_mid < 200.0,
            "BT at t=180s should be in ramp zone, got {bean_mid}"
        );
    }

    #[test]
    fn curve_respects_max_points() {
        let mut curve = RoastCurve::new();
        for i in 0..MAX_CURVE_POINTS {
            assert!(curve.add_point(CurvePoint {
                time_secs: i as u32,
                bean_temp: i as f32,
                env_temp: i as f32,
            }));
        }
        // 33rd point should fail
        assert!(!curve.add_point(CurvePoint {
            time_secs: 99,
            bean_temp: 99.0,
            env_temp: 99.0,
        }));
        assert_eq!(curve.len(), MAX_CURVE_POINTS);
    }
}
