//! CORE-6 (plan CORE-2026-10-09): the probe-stuck detector as one unit.
//!
//! A hard thermocouple short reads a flat ~0 °C, which is a VALID temperature
//! (no MAX31856 fault bit), so the fault/NaN paths never fire and the heater
//! would run blind. While the heater runs, BT must move by more than
//! `PROBE_STUCK_VARIATION_C` within the window; otherwise the probe is dead.
//!
//! Pure logic: no hardware, no clock, no wire output. `RoasterControl`
//! decides WHEN the detector is armed (`GuardArming::probe_stuck_mode`, the
//! equilibrium exemption) and EXECUTES the verdict (wire warning, latch).
//!
//! | Mode | Flat BT | Result |
//! |---|---|---|
//! | Firmware PID | `PROBE_STUCK_TIMEOUT_SECS` (120 s) | latch (`Rule::Pid`) |
//! | Manual / Artisan software PID | 120 s | one warning (`Rule::Manual`) |
//! | Manual / Artisan software PID | `PROBE_STUCK_MANUAL_LATCH_SECS` (300 s) | latch (`Rule::Manual`) |
//! | Equilibrium (BT hot, ET flat), any mode | — | clock re-anchors (no latch) … |
//! | … but firmware PID at ≥ `PROBE_STUCK_PID_PLATEAU_MIN_DUTY_PCT` (N4) | 300 s / 600 s of plateau | warning / latch (`Rule::Plateau`) |

use crate::config::constants::{
    PROBE_STUCK_MANUAL_LATCH_SECS, PROBE_STUCK_PID_PLATEAU_LATCH_SECS,
    PROBE_STUCK_PID_PLATEAU_MIN_DUTY_PCT, PROBE_STUCK_PID_PLATEAU_WARN_SECS,
    PROBE_STUCK_TIMEOUT_SECS, PROBE_STUCK_VARIATION_C,
};
use embassy_time::Instant;

/// Which rule produced a warning or a latch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rule {
    /// Firmware PID, single stage (latch at 120 s).
    Pid,
    /// Manual / Artisan software PID, two stage (warn 120 s, latch 300 s).
    Manual,
    /// N4: firmware-PID hot plateau (warn 300 s, latch 600 s at ≥ 50 % duty).
    Plateau,
}

/// What the caller must do on this tick, in this order: send the warning
/// line, then latch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Verdict {
    pub(crate) warning: Option<Rule>,
    pub(crate) latch: Option<Rule>,
}

/// One tick of input, all read by the caller after the heater write.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ProbeStuckInput {
    pub(crate) now: Instant,
    /// Applied heater duty (%) — the detector only runs while it is > 0.
    pub(crate) duty: f32,
    pub(crate) bean_temp: f32,
    pub(crate) env_temp: f32,
    /// `GuardArming::probe_stuck_mode` (false while the PID regulates).
    pub(crate) armed: bool,
    /// `GuardArming::probe_stuck_equilibrium_exempt`.
    pub(crate) equilibrium_exempt: bool,
    /// `SystemStatus::pid_enabled`.
    pub(crate) pid_enabled: bool,
}

/// Detector state for the current stuck episode.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ProbeStuckDetector {
    last_bt: Option<f32>,
    last_change: Option<Instant>,
    /// ET anchor for the equilibrium discriminator (R2): set where `last_bt`
    /// is set. While BT stays flat, ET must stay within
    /// `PROBE_STUCK_ET_FLAT_C` of it to count as equilibrium.
    et_anchor: Option<f32>,
    /// Manual two-stage: the warning line was sent for this episode.
    warning_sent: bool,
    /// N4: first tick of the PID plateau at ≥ the observable duty.
    plateau_since: Option<Instant>,
    /// N4: plateau warning already sent.
    plateau_warning_sent: bool,
}

impl ProbeStuckDetector {
    /// Forget the current episode (N1: no inherited clock across a mode change).
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    /// ET anchor read by `RoasterControl::guard_arming` for the equilibrium test.
    pub(crate) fn et_anchor(&self) -> Option<f32> {
        self.et_anchor
    }

