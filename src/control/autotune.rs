//! Step-test PID autotune (E4).
//!
//! Pure logic — no hardware, no clock, no globals. Procedure:
//! 1. **Baseline** (`TUNE_BASELINE_SECS`): heater held at the operator's
//!    manual duty `base`; the BT drift `r0` (°C/s) is measured by linear
//!    regression.
//! 2. **Step**: heater raised to `base + step`. The dead time is the moment BT
//!    rises `TUNE_DEADTIME_RISE_C` above the baseline extrapolation; the new
//!    slope `r1` is regressed over the next `TUNE_SLOPE_WINDOW_SECS`.
//! 3. **Result**: integrating-plant model `k = (r1 - r0) / step`
//!    (°C/s per %), dead time `θ`. SIMC rules with `τc = 2θ`:
//!    `Kp = 1 / (k·(τc+θ))`, `Ti = 4·(τc+θ)`, `Ki = Kp/Ti`, `Kd = 0`.
//!
//! The caller (`RoasterControl`) owns every safety decision: it aborts the
//! test on any operator command, on the safety latch and on a mode change.
//! This module adds its own over-temperature abort and refuses implausible
//! results instead of applying them.

use crate::config::constants::OVERTEMP_THRESHOLD;

/// Baseline phase length (s).
pub const TUNE_BASELINE_SECS: f32 = 60.0;
/// BT rise (°C) above the baseline extrapolation that marks the dead time.
pub const TUNE_DEADTIME_RISE_C: f32 = 1.0;
/// Regression window (s) after the dead time.
pub const TUNE_SLOPE_WINDOW_SECS: f32 = 60.0;
/// Hard limit (s) for the step phase.
pub const TUNE_STEP_MAX_SECS: f32 = 300.0;
/// Allowed manual base duty (%).
pub const TUNE_MIN_BASE_DUTY: f32 = 10.0;
pub const TUNE_MAX_BASE_DUTY: f32 = 80.0;
/// Allowed step size (%).
pub const TUNE_MIN_STEP: f32 = 5.0;
pub const TUNE_MAX_STEP: f32 = 40.0;
/// BT (°C) required to start: the drum must be warm and the probe alive.
pub const TUNE_MIN_BT_C: f32 = 60.0;
/// The test aborts this many °C below the BT over-temperature cutoff.
pub const TUNE_BT_ABORT_MARGIN_C: f32 = 30.0;
/// Plausible gain range for the result; anything else is refused.
pub const TUNE_KP_MIN: f32 = 0.1;
pub const TUNE_KP_MAX: f32 = 100.0;

/// Identified model and the PID gains derived from it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TuneResult {
    pub kp: f32,
    pub ki: f32,
    pub kd: f32,
    /// Integrating gain `k` (°C/s per % of heater).
    pub gain: f32,
    /// Dead time `θ` (s).
    pub dead_time_secs: f32,
}

/// Why a step test could not start or did not produce gains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuneError {
    BadBaseDuty,
    BadStep,
    ProbeCold,
    NoResponse,
    Implausible,
    TooHot,
}

impl TuneError {
    /// Stable wire token (sent as `ERR tune_<code>`).
    pub fn code(&self) -> &'static str {
        match self {
            TuneError::BadBaseDuty => "tune_bad_base_duty",
            TuneError::BadStep => "tune_bad_step",
            TuneError::ProbeCold => "tune_probe_cold",
            TuneError::NoResponse => "tune_no_response",
            TuneError::Implausible => "tune_implausible",
            TuneError::TooHot => "tune_too_hot",
        }
    }
}

/// What the caller must do after one `StepTest::tick`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TuneTick {
    /// Keep going; drive the heater at this duty (%).
    Drive(f32),
    /// Finished: apply these gains, release the heater to the manual value.
    Done(TuneResult),
    /// Finished without gains: release the heater to the manual value.
    Failed(TuneError),
}

/// Running sums for an ordinary least-squares slope (no buffers).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Regression {
    n: f32,
    st: f32,
    sy: f32,
    stt: f32,
    sty: f32,
}

impl Regression {
    fn add(&mut self, t: f32, y: f32) {
        self.n += 1.0;
        self.st += t;
        self.sy += y;
        self.stt += t * t;
        self.sty += t * y;
    }

