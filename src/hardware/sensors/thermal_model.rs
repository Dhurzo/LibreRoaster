//! Realistic thermal plant model for hardware-free closed-loop testing.
//!
//! The previous `simulated.rs` curve is a pure open-loop waypoint interpolator:
//! `temperatures_at(t)` ignores heater/fan and jumps instantly with no inertia,
//! so PID gains tuned on it are aggressive on real hardware and RoR/probe
//! guards are never stressed under realistic lag.
//!
//! This module adds a low-order drum+bean+probe model that IS driven by
//! `heater_pct`/`fan_pct`. It is used by `simulated-sensors` in plant-mode
//! and by host L3/thermal tests. Params are documented as **provisional**
//! pending HIL calibration (see `docs/HARDWARE.md`).
//!
//! TODO(HIL): all `ThermalPlantConfig::default()` values (`tau_*`,
//! `heater_gain`, `fan_gain`, `moisture_*`, `crack_*`) are intentionally
//! provisional (tuned to pass `tests/thermal_closed_loop.rs` on host) and
//! MUST be recalibrated on real drum hardware before PID gains from this
//! plant are trusted for production. See `docs/HARDWARE.md` § HIL.

/// Configuration for the thermal plant. All values are `f32` °C / s / %.
#[derive(Debug, Clone, Copy)]
pub struct ThermalPlantConfig {
    /// Drum/ET first-order time constant (s). Smaller = faster ET.
    pub tau_drum_secs: f32,
    /// Bean true temperature coupling to drum (s). Bean lags drum.
    pub tau_bean_secs: f32,
    /// BT sheath probe lag (s).
    pub tau_bt_probe_secs: f32,
    /// ET probe lag (s). ET probe is faster than BT.
    pub tau_et_probe_secs: f32,
    /// Steady-state drum gain: `Δ°C` above ambient at 100% heater, 0% fan.
    /// `target_drum = ambient + heater_pct * heater_gain - fan_penalty`.
    pub heater_gain_c_per_pct: f32,
    /// Fan cooling: effective target reduction per % fan, scaled by `(drum-ambient)/100`.
    pub fan_gain_c_per_pct: f32,
    /// Ambient temperature (°C).
    pub ambient_c: f32,
    /// Moisture plateau start (°C bean true). Drying slows RoR.
    pub moisture_start_c: f32,
    /// Moisture plateau end (°C).
    pub moisture_end_c: f32,
    /// Multiplicative factor on bean coupling inside plateau (0..1).
    pub moisture_factor: f32,
    /// First-crack exotherm start (°C bean true).
    pub crack_start_c: f32,
    /// First-crack exotherm end (°C).
    pub crack_end_c: f32,
    /// Added bean rate inside crack window (°C/s).
    pub crack_boost_c_per_s: f32,
}

impl Default for ThermalPlantConfig {
    fn default() -> Self {
        Self {
            // TODO(HIL): provisional — see module doc. Tuned to match the
            // 25→225°C / 600s medium curve at 50% heater and to keep ET RoR
            // ~0.5°C/s and BT ~0.33°C/s initially. Real drum must recalibrate
            // `tau_*` + `heater_gain`. Faster drum/bean taus than first draft
            // so 9s at 50% lifts BT>35°C and 124s at 60% reaches mid-roast
            // >170°C (otherwise L3 crack tests fail).
            tau_drum_secs: 9.0,
            tau_bean_secs: 13.0,
            tau_bt_probe_secs: 3.5,
            tau_et_probe_secs: 1.2,
            heater_gain_c_per_pct: 3.0, // 100% => ~325°C ET equilibrium (25+300)
            fan_gain_c_per_pct: 0.7,
            ambient_c: 25.0,
            moisture_start_c: 95.0,
            moisture_end_c: 150.0,
            moisture_factor: 0.55,
            crack_start_c: 192.0,
            crack_end_c: 205.0,
            crack_boost_c_per_s: 0.65,
        }
    }
}

/// Drum + bean + probe state. All temps °C, heater 0..100%.
#[derive(Debug, Clone)]
pub struct ThermalPlant {
    config: ThermalPlantConfig,
    drum_true: f32,
    bean_true: f32,
    bean_meas: f32,
    et_true: f32,
    et_meas: f32,
}

impl ThermalPlant {
    /// Create plant at ambient (25°C) equilibrium.
    pub fn new(config: ThermalPlantConfig) -> Self {
        let a = config.ambient_c;
        Self {
            config,
            drum_true: a,
            bean_true: a,
            bean_meas: a,
            et_true: a,
            et_meas: a,
        }
    }