    /// Start a new episode at this BT/ET.
    fn anchor(&mut self, bt: f32, et: f32, now: Instant) {
        self.last_bt = Some(bt);
        self.last_change = Some(now);
        self.et_anchor = Some(et);
        self.warning_sent = false;
        self.plateau_since = None;
        self.plateau_warning_sent = false;
    }

    /// One control tick.
    pub(crate) fn tick(&mut self, i: ProbeStuckInput) -> Verdict {
        let mut v = Verdict::default();
        if !(i.duty > 0.0 && i.bean_temp.is_finite() && i.armed) {
            // Heater off, BT faulted, or PID regulating — disarm.
            self.reset();
            return v;
        }
        let Some(prev) = self.last_bt else {
            self.anchor(i.bean_temp, i.env_temp, i.now);
            return v;
        };
        if (i.bean_temp - prev).abs() > PROBE_STUCK_VARIATION_C {
            self.anchor(i.bean_temp, i.env_temp, i.now);
        } else if i.equilibrium_exempt {
            // Both probes flat with BT hot: equilibrium. Re-anchor the clock
            // so that when ET starts moving BT gets the full window to respond.
            self.last_change = Some(i.now);
            self.warning_sent = false;
            // N4: in firmware-PID mode the exemption is bounded while the
            // heater is at or above the observable duty.
            if i.pid_enabled && i.duty >= PROBE_STUCK_PID_PLATEAU_MIN_DUTY_PCT {
                let since = *self.plateau_since.get_or_insert(i.now);
                let plateau_secs = i.now.saturating_duration_since(since).as_secs();
                if plateau_secs >= PROBE_STUCK_PID_PLATEAU_WARN_SECS && !self.plateau_warning_sent {
                    self.plateau_warning_sent = true;
                    v.warning = Some(Rule::Plateau);
                }
                if plateau_secs >= PROBE_STUCK_PID_PLATEAU_LATCH_SECS {
                    v.latch = Some(Rule::Plateau);
                }
            } else {
                self.plateau_since = None;
                self.plateau_warning_sent = false;
            }
        } else if let Some(last_change) = self.last_change {
            // Exemption ended (ET moved): the plateau bound is moot.
            self.plateau_since = None;
            self.plateau_warning_sent = false;
            let flat_secs = i.now.saturating_duration_since(last_change).as_secs();
            if flat_secs >= PROBE_STUCK_TIMEOUT_SECS {
                if i.pid_enabled {
                    // Firmware PID: single-stage latch.
                    v.latch = Some(Rule::Pid);
                } else {
                    // Manual / software PID: one warning, then the real latch.
                    if !self.warning_sent {
                        self.warning_sent = true;
                        v.warning = Some(Rule::Manual);
                    }
                    if flat_secs >= PROBE_STUCK_MANUAL_LATCH_SECS {
                        v.latch = Some(Rule::Manual);
                    }
                }
            }
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(secs: u64, bt: f32, et: f32) -> ProbeStuckInput {
        ProbeStuckInput {
            now: Instant::from_secs(1000 + secs),
            duty: 60.0,
            bean_temp: bt,
            env_temp: et,
            armed: true,
            equilibrium_exempt: false,
            pid_enabled: false,
        }
    }

    /// Run the detector once per second from `from` to `to` (inclusive) and
    /// return the first (second, verdict) with something in it.
    fn first_event(
        d: &mut ProbeStuckDetector,
        from: u64,
        to: u64,
        f: impl Fn(u64) -> ProbeStuckInput,
    ) -> Option<(u64, Verdict)> {
        for s in from..=to {
            let v = d.tick(f(s));
            if v != Verdict::default() {
                return Some((s, v));
            }
        }
        None
    }

    #[test]
    fn manual_mode_warns_at_120_s_then_latches_at_300_s() {
        let mut d = ProbeStuckDetector::default();
        let w = first_event(&mut d, 0, 400, |s| input(s, 150.0, 170.0 + s as f32));
        assert_eq!(
            w,
            Some((
                PROBE_STUCK_TIMEOUT_SECS,
                Verdict {
                    warning: Some(Rule::Manual),
                    latch: None
                }
            ))
        );
        let l = first_event(&mut d, PROBE_STUCK_TIMEOUT_SECS + 1, 400, |s| {
            input(s, 150.0, 170.0 + s as f32)
        });
        assert_eq!(
            l,
            Some((
                PROBE_STUCK_MANUAL_LATCH_SECS,
                Verdict {
                    warning: None,
                    latch: Some(Rule::Manual)
                }
            ))
        );
    }

    #[test]
    fn firmware_pid_latches_at_120_s_without_warning() {
        let mut d = ProbeStuckDetector::default();
        let e = first_event(&mut d, 0, 400, |s| ProbeStuckInput {
            pid_enabled: true,
            ..input(s, 150.0, 170.0 + s as f32)
        });
        assert_eq!(
            e,
            Some((
                PROBE_STUCK_TIMEOUT_SECS,
                Verdict {
                    warning: None,
                    latch: Some(Rule::Pid)
                }
            ))
        );
    }

    #[test]
    fn bt_movement_restarts_the_episode() {
        let mut d = ProbeStuckDetector::default();
        let e = first_event(&mut d, 0, 400, |s| {
            let bt = if s >= 100 { 152.0 } else { 150.0 };
            input(s, bt, 170.0 + s as f32)
        });
        assert_eq!(e.map(|(s, _)| s), Some(100 + PROBE_STUCK_TIMEOUT_SECS));
    }

    #[test]
    fn disarmed_ticks_reset_the_episode() {
        let mut d = ProbeStuckDetector::default();
        assert_eq!(
            first_event(&mut d, 0, 100, |s| input(s, 150.0, 170.0)),
            None
        );
        let _ = d.tick(ProbeStuckInput {
            duty: 0.0,
            ..input(101, 150.0, 170.0)
        });
        assert!(d.et_anchor().is_none(), "heater off resets");
        let e = first_event(&mut d, 102, 400, |s| input(s, 150.0, 170.0));
        assert_eq!(e.map(|(s, _)| s), Some(102 + PROBE_STUCK_TIMEOUT_SECS));
    }

    #[test]
    fn equilibrium_never_latches_in_manual_but_the_pid_plateau_is_bounded() {
        let mut d = ProbeStuckDetector::default();
        let eq = |s| ProbeStuckInput {
            equilibrium_exempt: true,
            ..input(s, 200.0, 220.0)
        };
        assert_eq!(first_event(&mut d, 0, 2000, eq), None);
        let mut p = ProbeStuckDetector::default();
        let _ = p.tick(input(0, 200.0, 220.0)); // anchor the episode
        let pid_eq = |s| ProbeStuckInput {
            equilibrium_exempt: true,
            pid_enabled: true,
            ..input(s, 200.0, 220.0)
        };
        let w = first_event(&mut p, 1, 2000, pid_eq);
        assert_eq!(
            w,
            Some((
                1 + PROBE_STUCK_PID_PLATEAU_WARN_SECS,
                Verdict {
                    warning: Some(Rule::Plateau),
                    latch: None
                }
            ))
        );
        let l = first_event(&mut p, 2 + PROBE_STUCK_PID_PLATEAU_WARN_SECS, 2000, pid_eq);
        assert_eq!(
            l,
            Some((
                1 + PROBE_STUCK_PID_PLATEAU_LATCH_SECS,
                Verdict {
                    warning: None,
                    latch: Some(Rule::Plateau)
                }
            ))
        );
    }

    #[test]
    fn nan_bt_disarms() {
        let mut d = ProbeStuckDetector::default();
        assert_eq!(
            first_event(&mut d, 0, 400, |s| input(s, f32::NAN, 170.0)),
            None
        );
    }
}