    fn slope(&self) -> Option<f32> {
        if self.n < 3.0 {
            return None;
        }
        let den = self.n * self.stt - self.st * self.st;
        if den.abs() < 1e-6 {
            return None;
        }
        let s = (self.n * self.sty - self.st * self.sy) / den;
        s.is_finite().then_some(s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Baseline,
    Step {
        r0: f32,
        t0: f32,
        bt0: f32,
        crossing: Option<f32>,
    },
}

/// One step test. Times are seconds since the test started.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepTest {
    base: f32,
    step: f32,
    phase: Phase,
    reg: Regression,
}

impl StepTest {
    /// Validate the preconditions and build the test. `bt` = current BT (°C).
    pub fn new(base: f32, step: f32, bt: f32) -> Result<Self, TuneError> {
        if !base.is_finite() || !(TUNE_MIN_BASE_DUTY..=TUNE_MAX_BASE_DUTY).contains(&base) {
            return Err(TuneError::BadBaseDuty);
        }
        if !step.is_finite() || !(TUNE_MIN_STEP..=TUNE_MAX_STEP).contains(&step) || base + step > 100.0
        {
            return Err(TuneError::BadStep);
        }
        if !bt.is_finite() || bt < TUNE_MIN_BT_C {
            return Err(TuneError::ProbeCold);
        }
        if bt >= OVERTEMP_THRESHOLD - TUNE_BT_ABORT_MARGIN_C {
            return Err(TuneError::TooHot);
        }
        Ok(Self {
            base,
            step,
            phase: Phase::Baseline,
            reg: Regression::default(),
        })
    }

    /// Heater duty (%) the test is driving right now.
    pub fn current_drive(&self) -> f32 {
        match self.phase {
            Phase::Baseline => self.base,
            Phase::Step { .. } => self.base + self.step,
        }
    }

    /// Advance one control tick. `t` = seconds since start, `bt` = BT (°C).
    pub fn tick(&mut self, t: f32, bt: f32) -> TuneTick {
        if !bt.is_finite() || !t.is_finite() {
            return TuneTick::Failed(TuneError::NoResponse);
        }
        if bt >= OVERTEMP_THRESHOLD - TUNE_BT_ABORT_MARGIN_C {
            return TuneTick::Failed(TuneError::TooHot);
        }
        match self.phase {
            Phase::Baseline => {
                self.reg.add(t, bt);
                if t < TUNE_BASELINE_SECS {
                    return TuneTick::Drive(self.base);
                }
                let Some(r0) = self.reg.slope() else {
                    return TuneTick::Failed(TuneError::NoResponse);
                };
                self.phase = Phase::Step {
                    r0,
                    t0: t,
                    bt0: bt,
                    crossing: None,
                };
                self.reg = Regression::default();
                TuneTick::Drive(self.base + self.step)
            }
            Phase::Step {
                r0,
                t0,
                bt0,
                crossing,
            } => {
                let ts = t - t0;
                if ts > TUNE_STEP_MAX_SECS {
                    return TuneTick::Failed(TuneError::NoResponse);
                }
                match crossing {
                    None => {
                        let expected = bt0 + r0 * ts;
                        if bt - expected >= TUNE_DEADTIME_RISE_C {
                            self.phase = Phase::Step {
                                r0,
                                t0,
                                bt0,
                                crossing: Some(ts),
                            };
                            self.reg.add(ts, bt);
                        }
                        TuneTick::Drive(self.base + self.step)
                    }
                    Some(tc) => {
                        self.reg.add(ts, bt);
                        if ts < tc + TUNE_SLOPE_WINDOW_SECS {
                            return TuneTick::Drive(self.base + self.step);
                        }
                        match self.reg.slope() {
                            Some(r1) => match Self::gains(r0, r1, self.step, tc) {
                                Some(result) => TuneTick::Done(result),
                                None => TuneTick::Failed(TuneError::Implausible),
                            },
                            None => TuneTick::Failed(TuneError::NoResponse),
                        }
                    }
                }
            }
        }
    }

