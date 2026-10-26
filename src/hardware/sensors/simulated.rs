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

#[cfg(feature = "simulated-sensors")]
use super::thermal_model::{ThermalPlant, ThermalPlantConfig};

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
    /// Create an empty curve. Use `add_point` to build it up.
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

    /// Create a medium roast from a real, public bean-probe curve.
    ///
    /// Source: Zenodo record 21383046 (Teodoro et al., 2025), distributed by the
    /// Hugging Face dataset `luispoveda93/coffee-roast-profiles` (CC-BY-4.0),
    /// real row `real-zenodo-000`. The source publishes bean-probe temperature
    /// only, so `env_temp` is derived as `bean_temp + 20.0 °C`; the bean trace is
    /// the published data sampled every 30 s plus the final 696 s point.
    ///
    /// Approximate profile:
    /// - 0–30s:   Charge at 184°C BT, briefly rising
    /// - 30–60s:  Turning point at 165°C BT
    /// - 60–420s: Drying/Maillard ramp to 195°C BT
    /// - 420–696s: Development to 207°C BT (drop)
    pub fn web_medium_roast() -> Self {
        let mut curve = Self::new();
        curve.add_point(CurvePoint {
            time_secs: 0,
            bean_temp: 183.8,
            env_temp: 203.8,
        });
        curve.add_point(CurvePoint {
            time_secs: 30,
            bean_temp: 188.0,
            env_temp: 208.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 60,
            bean_temp: 165.4,
            env_temp: 185.4,
        });
        curve.add_point(CurvePoint {
            time_secs: 90,
            bean_temp: 169.8,
            env_temp: 189.8,
        });
        curve.add_point(CurvePoint {
            time_secs: 120,
            bean_temp: 173.7,
            env_temp: 193.7,
        });
        curve.add_point(CurvePoint {
            time_secs: 150,
            bean_temp: 177.1,
            env_temp: 197.1,
        });
        curve.add_point(CurvePoint {
            time_secs: 180,
            bean_temp: 179.5,
            env_temp: 199.5,
        });
        curve.add_point(CurvePoint {
            time_secs: 210,
            bean_temp: 181.8,
            env_temp: 201.8,
        });
        curve.add_point(CurvePoint {
            time_secs: 240,
            bean_temp: 184.4,
            env_temp: 204.4,
        });
        curve.add_point(CurvePoint {
            time_secs: 270,
            bean_temp: 186.6,
            env_temp: 206.6,
        });
        curve.add_point(CurvePoint {
            time_secs: 300,
            bean_temp: 188.7,
            env_temp: 208.7,
        });
        curve.add_point(CurvePoint {
            time_secs: 330,
            bean_temp: 190.2,
            env_temp: 210.2,
        });
        curve.add_point(CurvePoint {
            time_secs: 360,
            bean_temp: 191.6,
            env_temp: 211.6,
        });
        curve.add_point(CurvePoint {
            time_secs: 390,
            bean_temp: 193.2,
            env_temp: 213.2,
        });
        curve.add_point(CurvePoint {
            time_secs: 420,
            bean_temp: 194.6,
            env_temp: 214.6,
        });
        curve.add_point(CurvePoint {
            time_secs: 450,
            bean_temp: 196.1,
            env_temp: 216.1,
        });
        curve.add_point(CurvePoint {
            time_secs: 480,
            bean_temp: 197.5,
            env_temp: 217.5,
        });
        curve.add_point(CurvePoint {
            time_secs: 510,
            bean_temp: 199.2,
            env_temp: 219.2,
        });
        curve.add_point(CurvePoint {
            time_secs: 540,
            bean_temp: 200.4,
            env_temp: 220.4,
        });
        curve.add_point(CurvePoint {
            time_secs: 570,
            bean_temp: 201.5,
            env_temp: 221.5,
        });
        curve.add_point(CurvePoint {
            time_secs: 600,
            bean_temp: 203.0,
            env_temp: 223.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 630,
            bean_temp: 204.4,
            env_temp: 224.4,
        });
        curve.add_point(CurvePoint {
            time_secs: 660,
            bean_temp: 205.6,
            env_temp: 225.6,
        });
        curve.add_point(CurvePoint {
            time_secs: 690,
            bean_temp: 207.0,
            env_temp: 227.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 696,
            bean_temp: 207.4,
            env_temp: 227.4,
        });
        curve
    }

    /// Create a guard-safe typical roast profile for START/Heating tests.
    ///
    /// The bean probe starts at a realistic charge temperature, drops through
    /// the turning point, and then rises slowly enough that every positive BT
    /// segment stays below the soft rate-of-rise limit. Unlike
    /// [`Self::default_medium_roast`], `START` can stay in `Heating` without
    /// latching `Bean temperature rate-of-rise exceeded`.
    ///
    /// Approximate profile:
    /// - 0–60s:   Charge at 200°C BT / 220°C ET, dropping to a 100°C turning point
    /// - 60–300s: Drying ramp to 168°C BT / 225°C ET
    /// - 300–420s: Maillard ramp to 196°C BT / 234°C ET (first crack)
    /// - 420–600s: Development to 214°C BT / 243°C ET
    pub fn typical_roast() -> Self {
        let mut curve = Self::new();
        curve.add_point(CurvePoint {
            time_secs: 0,
            bean_temp: 200.0,
            env_temp: 220.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 30,
            bean_temp: 120.0,
            env_temp: 210.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 60,
            bean_temp: 100.0,
            env_temp: 205.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 120,
            bean_temp: 110.0,
            env_temp: 210.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 180,
            bean_temp: 130.0,
            env_temp: 215.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 240,
            bean_temp: 150.0,
            env_temp: 220.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 300,
            bean_temp: 168.0,
            env_temp: 225.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 360,
            bean_temp: 184.0,
            env_temp: 230.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 420,
            bean_temp: 196.0,
            env_temp: 234.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 480,
            bean_temp: 204.0,
            env_temp: 238.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 540,
            bean_temp: 210.0,
            env_temp: 241.0,
        });
        curve.add_point(CurvePoint {
            time_secs: 600,
            bean_temp: 214.0,
            env_temp: 243.0,
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
            // When the FIRST two points share the same `time_secs`, scan
            // forward and return the LAST coincident one, so a 0-range at
            // t=0 is consistent with a 0-range elsewhere in the curve.
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
                if frac >= 1.0 {
                    // BUG-2b-1 (audit 2026-10-04): exact segment end — return
                    // the authored waypoint itself. The two-step lerp
                    // `a + fl(fl(b - a) * 1.0)` double-rounds and can land a
                    // few ULP OUTSIDE the segment, breaking the envelope
                    // property the regression proptest asserts.
                    return (curr.bean_temp, curr.env_temp);
                }
                let bean = prev.bean_temp + (curr.bean_temp - prev.bean_temp) * frac;
                let env = prev.env_temp + (curr.env_temp - prev.env_temp) * frac;
                // Clamp to the segment so the pair can never leave
                // [min(prev, curr), max(prev, curr)] for ANY frac — the
                // regression proptest asserts results stay inside the
                // authored envelope.
                let bean = bean.clamp(
                    prev.bean_temp.min(curr.bean_temp),
                    prev.bean_temp.max(curr.bean_temp),
                );
                let env = env.clamp(
                    prev.env_temp.min(curr.env_temp),
                    prev.env_temp.max(curr.env_temp),
                );
                return (bean, env);
            }
        }

        let last = self.points[self.points.len() - 1];
        (last.bean_temp, last.env_temp)
    }
}