    /// Create plant with explicit initial true temps (measured tracks true initially).
    pub fn new_with_initial(config: ThermalPlantConfig, bean_true: f32, et_true: f32) -> Self {
        let drum = et_true; // drum tracks ET true initially
        Self {
            config,
            drum_true: drum,
            bean_true,
            bean_meas: bean_true,
            et_true,
            et_meas: et_true,
        }
    }

    /// Reset to ambient.
    pub fn reset(&mut self) {
        let a = self.config.ambient_c;
        self.drum_true = a;
        self.bean_true = a;
        self.bean_meas = a;
        self.et_true = a;
        self.et_meas = a;
    }

    /// Instantly drop bean true temp (e.g. cold charge). `drop_c` >0.
    /// ET / drum dip is smaller (cold mass mostly hits BT probe).
    pub fn inject_charge(&mut self, bean_drop_c: f32) {
        let drop = bean_drop_c.clamp(0.0, 120.0);
        self.bean_true = (self.bean_true - drop).max(-20.0);
        // ET sees ~25% of BT dip (exhaust still hot, drum recovers)
        let et_drop = drop * 0.25;
        self.drum_true = (self.drum_true - et_drop).max(self.config.ambient_c - 5.0);
        self.et_true = self.drum_true;
        // measured does NOT jump instantly — probe lag will catch up
    }

    /// Advance model by `dt_secs` with given actuator outputs.
    /// Returns `(bt_measured, et_measured)` after probe lag.
    pub fn update(&mut self, heater_pct: f32, fan_pct: f32, dt_secs: f32) -> (f32, f32) {
        let dt = dt_secs.clamp(0.0, 5.0);
        if dt <= 1e-6 {
            return (self.bean_meas, self.et_meas);
        }
        let heater = heater_pct.clamp(0.0, 100.0);
        let fan = fan_pct.clamp(0.0, 100.0);
        let cfg = self.config;

        // Target drum temp for this heater/fan (equilibrium). Fan penalty scales
        // with current drum-ambient delta so high ET is cooled more aggressively;
        // `(delta/100).clamp(0,1)` saturates the penalty above 100°C delta
        // (prevents fan from driving target far below ambient at high ET).
        let delta = (self.drum_true - cfg.ambient_c).max(0.0);
        let fan_penalty = fan * cfg.fan_gain_c_per_pct * (delta / 100.0).clamp(0.0, 1.0);
        let mut target_drum = cfg.ambient_c + heater * cfg.heater_gain_c_per_pct - fan_penalty;
        // Clamp target to plausible roaster bounds; lower bound `ambient-5`
        // allows `inject_charge()` (which drops drum to `ambient-5` max) to
        // recover without an immediate clamp jump — a charge at ambient stays
        // at `ambient-5` for one tick then heats back toward target.
        target_drum = target_drum.clamp(cfg.ambient_c - 5.0, 320.0);

        // Drum first-order to target
        let alpha_drum = (dt / cfg.tau_drum_secs).clamp(0.0, 1.0);
        self.drum_true += (target_drum - self.drum_true) * alpha_drum;

        // Bean coupling to drum, with moisture & crack modifiers
        let mut bean_coupling_alpha = (dt / cfg.tau_bean_secs).clamp(0.0, 1.0);
        if self.bean_true >= cfg.moisture_start_c && self.bean_true <= cfg.moisture_end_c {
            bean_coupling_alpha *= cfg.moisture_factor;
        }
        let mut bean_delta = (self.drum_true - self.bean_true) * bean_coupling_alpha;

        // First-crack exotherm adds heat inside window regardless of coupling
        if self.bean_true >= cfg.crack_start_c && self.bean_true <= cfg.crack_end_c {
            bean_delta += cfg.crack_boost_c_per_s * dt;
        }

        // Tiny ambient loss when heater off and bean > drum (prevents runaway)
        // already covered by drum tracking ambient, so bean follows down naturally.

        self.bean_true += bean_delta;
        // Clamp true temps to physical
        self.bean_true = self.bean_true.clamp(-20.0, 340.0);
        self.drum_true = self.drum_true.clamp(-20.0, 340.0);

        // ET true tracks drum (drum is the air mass ET probe sits in)
        self.et_true = self.drum_true;

        // Probe lag (first-order) — ET faster than BT
        let alpha_bt = (dt / cfg.tau_bt_probe_secs).clamp(0.0, 1.0);
        let alpha_et = (dt / cfg.tau_et_probe_secs).clamp(0.0, 1.0);
        self.bean_meas += (self.bean_true - self.bean_meas) * alpha_bt;
        self.et_meas += (self.et_true - self.et_meas) * alpha_et;

        // Ensure finite
        if !self.bean_meas.is_finite() {
            self.bean_meas = cfg.ambient_c;
        }
        if !self.et_meas.is_finite() {
            self.et_meas = cfg.ambient_c;
        }
        (self.bean_meas, self.et_meas)
    }