    /// SIMC gains for an integrating plant with dead time.
    fn gains(r0: f32, r1: f32, step: f32, crossing: f32) -> Option<TuneResult> {
        let dr = r1 - r0;
        let gain = dr / step;
        if !gain.is_finite() || gain <= 1e-5 {
            return None;
        }
        // The crossing happens `rise / dr` seconds after the true dead time.
        let dead_time_secs = (crossing - TUNE_DEADTIME_RISE_C / dr).max(1.0);
        let tau_c = 2.0 * dead_time_secs;
        let kp = 1.0 / (gain * (tau_c + dead_time_secs));
        let ti = 4.0 * (tau_c + dead_time_secs);
        let ki = kp / ti;
        if !kp.is_finite() || !ki.is_finite() || !(TUNE_KP_MIN..=TUNE_KP_MAX).contains(&kp) {
            return None;
        }
        Some(TuneResult {
            kp,
            ki,
            kd: 0.0,
            gain,
            dead_time_secs,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// Integrating plant with dead time: dBT/dt = r0 + k·(u(t-θ) - base).
    fn run_plant(k: f32, theta: f32, r0: f32, base: f32, step: f32) -> TuneTick {
        let dt = 0.32f32;
        let mut test = StepTest::new(base, step, 150.0).unwrap();
        let mut bt = 150.0f32;
        // Ring buffer of past heater duties to model the dead time.
        let mut history = [base; 1024];
        let delay = (theta / dt) as usize;
        let mut t = 0.0f32;
        let mut u: f32;
        for n in 0..5000usize {
            match test.tick(t, bt) {
                TuneTick::Drive(d) => u = d,
                other => return other,
            }
            history[n % 1024] = u;
            let delayed = if n >= delay {
                history[(n - delay) % 1024]
            } else {
                base
            };
            bt += (r0 + k * (delayed - base)) * dt;
            t += dt;
        }
        TuneTick::Failed(TuneError::NoResponse)
    }

    #[test]
    fn identifies_integrating_plant() {
        match run_plant(0.003, 8.0, 0.01, 40.0, 20.0) {
            TuneTick::Done(r) => {
                assert!((r.gain - 0.003).abs() < 0.0005, "gain {}", r.gain);
                assert!((r.dead_time_secs - 8.0).abs() < 2.0, "theta {}", r.dead_time_secs);
                assert!(r.kp > 5.0 && r.kp < 25.0, "kp {}", r.kp);
                assert!(r.ki > 0.0 && r.kd == 0.0);
            }
            other => panic!("expected Done, got {other:?}"),
        }
    }

    #[test]
    fn no_response_when_heater_does_nothing() {
        assert_eq!(
            run_plant(0.0, 8.0, 0.0, 40.0, 20.0),
            TuneTick::Failed(TuneError::NoResponse)
        );
    }

    #[test]
    fn preconditions_are_enforced() {
        assert_eq!(StepTest::new(5.0, 20.0, 150.0), Err(TuneError::BadBaseDuty));
        assert_eq!(StepTest::new(40.0, 50.0, 150.0), Err(TuneError::BadStep));
        assert_eq!(StepTest::new(75.0, 30.0, 150.0), Err(TuneError::BadStep));
        assert_eq!(StepTest::new(40.0, 20.0, 30.0), Err(TuneError::ProbeCold));
        assert_eq!(StepTest::new(40.0, 20.0, f32::NAN), Err(TuneError::ProbeCold));
        assert_eq!(
            StepTest::new(40.0, 20.0, OVERTEMP_THRESHOLD - 10.0),
            Err(TuneError::TooHot)
        );
    }

    #[test]
    fn too_hot_aborts_mid_test() {
        let mut test = StepTest::new(40.0, 20.0, 150.0).unwrap();
        assert!(matches!(test.tick(0.0, 150.0), TuneTick::Drive(_)));
        assert_eq!(
            test.tick(1.0, OVERTEMP_THRESHOLD - TUNE_BT_ABORT_MARGIN_C),
            TuneTick::Failed(TuneError::TooHot)
        );
    }

    #[test]
    fn drive_never_exceeds_base_plus_step() {
        let mut test = StepTest::new(40.0, 20.0, 150.0).unwrap();
        let mut bt = 150.0;
        for i in 0..2000 {
            match test.tick(i as f32 * 0.32, bt) {
                TuneTick::Drive(d) => assert!(d == 40.0 || d == 60.0),
                _ => break,
            }
            bt += 0.02;
        }
    }
}
