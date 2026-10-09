//! RoR-follow mode (E3): turn a rate-of-rise profile into a moving BT
//! setpoint for the existing firmware PID.
//!
//! Pure logic — no hardware, no clock, no globals. The caller passes the
//! seconds elapsed since the bean charge and the current BT; `RorFollower`
//! answers with the setpoint the PID must chase. Until the turning point
//! (BT stops falling after the charge) it answers `WaitingTurningPoint` and
//! the PID keeps its previous setpoint.
//!
//! Safety properties (unit-tested below):
//! - the generated setpoint never leaves `BT ± ROR_FOLLOW_MAX_LEAD_C`, so the
//!   PID can never be asked to chase a runaway target;
//! - a profile can never request more than `ROR_PROFILE_MAX_C_PER_MIN`
//!   (30 °C/min), well below the soft RoR safety guard (45 °C/min);
//! - a gap longer than `ROR_FOLLOW_MAX_STEP_SECS` (stale hold, latch, manual
//!   takeover) is not integrated, so the setpoint never jumps.

use crate::config::constants::RorProfile;

/// Maximum distance (°C) between the generated setpoint and the measured BT.
pub const ROR_FOLLOW_MAX_LEAD_C: f32 = 3.0;
/// BT must climb this far above its post-charge minimum to declare the
/// turning point.
pub const ROR_FOLLOW_TP_RISE_C: f32 = 0.5;
/// The post-charge minimum must sit at least this far below the BT seen at
/// the charge before a turning point is accepted — also on the timeout path.
/// Beans reach the probe a few seconds after the CHARGE button; a real charge
/// drops BT 40–100 °C. M-1 (audit 2026-10-06): 20 °C rejects a door/tryer
/// dip that tripped the automatic #CHARGE detector in an empty drum.
pub const ROR_FOLLOW_MIN_DROP_C: f32 = 20.0;
/// Fallback: once the charge drop was seen, start ramping this long after
/// the charge even if BT never rose `ROR_FOLLOW_TP_RISE_C` (slow probe).
/// Without the drop there is no ramp at all (M-1).
pub const ROR_FOLLOW_TP_TIMEOUT_SECS: f32 = 180.0;
/// Largest time step (s) integrated at once. Longer gaps restart the
/// integration from the current setpoint instead of jumping.
pub const ROR_FOLLOW_MAX_STEP_SECS: f32 = 2.0;
/// R-4 (audit 2026-10-09): during this many ramp steps (one per control
/// tick, ≈ 3.2 s) a `PID;SV` does not end RoR-follow — it is the tail of
/// Artisan's *PID ON* burst, not the operator moving the SV slider.
pub const ROR_FOLLOW_SV_GRACE_STEPS: u16 = 10;

/// Result of one `RorFollower::step`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RorStep {
    /// Turning point not reached yet: leave the PID setpoint alone.
    WaitingTurningPoint,
    /// Hand `sv` (°C) to the PID; `target_ror` is the profile RoR (°C/min).
    Setpoint { sv: f32, target_ror: f32 },
}

/// Setpoint generator state for one roast.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RorFollower {
    /// BT at the charge: seeded by `with_charge_bt` (BT just before the
    /// charge), otherwise the BT at the first step.
    charge_bt: Option<f32>,
    /// False for a follower created after the charge already happened
    /// (`resumed()`): the drop may be over, so only the rise is required.
    require_drop: bool,
    min_bt: f32,
    sv: Option<f32>,
    last_elapsed: Option<f32>,
    /// Ramp steps taken since the turning point (saturating).
    ramp_steps: u16,
}

impl Default for RorFollower {
    fn default() -> Self {
        Self::new()
    }
}

impl RorFollower {
    /// Fresh follower, waiting for the turning point.
    pub fn new() -> Self {
        Self {
            charge_bt: None,
            require_drop: true,
            min_bt: f32::INFINITY,
            sv: None,
            last_elapsed: None,
            ramp_steps: 0,
        }
    }

    /// Follower armed AFTER the charge (e.g. `PID;ON` after an OT1 takeover
    /// that spanned the charge, F5): the post-charge drop may already be over,
    /// so the turning point only needs BT to rise `ROR_FOLLOW_TP_RISE_C` above
    /// its minimum (or the timeout).
    pub fn resumed() -> Self {
        Self {
            require_drop: false,
            ..Self::new()
        }
    }