/// Runtime state for the synthetic temperature generator.
///
/// Tracks elapsed time and produces temperatures from a `RoastCurve`
/// at the control-loop cadence. When a `ThermalPlant` is attached
/// (`with_thermal_plant`), temperatures are driven by heater/fan via
/// the plant model (closed-loop) instead of pure time interpolation.
pub struct SimulatedSensorSource {
    curve: RoastCurve,
    start_instant: Instant,
    /// Optional: add a small sinusoidal noise to make the signal more realistic.
    /// Amplitude in °C. Set to 0.0 for a perfectly clean signal.
    noise_amplitude: f32,
    /// Running counter for noise phase.
    tick_count: u32,
    /// When `Some`, closed-loop plant drives temps (heater/fan aware).
    plant: Option<ThermalPlant>,
    last_heater_pct: f32,
    last_fan_pct: f32,
    last_update_instant: Option<Instant>,
    /// Last emitted temperatures. Used by `SIM;STATUS` and by freeze mode.
    last_temps: (f32, f32),
    /// Simulated MAX31856 fault-register masks (bit map is the same as the
    /// real chip). Applied to the next samples.
    bean_fault_mask: u8,
    env_fault_mask: u8,
    /// Optional hard temperature override, e.g. to exercise ET over-temp.
    override_temps: Option<(f32, f32)>,
    /// When true, temperatures stop evolving but keep their last value.
    frozen: bool,
    paused_since: Option<Instant>,
}