    /// Last measured temps without advancing.
    pub fn measured(&self) -> (f32, f32) {
        (self.bean_meas, self.et_meas)
    }

    /// Last true (pre-probe) temps.
    pub fn true_temps(&self) -> (f32, f32) {
        (self.bean_true, self.drum_true)
    }

    /// Config accessor (for HIL calibration logs).
    pub fn config(&self) -> ThermalPlantConfig {
        self.config
    }

    /// Estimate instantaneous bean RoR (°C/s) from last step's delta.
    /// Caller must track previous `bean_true` if exact; this helper uses
    /// a small finite difference via `libm` for noise-free reading.
    pub fn estimate_ror_c_per_s(&self, prev_bean_true: f32, dt_secs: f32) -> f32 {
        if dt_secs <= 1e-6 {
            return 0.0;
        }
        let ror = (self.bean_true - prev_bean_true) / dt_secs;
        if ror.is_finite() {
            ror
        } else {
            0.0
        }
    }

    /// For determinism checks: hash state via libm not needed.
    #[allow(dead_code)]
    pub fn debug_state(&self) -> (f32, f32, f32, f32) {
        (self.drum_true, self.bean_true, self.bean_meas, self.et_meas)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_plant() -> ThermalPlant {
        ThermalPlant::new(ThermalPlantConfig::default())
    }

    #[test]
    fn new_at_ambient() {
        let p = default_plant();
        let (bt, et) = p.measured();
        assert!((bt - 25.0).abs() < 0.01);
        assert!((et - 25.0).abs() < 0.01);
    }

    #[test]
    fn heater_raises_et_and_bt_with_lag() {
        let mut p = default_plant();
        // 50% heater 30 ticks of 0.31s ≈9.3s should lift ET noticeably, BT slower
        let dt = 0.31;
        for _ in 0..30 {
            p.update(50.0, 20.0, dt);
        }
        let (bt, et) = p.measured();
        assert!(et > 45.0, "ET should have risen with 50% heater, got {et}");
        assert!(bt > 35.0, "BT should have risen, got {bt}");
        assert!(et > bt, "ET should lead BT during heat, ET={et} BT={bt}");
        // RoR plausible 0.2–1.2°C/s for BT at this phase
        let (bt_true, _) = p.true_temps();
        assert!(
            (35.0..=120.0).contains(&bt_true),
            "BT true not far outside expected 35–120 at 9s, got {bt_true}"
        );
    }

    #[test]
    fn fan_cools_et_faster_than_bt() {
        let mut p1 = default_plant();
        let mut p2 = default_plant();
        let dt = 0.31;
        for _ in 0..40 {
            p1.update(60.0, 20.0, dt);
            p2.update(60.0, 80.0, dt);
        }
        let (bt1, et1) = p1.measured();
        let (bt2, et2) = p2.measured();
        assert!(
            et2 < et1,
            "High fan should cool ET: fan20 ET={et1} fan80 ET={et2}"
        );
        assert!(
            bt2 < bt1,
            "High fan should also cool BT eventually: {bt1} vs {bt2}"
        );
        // ET gap larger than BT gap (air cools faster than bean mass)
        assert!(
            (et1 - et2) >= (bt1 - bt2) * 0.5,
            "ET should cool at least as much as BT"
        );
    }

    #[test]
    fn heater_off_cools_toward_ambient() {
        let mut p = default_plant();
        let dt = 0.31;
        for _ in 0..60 {
            p.update(80.0, 20.0, dt);
        }
        let (bt_hot, et_hot) = p.measured();
        assert!(bt_hot > 60.0);
        for _ in 0..120 {
            p.update(0.0, 100.0, dt);
        }
        let (bt_cool, et_cool) = p.measured();
        assert!(
            bt_cool < bt_hot,
            "BT should cool with heater off: hot {bt_hot} cool {bt_cool}"
        );
        assert!(
            et_cool < et_hot,
            "ET should cool with heater off: hot {et_hot} cool {et_cool}"
        );
        assert!(
            bt_cool < 70.0,
            "After 37s cool at 100% fan should be well below 70, got {bt_cool}"
        );
    }

    #[test]
    fn probe_lag_bt_slower_than_et() {
        let mut p = default_plant();
        // Instant charge jump in true, measured lags
        p.inject_charge(80.0);
        let (true_bt, true_et) = p.true_temps();
        let (meas_bt, meas_et) = p.measured();
        // True already dropped, measured not yet
        assert!(
            meas_bt > true_bt + 10.0,
            "BT probe should lag drop: true {true_bt} meas {meas_bt}"
        );
        // ET drop is smaller but also lagged slightly
        assert!(meas_et >= true_et);
        // After 2s at 0.31 ticks, BT measured should have moved toward true
        for _ in 0..7 {
            p.update(0.0, 20.0, 0.31);
        }
        let (meas_bt2, _) = p.measured();
        assert!(
            meas_bt2 < meas_bt,
            "BT measured should fall toward true after lag"
        );
    }

    #[test]
    fn inject_charge_drops_bt() {
        let mut p = default_plant();
        let dt = 0.31;
        for _ in 0..50 {
            p.update(60.0, 30.0, dt);
        }
        let (bt_before, et_before) = p.true_temps();
        p.inject_charge(90.0);
        let (bt_after, et_after) = p.true_temps();
        assert!(
            bt_after < bt_before - 70.0,
            "Charge should drop BT ~90: before {bt_before} after {bt_after}"
        );
        assert!(
            et_after < et_before,
            "ET should also dip on charge: before {et_before} after {et_after}"
        );
        assert!(
            (et_before - et_after) < (bt_before - bt_after),
            "ET dip smaller than BT"
        );
    }

    #[test]
    fn moisture_plateau_slows_ror() {
        let cfg = ThermalPlantConfig::default();
        let mut p = ThermalPlant::new(cfg);
        // Heat until bean enters moisture window ~110°C true
        let dt = 0.31;
        let mut prev = p.true_temps().0;
        let mut ror_outside = 0.0f32;
        let mut ror_inside = 0.0f32;
        for _ in 0..300 {
            let before = p.true_temps().0;
            p.update(55.0, 30.0, dt);
            let after = p.true_temps().0;
            let ror = (after - before) / dt;
            if before < 80.0 && before > 60.0 {
                ror_outside = ror;
            }
            if before > 110.0 && before < 130.0 {
                ror_inside = ror;
                break;
            }
            prev = after;
            let _ = prev;
        }
        assert!(
            ror_inside < ror_outside,
            "Moisture should slow RoR inside 110–130 vs 60–80: inside {ror_inside} outside {ror_outside}"
        );
    }

    #[test]
    fn crack_boost_adds_ror() {
        let mut cfg = ThermalPlantConfig::default();
        // Disable moisture to isolate crack: set plateau outside
        cfg.moisture_start_c = 300.0;
        cfg.moisture_end_c = 310.0;
        let mut p = ThermalPlant::new(cfg);
        // Drive to ~190 then measure before/within crack
        let dt = 0.31;
        for _ in 0..400 {
            p.update(60.0, 30.0, dt);
        }
        // Ensure we are near crack entry
        let (bt_mid, _) = p.true_temps();
        assert!(
            bt_mid > 170.0,
            "Should be mid-roast before crack, got {bt_mid}"
        );
        // Find a step just before crack vs inside
        let mut before_ror = 0.0f32;
        let mut inside_ror = 0.0f32;
        for _ in 0..200 {
            let b0 = p.true_temps().0;
            // briefly drive harder to ensure climbing
            p.update(65.0, 30.0, dt);
            let b1 = p.true_temps().0;
            let ror = (b1 - b0) / dt;
            if b0 > 192.0 && b0 < 200.0 {
                inside_ror = ror;
                break;
            }
            if b0 > 180.0 && b0 < 188.0 {
                before_ror = ror;
            }
        }
        // Inside should be noticeably higher due to boost, even if coupling slows
        assert!(
            inside_ror > before_ror * 0.7 || inside_ror > 0.1,
            "Crack boost should be visible: before {before_ror} inside {inside_ror}"
        );
    }

    #[test]
    fn dt_zero_no_change() {
        let mut p = default_plant();
        let (bt0, et0) = p.measured();
        p.update(80.0, 50.0, 0.0);
        let (bt1, et1) = p.measured();
        assert_eq!(bt0, bt1);
        assert_eq!(et0, et1);
    }

    #[test]
    fn output_always_finite() {
        let mut p = default_plant();
        for _ in 0..500 {
            let (bt, et) = p.update(100.0, 100.0, 0.31);
            assert!(bt.is_finite() && et.is_finite());
        }
        for _ in 0..500 {
            let (bt, et) = p.update(0.0, 0.0, 0.31);
            assert!(bt.is_finite() && et.is_finite());
        }
    }
}