    /// Fresh follower whose drop is measured from `charge_bt` (BT just BEFORE
    /// the charge) instead of the BT at its first step. R-3 (audit
    /// 2026-10-09): a follower created after the drop began (marker held
    /// through Preheating, automatic detection 6 °C into the drop) would
    /// otherwise miss most of the drop and never ramp. A non-finite value
    /// falls back to `new()`.
    pub fn with_charge_bt(charge_bt: f32) -> Self {
        Self {
            charge_bt: if charge_bt.is_finite() {
                Some(charge_bt)
            } else {
                None
            },
            ..Self::new()
        }
    }

    /// True once the turning point was accepted and the setpoint is ramping.
    pub fn ramping(&self) -> bool {
        self.sv.is_some()
    }

    /// R-4: true during the first `ROR_FOLLOW_SV_GRACE_STEPS` ramp steps.
    pub fn in_sv_grace(&self) -> bool {
        self.sv.is_some() && self.ramp_steps < ROR_FOLLOW_SV_GRACE_STEPS
    }

    /// Advance the generator. `elapsed_secs` = seconds since the charge,
    /// `bt` = current bean temperature (°C).
    pub fn step(&mut self, profile: &RorProfile, elapsed_secs: f32, bt: f32) -> RorStep {
        if !bt.is_finite() || !elapsed_secs.is_finite() || elapsed_secs < 0.0 {
            // Bad input: keep the last setpoint (the NaN guard in
            // `update_control` latches the emergency on a NaN BT anyway).
            return match self.sv {
                Some(sv) => RorStep::Setpoint {
                    sv,
                    target_ror: profile.ror_at(self.last_elapsed.unwrap_or(0.0)),
                },
                None => RorStep::WaitingTurningPoint,
            };
        }
        let target_ror = profile.ror_at(elapsed_secs);
        match self.sv {
            None => {
                let charge_bt = *self.charge_bt.get_or_insert(bt);
                if bt < self.min_bt {
                    self.min_bt = bt;
                }
                let dropped =
                    !self.require_drop || self.min_bt <= charge_bt - ROR_FOLLOW_MIN_DROP_C;
                // M-1: the timeout path also needs the drop — no drop, no beans.
                let turned = dropped
                    && (bt >= self.min_bt + ROR_FOLLOW_TP_RISE_C
                        || elapsed_secs >= ROR_FOLLOW_TP_TIMEOUT_SECS);
                if !turned {
                    return RorStep::WaitingTurningPoint;
                }
                self.sv = Some(bt);
                self.last_elapsed = Some(elapsed_secs);
                RorStep::Setpoint { sv: bt, target_ror }
            }
            Some(sv) => {
                self.ramp_steps = self.ramp_steps.saturating_add(1);
                let mut next = sv;
                if let Some(last) = self.last_elapsed {
                    let dt = elapsed_secs - last;
                    if dt > 0.0 && dt <= ROR_FOLLOW_MAX_STEP_SECS {
                        next += target_ror / 60.0 * dt;
                    }
                }
                next = next.clamp(bt - ROR_FOLLOW_MAX_LEAD_C, bt + ROR_FOLLOW_MAX_LEAD_C);
                self.sv = Some(next);
                self.last_elapsed = Some(elapsed_secs);
                RorStep::Setpoint {
                    sv: next,
                    target_ror,
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::config::constants::{RorPoint, ROR_PROFILE_MAX_C_PER_MIN};
    use proptest::prelude::*;

    fn profile(points: &[(u32, f32)]) -> RorProfile {
        let mut p = RorProfile::new();
        for &(t, r) in points {
            p.points
                .push(RorPoint {
                    time_secs: t,
                    ror_c_per_min: r,
                })
                .unwrap();
        }
        p
    }

    #[test]
    fn validate_rejects_bad_profiles() {
        assert!(RorProfile::new().validate().is_err());
        assert!(profile(&[(0, -1.0)]).validate().is_err());
        assert!(profile(&[(0, 31.0)]).validate().is_err());
        assert!(profile(&[(0, f32::NAN)]).validate().is_err());
        assert!(profile(&[(60, 10.0), (60, 9.0)]).validate().is_err());
        assert!(profile(&[(0, 20.0), (300, 10.0), (600, 5.0)])
            .validate()
            .is_ok());
    }

    #[test]
    fn ror_at_interpolates_and_holds_ends() {
        let p = profile(&[(60, 20.0), (360, 10.0)]);
        assert_eq!(p.ror_at(0.0), 20.0);
        assert_eq!(p.ror_at(60.0), 20.0);
        assert!((p.ror_at(210.0) - 15.0).abs() < 1e-4);
        assert_eq!(p.ror_at(1000.0), 10.0);
        assert_eq!(RorProfile::new().ror_at(10.0), 0.0);
    }

    #[test]
    fn waits_for_turning_point_then_ramps() {
        let p = profile(&[(0, 12.0)]);
        let mut f = RorFollower::new();
        // BT falling after charge: no setpoint yet.
        assert_eq!(f.step(&p, 1.0, 150.0), RorStep::WaitingTurningPoint);
        assert_eq!(f.step(&p, 2.0, 120.0), RorStep::WaitingTurningPoint);
        assert_eq!(f.step(&p, 3.0, 100.0), RorStep::WaitingTurningPoint);
        // BT turns (+0.5 above minimum): setpoint starts at BT.
        match f.step(&p, 4.0, 100.5) {
            RorStep::Setpoint { sv, target_ror } => {
                assert_eq!(sv, 100.5);
                assert_eq!(target_ror, 12.0);
            }
            other => panic!("expected setpoint, got {other:?}"),
        }
        // 1 s later at 12 °C/min the setpoint rises 0.2 °C.
        match f.step(&p, 5.0, 100.6) {
            RorStep::Setpoint { sv, .. } => assert!((sv - 100.7).abs() < 1e-4),
            other => panic!("expected setpoint, got {other:?}"),
        }
    }

    #[test]
    fn noise_before_beans_reach_probe_is_not_a_turning_point() {
        let p = profile(&[(0, 12.0)]);
        let mut f = RorFollower::new();
        // CHARGE pressed at BT 200; probe noise ±0.6 before the beans arrive.
        assert_eq!(f.step(&p, 0.5, 200.0), RorStep::WaitingTurningPoint);
        assert_eq!(f.step(&p, 1.0, 199.4), RorStep::WaitingTurningPoint);
        assert_eq!(f.step(&p, 1.5, 200.1), RorStep::WaitingTurningPoint);
        assert!(!f.ramping());
        // Real drop, then the real turning point.
        assert_eq!(f.step(&p, 20.0, 120.0), RorStep::WaitingTurningPoint);
        assert_eq!(f.step(&p, 40.0, 100.0), RorStep::WaitingTurningPoint);
        assert!(matches!(f.step(&p, 41.0, 100.6), RorStep::Setpoint { .. }));
        assert!(f.ramping());
    }

    #[test]
    fn turning_point_timeout_starts_ramp() {
        let p = profile(&[(0, 10.0)]);
        let mut f = RorFollower::new();
        // Charge at 200 °C, BT dropped well below (real beans), never rises.
        assert_eq!(f.step(&p, 1.0, 200.0), RorStep::WaitingTurningPoint);
        assert_eq!(f.step(&p, 10.0, 90.0), RorStep::WaitingTurningPoint);
        assert!(matches!(
            f.step(&p, ROR_FOLLOW_TP_TIMEOUT_SECS, 89.0),
            RorStep::Setpoint { .. }
        ));
    }

    #[test]
    fn sv_grace_covers_only_the_first_ramp_steps() {
        let p = profile(&[(0, 10.0)]);
        let mut f = RorFollower::resumed();
        assert!(!f.in_sv_grace(), "no grace before the ramp");
        assert!(matches!(f.step(&p, 200.0, 100.0), RorStep::Setpoint { .. }));
        for i in 0..ROR_FOLLOW_SV_GRACE_STEPS {
            assert!(f.in_sv_grace(), "grace at ramp step {i}");
            let _ = f.step(&p, 200.3 + i as f32 * 0.3, 100.0);
        }
        assert!(!f.in_sv_grace(), "grace over after the window");
        assert!(f.ramping());
    }

    #[test]
    fn seeded_follower_counts_the_drop_before_it_was_created() {
        // R-3: created at the bottom of the drop (BT 105) after a charge at
        // BT 200. A plain `new()` would take 105 as the charge BT.
        let p = profile(&[(0, 10.0)]);
        let mut f = RorFollower::with_charge_bt(200.0);
        assert_eq!(f.step(&p, 50.0, 105.0), RorStep::WaitingTurningPoint);
        assert_eq!(f.step(&p, 51.0, 104.0), RorStep::WaitingTurningPoint);
        assert!(matches!(f.step(&p, 52.0, 105.0), RorStep::Setpoint { .. }));
        let mut plain = RorFollower::new();
        assert_eq!(plain.step(&p, 50.0, 105.0), RorStep::WaitingTurningPoint);
        assert_eq!(plain.step(&p, 51.0, 104.0), RorStep::WaitingTurningPoint);
        assert_eq!(plain.step(&p, 52.0, 105.0), RorStep::WaitingTurningPoint);
        // Non-finite seed behaves like `new()`.
        assert_eq!(RorFollower::with_charge_bt(f32::NAN), RorFollower::new());
    }

    #[test]
    fn no_drop_never_ramps_even_after_timeout() {
        // M-1: a door dip trips #CHARGE in an empty drum; BT never drops 20 °C.
        let p = profile(&[(0, 10.0)]);
        let mut f = RorFollower::new();
        assert_eq!(f.step(&p, 1.0, 190.0), RorStep::WaitingTurningPoint);
        // A 12 °C door dip (passes a 5 °C rule, not the 20 °C one), then BT recovers.
        assert_eq!(f.step(&p, 5.0, 178.0), RorStep::WaitingTurningPoint);
        assert_eq!(
            f.step(&p, ROR_FOLLOW_TP_TIMEOUT_SECS + 60.0, 200.0),
            RorStep::WaitingTurningPoint
        );
        assert!(!f.ramping());
    }

    #[test]
    fn resumed_follower_needs_only_the_rise() {
        let p = profile(&[(0, 10.0)]);
        let mut f = RorFollower::resumed();
        assert_eq!(f.step(&p, 30.0, 96.0), RorStep::WaitingTurningPoint);
        assert_eq!(f.step(&p, 31.0, 95.0), RorStep::WaitingTurningPoint);
        assert!(matches!(f.step(&p, 32.0, 95.6), RorStep::Setpoint { .. }));
    }

    #[test]
    fn long_gap_is_not_integrated() {
        let p = profile(&[(0, 30.0)]);
        let mut f = RorFollower::resumed();
        let _ = f.step(&p, ROR_FOLLOW_TP_TIMEOUT_SECS, 150.0);
        // 60 s gap: no 30 °C jump; setpoint stays where it was.
        match f.step(&p, ROR_FOLLOW_TP_TIMEOUT_SECS + 60.0, 150.0) {
            RorStep::Setpoint { sv, .. } => assert_eq!(sv, 150.0),
            other => panic!("expected setpoint, got {other:?}"),
        }
    }

    proptest! {
        #[test]
        fn setpoint_always_within_lead_of_bt(
            rors in prop::collection::vec(0.0f32..=30.0, 1..8),
            bts in prop::collection::vec(20.0f32..260.0, 1..200),
            dts in prop::collection::vec(0.0f32..5.0, 1..200),
        ) {
            let mut p = RorProfile::new();
            for (i, r) in rors.iter().enumerate() {
                p.points.push(RorPoint { time_secs: (i as u32) * 60, ror_c_per_min: *r }).unwrap();
            }
            let mut f = RorFollower::new();
            let mut t = 0.0f32;
            for (i, bt) in bts.iter().enumerate() {
                t += dts[i % dts.len()];
                if let RorStep::Setpoint { sv, target_ror } = f.step(&p, t, *bt) {
                    prop_assert!(sv.is_finite());
                    prop_assert!((sv - bt).abs() <= ROR_FOLLOW_MAX_LEAD_C + 1e-3);
                    prop_assert!((0.0..=ROR_PROFILE_MAX_C_PER_MIN).contains(&target_ror));
                }
            }
        }
    }
}