/// Read-only snapshot of the synthetic source for `SIM;STATUS`.
#[derive(Debug, Clone, Copy)]
pub struct SimulationSnapshot {
    pub bean_temp: f32,
    pub env_temp: f32,
    pub heater_pct: f32,
    pub fan_pct: f32,
    pub has_plant: bool,
    pub frozen: bool,
    pub bean_fault: u8,
    pub env_fault: u8,
}

impl SimulatedSensorSource {
    /// Create a new simulated source from a roast curve.
    pub fn new(curve: RoastCurve) -> Self {
        let initial_temps = curve.temperatures_at(0);
        Self {
            curve,
            start_instant: Instant::now(),
            noise_amplitude: 0.0,
            tick_count: 0,
            plant: None,
            last_heater_pct: 0.0,
            last_fan_pct: 0.0,
            last_update_instant: None,
            last_temps: initial_temps,
            bean_fault_mask: 0,
            env_fault_mask: 0,
            override_temps: None,
            frozen: false,
            paused_since: None,
        }
    }

    /// Create a simulated source using the build's default curve.
    ///
    /// With `simulated-web-medium-roast`, the default is the public
    /// [`RoastCurve::web_medium_roast`]; with `simulated-typical-roast` it is
    /// the guard-safe [`RoastCurve::typical_roast`]; otherwise it is
    /// [`RoastCurve::default_medium_roast`]. The two profile features are
    /// mutually exclusive (enforced at crate level), so `AppBuilder` and
    /// direct callers always agree on the selected curve.
    pub fn default_curve() -> Self {
        #[cfg(feature = "simulated-web-medium-roast")]
        let curve = RoastCurve::web_medium_roast();
        #[cfg(all(
            feature = "simulated-typical-roast",
            not(feature = "simulated-web-medium-roast")
        ))]
        let curve = RoastCurve::typical_roast();
        #[cfg(not(any(
            feature = "simulated-typical-roast",
            feature = "simulated-web-medium-roast"
        )))]
        let curve = RoastCurve::default_medium_roast();
        Self::new(curve)
    }

    /// Create a simulated source using [`RoastCurve::web_medium_roast`].
    pub fn web_medium_roast() -> Self {
        Self::new(RoastCurve::web_medium_roast())
    }

    /// Set noise amplitude in °C. A value of 0.5 adds ±0.5°C noise.
    pub fn with_noise_amplitude(mut self, amplitude: f32) -> Self {
        self.noise_amplitude = amplitude;
        self
    }

    /// Attach a realistic thermal plant (closed-loop). Temperatures will be
    /// driven by heater/fan instead of pure waypoint time. Call
    /// `set_heater_fan` after each `update_control` tick.
    pub fn with_thermal_plant(mut self, config: ThermalPlantConfig) -> Self {
        let plant = ThermalPlant::new(config);
        self.last_temps = plant.measured();
        self.plant = Some(plant);
        self.last_update_instant = Some(Instant::now());
        self
    }

    /// Attach thermal plant with explicit initial temps (e.g. after charge).
    pub fn with_thermal_plant_and_initial(
        mut self,
        config: ThermalPlantConfig,
        bean_true: f32,
        et_true: f32,
    ) -> Self {
        let plant = ThermalPlant::new_with_initial(config, bean_true, et_true);
        self.last_temps = plant.measured();
        self.plant = Some(plant);
        self.last_update_instant = Some(Instant::now());
        self
    }

    /// Update the plant's actuator inputs (heater 0..100%, fan 0..100%).
    /// The next `current_temperatures()` will advance the plant using these
    /// values and elapsed dt. No-op when no plant attached.
    pub fn set_heater_fan(&mut self, heater_pct: f32, fan_pct: f32) {
        self.last_heater_pct = heater_pct.clamp(0.0, 100.0);
        self.last_fan_pct = fan_pct.clamp(0.0, 100.0);
    }

    /// Simulate a cold bean charge: drops BT true, ET less. No-op without plant.
    pub fn inject_charge(&mut self, bean_drop_c: f32) {
        if let Some(p) = self.plant.as_mut() {
            p.inject_charge(bean_drop_c);
        }
    }

    /// Whether this source is in closed-loop plant mode.
    pub fn has_plant(&self) -> bool {
        self.plant.is_some()
    }

    /// Inject MAX31856 fault-register bits on the next samples. `0` clears.
    pub fn set_fault_masks(&mut self, bean_mask: u8, env_mask: u8) {
        self.bean_fault_mask = bean_mask;
        self.env_fault_mask = env_mask;
    }

    /// Current simulated fault-register masks.
    pub fn fault_masks(&self) -> (u8, u8) {
        (self.bean_fault_mask, self.env_fault_mask)
    }

    /// Override both reported temperatures. `None` resumes curve/plant mode.
    pub fn set_temperature_override(&mut self, temps: Option<(f32, f32)>) {
        self.override_temps = temps;
        if let Some(values) = temps {
            self.last_temps = values;
        }
        self.sync_pause_anchor(Instant::now());
    }

    /// Freeze/unfreeze temperature evolution while keeping the last value.
    pub fn set_frozen(&mut self, frozen: bool) {
        self.frozen = frozen;
        self.sync_pause_anchor(Instant::now());
    }

    fn sync_pause_anchor(&mut self, now: Instant) {
        let paused = self.frozen || self.override_temps.is_some();
        if paused {
            if self.paused_since.is_none() {
                self.paused_since = Some(now);
            }
            return;
        }
        if let Some(since) = self.paused_since.take() {
            let pause = now.saturating_duration_since(since);
            self.start_instant += pause;
            self.last_update_instant = Some(now);
        }
    }

    /// Whether temperature evolution is frozen.
    pub fn frozen(&self) -> bool {
        self.frozen
    }

    /// Last emitted temperatures without advancing the model.
    pub fn last_temperatures(&self) -> (f32, f32) {
        self.last_temps
    }

    /// Snapshot for `SIM;STATUS` without advancing the simulation.
    pub fn snapshot(&self) -> SimulationSnapshot {
        SimulationSnapshot {
            bean_temp: self.last_temps.0,
            env_temp: self.last_temps.1,
            heater_pct: self.last_heater_pct,
            fan_pct: self.last_fan_pct,
            has_plant: self.plant.is_some(),
            frozen: self.frozen,
            bean_fault: self.bean_fault_mask,
            env_fault: self.env_fault_mask,
        }
    }

    /// Advance plant with explicit dt (s) without noise. Useful for
    /// deterministic tests (`Instant` wall-clock would otherwise quantise to
    /// seconds via `temperatures_at`).
    pub fn plant_advance(&mut self, heater_pct: f32, fan_pct: f32, dt_secs: f32) -> (f32, f32) {
        if let Some(p) = self.plant.as_mut() {
            let (mut bean, mut env) = p.update(heater_pct, fan_pct, dt_secs);
            if self.noise_amplitude > 0.0 {
                bean += self.noise(self.tick_count, 0);
                env += self.noise(self.tick_count, 1);
            }
            self.tick_count = self.tick_count.wrapping_add(1);
            self.last_heater_pct = heater_pct.clamp(0.0, 100.0);
            self.last_fan_pct = fan_pct.clamp(0.0, 100.0);
            self.last_temps = (bean, env);
            // Keep wall-clock anchor coherent for HIL (`not(test)` uses
            // wall-clock dt). In `cfg(test)` dt is fixed (`CONTROL_LOOP_TICK_MS`),
            // so do NOT advance wall-clock — otherwise a subsequent
            // `current_temperatures()` would see dt≈0 and suppress evolution
            // for one tick, breaking mixed `plant_advance`→`current_temperatures`
            // sequences used in some L3 tests. Advance logical anchor instead.
            #[cfg(not(test))]
            {
                self.last_update_instant = Some(Instant::now());
            }
            #[cfg(test)]
            {
                if let Some(last) = self.last_update_instant {
                    self.last_update_instant =
                        Some(last + embassy_time::Duration::from_millis((dt_secs * 1000.0) as u64));
                } else {
                    self.last_update_instant = Some(Instant::now());
                }
            }
            (bean, env)
        } else {
            self.current_temperatures()
        }
    }

    /// Reset the simulation to t=0 and clear injected faults/overrides.
    pub fn reset(&mut self) {
        self.start_instant = Instant::now();
        self.tick_count = 0;
        self.last_update_instant = Some(Instant::now());
        self.last_heater_pct = 0.0;
        self.last_fan_pct = 0.0;
        self.bean_fault_mask = 0;
        self.env_fault_mask = 0;
        self.override_temps = None;
        self.frozen = false;
        self.paused_since = None;
        if let Some(p) = self.plant.as_mut() {
            p.reset();
            self.last_temps = p.measured();
        } else {
            self.last_temps = self.curve.temperatures_at(0);
        }
    }

    /// Compute the current (bean_temp, env_temp) from the curve at the
    /// current elapsed time, with optional noise applied.
    /// In plant mode, advances the `ThermalPlant` by elapsed dt since last
    /// call using the last `heater/fan` values, so closed-loop heat truly
    /// moves BT/ET with realistic lag (drum τ~12s, bean τ~18s, probe τ~4s /
    /// 1.5s, moisture plateau, crack exotherm).
    pub fn current_temperatures(&mut self) -> (f32, f32) {
        if let Some(values) = self.override_temps {
            self.last_temps = values;
            return values;
        }
        if self.frozen {
            return self.last_temps;
        }

        if let Some(plant) = self.plant.as_mut() {
            // Deterministic dt in tests (`cfg(test)` → 0.31s = CONTROL_LOOP_TICK_MS)
            // eliminates wall-clock flakiness for L3/thermal tests that drive
            // `current_temperatures()` directly (see `plant_advance` for explicit
            // dt). On non-test builds (HIL/embedded with `simulated-sensors`)
            // keep wall-clock jitter handling.
            let dt_secs: f32 = {
                #[cfg(test)]
                {
                    // Use the real tick constant so HIL calibration stays valid.
                    crate::config::constants::CONTROL_LOOP_TICK_MS as f32 / 1000.0
                }
                #[cfg(not(test))]
                {
                    let now_tmp = Instant::now();
                    if let Some(last) = self.last_update_instant {
                        let elapsed_secs =
                            now_tmp.saturating_duration_since(last).as_millis() as f32 / 1000.0;
                        // Clamp to plausible tick (0.02..2.0s) to avoid wall-clock jumps
                        // on host (e.g. debugger pause) blowing up the integrator.
                        elapsed_secs.clamp(0.02, 2.0)
                    } else {
                        crate::config::constants::CONTROL_LOOP_TICK_MS as f32 / 1000.0
                    }
                }
            };
            self.last_update_instant = Some(Instant::now());
            let (mut bean, mut env) =
                plant.update(self.last_heater_pct, self.last_fan_pct, dt_secs);
            if self.noise_amplitude > 0.0 {
                bean += self.noise(self.tick_count, 0);
                env += self.noise(self.tick_count, 1);
            }
            self.tick_count = self.tick_count.wrapping_add(1);
            self.last_temps = (bean, env);
            return (bean, env);
        }

        let elapsed_ms = self.start_instant.elapsed().as_millis();
        let elapsed_secs = (elapsed_ms / 1000) as u32;
        let (mut bean, mut env) = self.curve.temperatures_at(elapsed_secs);

        if self.noise_amplitude > 0.0 {
            bean += self.noise(self.tick_count, 0);
            env += self.noise(self.tick_count, 1);
        }

        self.tick_count = self.tick_count.wrapping_add(1);
        self.last_temps = (bean, env);
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
    use crate::config::constants::MAX_BT_RATE_OF_RISE;
    use proptest::prelude::*;

    proptest! {
        /// ANY waypoint set — including
        /// degenerate curves (empty, single point, coincident times =
        /// zero-range, unsorted times, huge values) — must answer
        /// `temperatures_at` without panicking, always returning a FINITE
        /// pair, and (for non-empty curves) a pair inside the authored
        /// temperature envelope. Zero-range waypoints are handled; this property keeps the interpolation total.
        #[test]
        fn temperatures_at_never_panics_and_stays_bounded(
            times in prop::collection::vec(0u32..86_400, 0..16),
            bean_temps in prop::collection::vec(-50f32..400.0, 0..16),
            env_temps in prop::collection::vec(-50f32..400.0, 0..16),
            elapsed in 0u32..86_400,
        ) {
            let mut curve = RoastCurve::new();
            for (i, &time_secs) in times.iter().enumerate() {
                // The temp vectors are independent generators: either may be
                // empty while `times` is not. Fall back to 0.0 for missing
                // temps.
                let bean = bean_temps
                    .get(i % bean_temps.len().max(1))
                    .copied()
                    .unwrap_or(0.0);
                let env = env_temps
                    .get(i % env_temps.len().max(1))
                    .copied()
                    .unwrap_or(0.0);
                curve.add_point(CurvePoint {
                    time_secs,
                    bean_temp: bean,
                    env_temp: env,
                });
            }

            let (bt, et) = curve.temperatures_at(elapsed);
            assert!(
                bt.is_finite() && et.is_finite(),
                "temperatures_at must return finite pairs, got ({bt:?}, {et:?})"
            );

            // Envelope check only makes sense when both temp vectors are
            // non-empty: `envelope(&[])` is (MAX, MIN), an empty range.
            if !times.is_empty() && !bean_temps.is_empty() && !env_temps.is_empty() {
                let (min_bt, max_bt) = envelope(&bean_temps);
                let (min_et, max_et) = envelope(&env_temps);
                assert!(
                    (min_bt..=max_bt).contains(&bt) && (min_et..=max_et).contains(&et),
                    "interpolated temps must stay inside the authored envelope, \
                     got ({bt}, {et}) for envelope BT[{min_bt},{max_bt}] ET[{min_et},{max_et}]"
                );
            }
        }

        /// Fase 3: a curve built only from coincident times (all points at
        /// t=0, the degenerate zero-range case) must still answer finitely
        /// at any elapsed time.
        #[test]
        fn all_coincident_waypoints_never_panic(
            temps in prop::collection::vec(-50f32..400.0, 1..16),
            elapsed in 0u32..86_400,
        ) {
            let mut curve = RoastCurve::new();
            for t in &temps {
                curve.add_point(CurvePoint {
                    time_secs: 0,
                    bean_temp: *t,
                    env_temp: *t,
                });
            }
            let (bt, et) = curve.temperatures_at(elapsed);
            assert!(bt.is_finite() && et.is_finite());
        }
    }

    fn envelope(values: &[f32]) -> (f32, f32) {
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        for v in values {
            min = min.min(*v);
            max = max.max(*v);
        }
        (min, max)
    }

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
    fn typical_roast_has_charge_dip_and_development() {
        let curve = RoastCurve::typical_roast();
        assert!(curve.len() >= 10);

        let (charge_bt, charge_et) = curve.temperatures_at(0);
        assert!(
            (190.0..=210.0).contains(&charge_bt),
            "charge BT should be ~200, got {charge_bt}"
        );
        assert!(
            (210.0..=230.0).contains(&charge_et),
            "charge ET should be ~220, got {charge_et}"
        );

        let (turning_bt, _) = curve.temperatures_at(60);
        assert!(
            (95.0..=105.0).contains(&turning_bt),
            "turning point BT should be ~100, got {turning_bt}"
        );

        let (first_crack_bt, _) = curve.temperatures_at(420);
        assert!(
            (190.0..=200.0).contains(&first_crack_bt),
            "first-crack BT should be ~196, got {first_crack_bt}"
        );

        let (end_bt, end_et) = curve.temperatures_at(600);
        assert!(
            (210.0..=215.0).contains(&end_bt),
            "development BT should be ~214, got {end_bt}"
        );
        assert!(
            end_et < 250.0,
            "development ET should stay below 250, got {end_et}"
        );
    }

    #[cfg(feature = "simulated-typical-roast")]
    #[test]
    fn default_curve_uses_typical_roast_when_feature_is_enabled() {
        let mut source = SimulatedSensorSource::default_curve();
        let (bt, et) = source.current_temperatures();
        assert!(
            (150.0..=200.0).contains(&bt),
            "F3 default BT should start near the 200 C charge, got {bt}"
        );
        assert!(
            (200.0..=220.0).contains(&et),
            "F3 default ET should start near the 220 C charge, got {et}"
        );
    }

    #[test]
    fn typical_roast_positive_bt_slopes_stay_below_soft_ror_limit() {
        let curve = RoastCurve::typical_roast();
        let mut previous_bt = curve.temperatures_at(0).0;
        for time_secs in (30u32..=600).step_by(30) {
            let current_bt = curve.temperatures_at(time_secs).0;
            let slope = (current_bt - previous_bt) / 30.0;
            if slope > 0.0 {
                assert!(
                    slope <= MAX_BT_RATE_OF_RISE,
                    "positive BT slope {slope} °C/s at {time_secs}s exceeds soft RoR limit {MAX_BT_RATE_OF_RISE}"
                );
            }
            previous_bt = current_bt;
        }
    }

    #[test]
    fn web_medium_roast_has_published_endpoints_and_drop() {
        let curve = RoastCurve::web_medium_roast();
        assert_eq!(curve.len(), 25);

        let (start_bt, start_et) = curve.temperatures_at(0);
        assert!((start_bt - 183.8).abs() < 0.01);
        assert!((start_et - 203.8).abs() < 0.01);

        let (turning_bt, _) = curve.temperatures_at(60);
        assert!((turning_bt - 165.4).abs() < 0.01);

        let (drop_bt, drop_et) = curve.temperatures_at(696);
        assert!((drop_bt - 207.4).abs() < 0.01);
        assert!((drop_et - 227.4).abs() < 0.01);
    }

    #[test]
    fn web_medium_roast_positive_bt_slopes_stay_below_soft_ror_limit() {
        let curve = RoastCurve::web_medium_roast();
        let mut previous_bt = curve.temperatures_at(0).0;
        for time_secs in (30u32..=696).step_by(30) {
            let current_bt = curve.temperatures_at(time_secs).0;
            let slope = (current_bt - previous_bt) / 30.0;
            if slope > 0.0 {
                assert!(
                    slope <= MAX_BT_RATE_OF_RISE,
                    "positive BT slope {slope} °C/s at {time_secs}s exceeds soft RoR limit {MAX_BT_RATE_OF_RISE}"
                );
            }
            previous_bt = current_bt;
        }
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
    fn temperatures_at_exact_segment_end_returns_authored_waypoint() {
        // BUG-2b-1 reproducer (baseline seed `cc d8a1179a...`): the env lerp
        // at frac = 1.0 double-rounded ~5 ULP below the authored envelope
        // minimum (-21.284622192382812 < -21.284613) and tripped the
        // `temperatures_at_never_panics_and_stays_bounded` assertion.
        let mut curve = RoastCurve::new();
        assert!(curve.add_point(CurvePoint {
            time_secs: 0,
            bean_temp: 0.0,
            env_temp: 0.0,
        }));
        assert!(curve.add_point(CurvePoint {
            time_secs: 0,
            bean_temp: 0.0,
            env_temp: 236.89806,
        }));
        assert!(curve.add_point(CurvePoint {
            time_secs: 77922,
            bean_temp: 0.0,
            env_temp: -21.284613,
        }));

        let (bt, et) = curve.temperatures_at(77922);
        assert_eq!(bt, 0.0f32);
        // The authored waypoint, exactly — not the double-rounded lerp.
        assert_eq!(et, -21.284613f32);
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

    #[test]
    fn simulation_override_and_freeze_hold_values() {
        let mut source = SimulatedSensorSource::default_curve();
        source.set_temperature_override(Some((123.0, 234.0)));
        assert_eq!(source.current_temperatures(), (123.0, 234.0));
        assert_eq!(source.last_temperatures(), (123.0, 234.0));

        source.set_temperature_override(None);
        source.set_frozen(true);
        let frozen = source.current_temperatures();
        assert_eq!(source.current_temperatures(), frozen);

        source.set_frozen(false);
        source.reset();
        assert!(!source.frozen());
        assert_eq!(source.last_temperatures(), source.curve.temperatures_at(0));
    }
    #[test]
    fn unfreeze_reanchors_curve_and_plant_clocks() {
        let mut source = SimulatedSensorSource::default_curve()
            .with_thermal_plant(ThermalPlantConfig::default());
        let start_before = source.start_instant;
        source.set_frozen(true);
        std::thread::sleep(std::time::Duration::from_millis(150));
        let resume_before = Instant::now();
        source.set_frozen(false);

        let shifted_ms = source
            .start_instant
            .saturating_duration_since(start_before)
            .as_millis();
        assert!(
            shifted_ms >= 100,
            "curve clock should resume at the pause point, shifted {shifted_ms}ms"
        );
        let last_update = source
            .last_update_instant
            .expect("plant clock must be anchored");
        assert!(
            last_update >= resume_before,
            "plant clock must be re-anchored on unfreeze"
        );
    }

    #[test]
    fn clearing_override_reanchors_curve_and_plant_clocks() {
        let mut source = SimulatedSensorSource::default_curve()
            .with_thermal_plant(ThermalPlantConfig::default());
        let start_before = source.start_instant;
        source.set_temperature_override(Some((123.0, 234.0)));
        std::thread::sleep(std::time::Duration::from_millis(150));
        let resume_before = Instant::now();
        source.set_temperature_override(None);

        let shifted_ms = source
            .start_instant
            .saturating_duration_since(start_before)
            .as_millis();
        assert!(
            shifted_ms >= 100,
            "curve clock should resume at the pause point, shifted {shifted_ms}ms"
        );
        let last_update = source
            .last_update_instant
            .expect("plant clock must be anchored");
        assert!(
            last_update >= resume_before,
            "plant clock must be re-anchored when the override clears"
        );
    }
    #[test]
    fn simulation_fault_masks_reach_snapshot_and_reset_clears_them() {
        let mut source = SimulatedSensorSource::default_curve();
        source.set_fault_masks(0x21, 0x80);
        let snap = source.snapshot();
        assert_eq!((snap.bean_fault, snap.env_fault), (0x21, 0x80));
        assert!(!snap.has_plant);

        source.reset();
        let snap = source.snapshot();
        assert_eq!((snap.bean_fault, snap.env_fault), (0, 0));
    }

    #[test]
    fn simulation_snapshot_tracks_plant_actuators() {
        let mut source = SimulatedSensorSource::default_curve()
            .with_thermal_plant(ThermalPlantConfig::default());
        source.set_heater_fan(80.0, 20.0);
        let _ = source.current_temperatures();
        let snap = source.snapshot();
        assert!(snap.has_plant);
        assert_eq!(snap.heater_pct, 80.0);
        assert_eq!(snap.fan_pct, 20.0);
    }
}
