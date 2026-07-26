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

    /// Create a light roast profile: shorter, milder curve peaking at 210°C BT.
    ///
    /// Approximate profile:
    /// - 0–30s:   Charge from ambient 25°C
    /// - 30–120s: Ramp to 140°C BT / 165°C ET (rapid drying)
    /// - 120–240s: Ramp to 180°C BT / 210°C ET (Maillard onset)
    /// - 240–360s: Ramp to 200°C BT / 230°C ET (early first crack)
    /// - 360–420s: Ramp to 210°C BT / 238°C ET (light development)
    /// - 420–480s: Hold at 210°C BT / 238°C ET
    pub fn light_roast() -> Self {
        let mut curve = Self::new();
        curve.add_point(CurvePoint {
            time_secs: 0,
            bean_temp: 25.0,
            env_temp: 25.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 30,
            bean_temp: 90.0,
            env_temp: 115.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 60,
            bean_temp: 120.0,
            env_temp: 145.0,
        });
        // Drying phase
        curve.add_point(CurvePoint {
            time_secs: 120,
            bean_temp: 140.0,
            env_temp: 165.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 180,
            bean_temp: 160.0,
            env_temp: 190.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 240,
            bean_temp: 180.0,
            env_temp: 210.0,
        });
        // Maillard / early first crack
        curve.add_point(CurvePoint {
            time_secs: 300,
            bean_temp: 190.0,
            env_temp: 220.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 360,
            bean_temp: 200.0,
            env_temp: 230.0,
        });
        // Light development
        curve.add_point(CurvePoint {
            time_secs: 420,
            bean_temp: 210.0,
            env_temp: 238.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 480,
            bean_temp: 210.0,
            env_temp: 238.0,
        });
        curve
    }

    /// Create a dark roast profile: longer, deeper curve peaking at 245°C BT.
    ///
    /// Approximate profile:
    /// - 0–60s:   Charge from ambient 25°C
    /// - 60–180s: Ramp to 150°C BT / 180°C ET (extended drying)
    /// - 180–360s: Ramp to 200°C BT / 228°C ET (Maillard phase)
    /// - 360–540s: Ramp to 225°C BT / 248°C ET (first crack through second crack)
    /// - 540–660s: Ramp to 245°C BT / 256°C ET (dark development)
    /// - 660–720s: Hold at 245°C BT / 256°C ET
    pub fn dark_roast() -> Self {
        let mut curve = Self::new();
        curve.add_point(CurvePoint {
            time_secs: 0,
            bean_temp: 25.0,
            env_temp: 25.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 30,
            bean_temp: 75.0,
            env_temp: 95.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 60,
            bean_temp: 110.0,
            env_temp: 140.0,
        });
        // Extended drying
        curve.add_point(CurvePoint {
            time_secs: 120,
            bean_temp: 130.0,
            env_temp: 160.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 180,
            bean_temp: 150.0,
            env_temp: 180.0,
        });
        // Maillard phase
        curve.add_point(CurvePoint {
            time_secs: 240,
            bean_temp: 170.0,
            env_temp: 200.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 300,
            bean_temp: 185.0,
            env_temp: 215.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 360,
            bean_temp: 200.0,
            env_temp: 228.0,
        });
        // First crack → second crack
        curve.add_point(CurvePoint {
            time_secs: 420,
            bean_temp: 210.0,
            env_temp: 238.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 480,
            bean_temp: 220.0,
            env_temp: 245.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 540,
            bean_temp: 230.0,
            env_temp: 250.0,
        });
        // Dark development
        curve.add_point(CurvePoint {
            time_secs: 600,
            bean_temp: 240.0,
            env_temp: 254.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 660,
            bean_temp: 245.0,
            env_temp: 256.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 720,
            bean_temp: 245.0,
            env_temp: 256.0,
        });
        curve
    }

    /// Create a fast roast test profile: quick aggressive ramp peaking at 220°C BT in ~300s.
    ///
    /// Approximate profile:
    /// - 0–20s:   Rapid charge from ambient 25°C
    /// - 20–60s:  Aggressive ramp to 130°C BT / 155°C ET
    /// - 60–150s: Ramp to 180°C BT / 210°C ET
    /// - 150–240s: Ramp to 210°C BT / 238°C ET
    /// - 240–300s: Ramp to 220°C BT / 245°C ET (short development)
    pub fn fast_roast() -> Self {
        let mut curve = Self::new();
        curve.add_point(CurvePoint {
            time_secs: 0,
            bean_temp: 25.0,
            env_temp: 25.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 20,
            bean_temp: 80.0,
            env_temp: 110.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 60,
            bean_temp: 130.0,
            env_temp: 155.0,
        });
        // Aggressive mid-ramp
        curve.add_point(CurvePoint {
            time_secs: 100,
            bean_temp: 160.0,
            env_temp: 185.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 150,
            bean_temp: 180.0,
            env_temp: 210.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 200,
            bean_temp: 200.0,
            env_temp: 228.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 240,
            bean_temp: 210.0,
            env_temp: 238.0,
        });
        // Short development
        curve.add_point(CurvePoint {
            time_secs: 270,
            bean_temp: 218.0,
            env_temp: 244.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 300,
            bean_temp: 220.0,
            env_temp: 245.0,
        });
        curve
    }

    /// Create a minimal pinout verification curve: ramps 25°C → 200°C → 25°C in ~120s.
    ///
    /// Intended for quick GPIO / hardware verification without a full roast cycle.
    /// Approximate profile:
    /// - 0–50s:   Ramp from ambient 25°C to 200°C BT / 225°C ET
    /// - 50–100s: Hold at 200°C BT / 225°C ET (exercise steady-state)
    /// - 100–120s: Cool back down to 25°C BT / 25°C ET
    pub fn pinout_verify() -> Self {
        let mut curve = Self::new();
        curve.add_point(CurvePoint {
            time_secs: 0,
            bean_temp: 25.0,
            env_temp: 25.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 15,
            bean_temp: 80.0,
            env_temp: 100.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 30,
            bean_temp: 130.0,
            env_temp: 155.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 50,
            bean_temp: 200.0,
            env_temp: 225.0,
        });
        // Hold steady
        curve.add_point(CurvePoint {
            time_secs: 100,
            bean_temp: 200.0,
            env_temp: 225.0,
        });
        // Cooldown
        curve.add_point(CurvePoint {
            time_secs: 120,
            bean_temp: 25.0,
            env_temp: 25.0,
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
            // Bug C4b-exposed (2026-07-25): when the FIRST two points share
            // the same `time_secs` (e.g. a curve authored so the user can
            // express "from t=0 the bean is at X, the env is at Y" as two
            // coincident points), `elapsed_secs == points[0].time_secs`
            // matched the FIRST point rather than the LATEST point at that
            // time. Returning the first point is also what `points[0]`
            // already gives us, so this early-return was only the legacy
            // behaviour — but the same loop below resolves `range == 0` in
            // favour of `curr`. We now do the same here: scan forward while
            // the next point shares this `time_secs` and return the LAST
            // coincident one, so a 0-range at t=0 is consistent with a
            // 0-range elsewhere in the curve. (This test was previously
            // unreachable because the C4b cfg bug blocked `regression`
            // from compiling at all; fixing that exposed this latent gap.)
            let mut idx = 0usize;
            while idx + 1 < self.points.len()
                && self.points[idx + 1].time_secs == self.points[0].time_secs
            {
                idx += 1;
            }
            return (self.points[idx].bean_temp, self.points[idx].env_temp);
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

    #[test]
    fn light_roast_has_points_and_reasonable_bounds() {
        let curve = RoastCurve::light_roast();
        assert!(!curve.is_empty());
        assert!(curve.len() > 5);

        let (bean, env) = curve.temperatures_at(0);
        assert!(
            bean > 20.0 && bean < 30.0,
            "BT at t=0 should be ~25, got {bean}"
        );
        assert!(
            env > 20.0 && env < 30.0,
            "ET at t=0 should be ~25, got {env}"
        );

        let (bean_mid, _) = curve.temperatures_at(120);
        assert!(
            bean_mid > 130.0 && bean_mid < 160.0,
            "BT at t=120s should be in drying zone, got {bean_mid}"
        );

        let (bean_end, env_end) = curve.temperatures_at(480);
        assert!(
            bean_end > 200.0 && bean_end <= 210.0,
            "BT at t=480s should peak near 210, got {bean_end}"
        );
        assert!(
            env_end < 250.0,
            "ET at t=480s should stay below 250, got {env_end}"
        );
    }

    #[test]
    fn light_roast_monotonic_bt_during_ramp() {
        let curve = RoastCurve::light_roast();
        let (bt_0, _) = curve.temperatures_at(0);
        let (bt_120, _) = curve.temperatures_at(120);
        let (bt_240, _) = curve.temperatures_at(240);
        let (bt_360, _) = curve.temperatures_at(360);
        let (bt_420, _) = curve.temperatures_at(420);
        assert!(bt_0 < bt_120, "BT should increase 0→120s");
        assert!(bt_120 < bt_240, "BT should increase 120→240s");
        assert!(bt_240 < bt_360, "BT should increase 240→360s");
        assert!(bt_360 < bt_420, "BT should increase 360→420s");
    }

    #[test]
    fn dark_roast_has_points_and_reasonable_bounds() {
        let curve = RoastCurve::dark_roast();
        assert!(!curve.is_empty());
        assert!(curve.len() > 5);

        let (bean, env) = curve.temperatures_at(0);
        assert!(
            bean > 20.0 && bean < 30.0,
            "BT at t=0 should be ~25, got {bean}"
        );
        assert!(
            env > 20.0 && env < 30.0,
            "ET at t=0 should be ~25, got {env}"
        );

        let (bean_end, env_end) = curve.temperatures_at(720);
        assert!(
            bean_end > 240.0 && bean_end <= 245.0,
            "BT at t=720s should peak near 245, got {bean_end}"
        );
        assert!(
            env_end <= 258.0,
            "ET at t=720s should stay below 260, got {env_end}"
        );
    }

    #[test]
    fn dark_roast_monotonic_bt_during_ramp() {
        let curve = RoastCurve::dark_roast();
        let (bt_0, _) = curve.temperatures_at(0);
        let (bt_180, _) = curve.temperatures_at(180);
        let (bt_360, _) = curve.temperatures_at(360);
        let (bt_540, _) = curve.temperatures_at(540);
        let (bt_660, _) = curve.temperatures_at(660);
        assert!(bt_0 < bt_180, "BT should increase 0→180s");
        assert!(bt_180 < bt_360, "BT should increase 180→360s");
        assert!(bt_360 < bt_540, "BT should increase 360→540s");
        assert!(bt_540 < bt_660, "BT should increase 540→660s");
    }

    #[test]
    fn fast_roast_has_points_and_reasonable_bounds() {
        let curve = RoastCurve::fast_roast();
        assert!(!curve.is_empty());
        assert!(curve.len() > 5);

        let (bean, env) = curve.temperatures_at(0);
        assert!(
            bean > 20.0 && bean < 30.0,
            "BT at t=0 should be ~25, got {bean}"
        );
        assert!(
            env > 20.0 && env < 30.0,
            "ET at t=0 should be ~25, got {env}"
        );

        let (bean_end, env_end) = curve.temperatures_at(300);
        assert!(
            bean_end > 215.0 && bean_end <= 220.0,
            "BT at t=300s should peak near 220, got {bean_end}"
        );
        assert!(
            env_end <= 250.0,
            "ET at t=300s should stay below 250, got {env_end}"
        );
    }

    #[test]
    fn fast_roast_monotonic_bt_during_ramp() {
        let curve = RoastCurve::fast_roast();
        let (bt_0, _) = curve.temperatures_at(0);
        let (bt_60, _) = curve.temperatures_at(60);
        let (bt_150, _) = curve.temperatures_at(150);
        let (bt_240, _) = curve.temperatures_at(240);
        let (bt_300, _) = curve.temperatures_at(300);
        assert!(bt_0 < bt_60, "BT should increase 0→60s");
        assert!(bt_60 < bt_150, "BT should increase 60→150s");
        assert!(bt_150 < bt_240, "BT should increase 150→240s");
        assert!(bt_240 < bt_300, "BT should increase 240→300s");
    }

    #[test]
    fn pinout_verify_has_points_and_reasonable_bounds() {
        let curve = RoastCurve::pinout_verify();
        assert!(!curve.is_empty());
        assert!(curve.len() >= 4);

        let (bean, env) = curve.temperatures_at(0);
        assert!(
            bean > 20.0 && bean < 30.0,
            "BT at t=0 should be ~25, got {bean}"
        );
        assert!(
            env > 20.0 && env < 30.0,
            "ET at t=0 should be ~25, got {env}"
        );
    }

    #[test]
    fn pinout_verify_monotonic_ramp_up_and_cooldown() {
        let curve = RoastCurve::pinout_verify();
        let (bt_0, _) = curve.temperatures_at(0);
        let (bt_30, _) = curve.temperatures_at(30);
        let (bt_50, _) = curve.temperatures_at(50);
        let (bt_100, _) = curve.temperatures_at(100);
        let (bt_120, _) = curve.temperatures_at(120);

        assert!(bt_0 < bt_30, "BT should increase 0→30s");
        assert!(bt_30 < bt_50, "BT should increase 30→50s");
        assert!(
            (bt_50 - bt_100).abs() < 1.0,
            "BT should hold steady 50→100s, got {bt_50}→{bt_100}"
        );
        assert!(
            bt_120 < bt_100,
            "BT should decrease during cooldown 100→120s"
        );
    }

    #[test]
    fn all_curves_stay_below_overtemp() {
        let overtemp_bt = 250.0_f32;
        let overtemp_et = 260.0_f32;

        let curves: Vec<(&str, RoastCurve)> = vec![
            ("medium", RoastCurve::default_medium_roast()),
            ("light", RoastCurve::light_roast()),
            ("dark", RoastCurve::dark_roast()),
            ("fast", RoastCurve::fast_roast()),
            ("pinout_verify", RoastCurve::pinout_verify()),
        ];

        for (name, curve) in curves {
            let (bt, et) = curve.temperatures_at(0);
            assert!(
                bt <= overtemp_bt,
                "{name}: BT at t=0 should be ≤250, got {bt}"
            );
            assert!(
                et <= overtemp_et,
                "{name}: ET at t=0 should be ≤260, got {et}"
            );
        }
    }
}
