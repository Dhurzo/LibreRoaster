//! CORE-4 (plan CORE-2026-10-09): the state of ONE batch of beans.
//!
//! Everything that is born at a charge and dies at a drop lives here:
//! automatic charge detection, the CHARGE/DROP markers, the batch weight and
//! the RoR-follow generator. Before CORE-4 these 13 fields sat among the 50
//! fields of `RoasterControl`, and each lifecycle event (STOP, recovery, new
//! roast, DROP, PREHEAT) cleared its own hand-picked subset of them — the
//! source of N2, M-3, M-7, R-1, R-2 and R-5.
//!
//! `RoasterControl` owns one `BatchState` (`self.batch`). The status mirror
//! `SystemStatus::charge_detected` stays in `RoasterControl`.

use crate::config::constants::{RorProfile, CHARGE_DROP_THRESHOLD_C, CHARGE_SAMPLE_TICK_DIV};
use crate::control::ror_follow::{RorFollower, RorStep};
use embassy_time::Instant;
use log::info;

/// State of the current batch (see module docs).
pub struct BatchState {
    /// A charge was detected (automatic `#CHARGE`) or marked (CHARGE marker).
    pub(crate) charge_detected: bool,
    /// Tick time of the charge: time-budget anchor and RoR-follow clock.
    pub(crate) charge_time: Option<Instant>,
    /// Rolling bean-temperature samples feeding charge (bean-drop) detection.
    pub(crate) bt_charge_history: heapless::Deque<f32, 10>,
    /// Per-tick divider that throttles `bt_charge_history` sampling to once
    /// every `CHARGE_SAMPLE_TICK_DIV` ticks. With the real tick cadence
    /// (`CONTROL_LOOP_TICK_MS` ≈ 330 ms, see constants.rs) the divisor
    /// resolves to 1 — the deque of 10 samples covers the intended ≈ 3 s
    /// charge window.
    pub(crate) charge_history_tick_div: u8,
    /// DIFF E1: a `CHARGE` marker arrived; applied on a control tick (tick
    /// time base) by `apply_pending_charge`.
    pub(crate) pending_charge: bool,
    /// DIFF E1: tick time of the first attempt to apply the pending marker
    /// (start of the `CHARGE_MARKER_GRACE_SECS` window).
    pub(crate) pending_charge_since: Option<Instant>,
    /// R-3 (audit 2026-10-09): highest BT seen when CHARGE commands of the
    /// pending marker arrived — the reference for the RoR-follow drop rule.
    /// Reset by `handle_charge` whenever no marker is pending; consumed when
    /// the marker is applied in a roast.
    pub(crate) pending_charge_bt: Option<f32>,
    /// DIFF E1: an explicit CHARGE already anchored this batch. Cleared by
    /// DROP, by a new roast handoff and by `stop_streaming`.
    pub(crate) explicit_charge_seen: bool,
    /// R-1 (audit 2026-10-09): a CHARGE marker was applied while the PID was
    /// not in control (OT1 takeover), so RoR-follow could not arm. The next
    /// bumpless `PID;ON` arms it (`maybe_resume_ror_follow`) and clears this.
    /// Cleared by `stop_ror_follow` (DROP, STOP, latch, recovery, new roast,
    /// `RORPROFILE;OFF`, `PID;CHAN;1`) and by any `PID;SV`.
    pub(crate) ror_resume_pending: bool,
    /// R-2 (audit 2026-10-09): a DROP ended the batch. Until a CHARGE marker
    /// anchors the next one, the automatic `#CHARGE` detector does not arm
    /// RoR-follow (the BT fall of the drop itself looks like a charge). Set
    /// by `handle_drop`; cleared by a CHARGE marker, a new roast handoff,
    /// `stop_streaming` and `clear_emergency_explicit`.
    pub(crate) batch_dropped: bool,
    /// DIFF E1: batch weight from `CHARGE;<grams>` (Artisan `{WEIGHTin}`).
    pub(crate) batch_grams: Option<u16>,
    /// DIFF E3: active RoR-follow generator (`Some` between CHARGE and DROP).
    pub(crate) ror_follower: Option<RorFollower>,
    /// DIFF E3: RoR the generator is following right now (°C/min; 0 = none).
    pub(crate) ror_target_c_per_min: f32,
}

impl Default for BatchState {
    fn default() -> Self {
        Self {
            charge_detected: false,
            charge_time: None,
            bt_charge_history: heapless::Deque::new(),
            charge_history_tick_div: 0,
            pending_charge: false,
            pending_charge_since: None,
            pending_charge_bt: None,
            explicit_charge_seen: false,
            ror_resume_pending: false,
            batch_dropped: false,
            batch_grams: None,
            ror_follower: None,
            ror_target_c_per_min: 0.0,
        }
    }
}

/// DIFF E1: a `CHARGE` marker that cannot be applied yet (no roast running)
/// is kept this long, so Artisan's pidOnCHARGE (CHARGE marker and `PID;ON`
/// sent within milliseconds of each other, in either order) still anchors
/// the roast to the charge even when the two land in different control ticks.
pub(crate) const CHARGE_MARKER_GRACE_SECS: u64 = 5;

/// What `BatchState::apply_pending_charge` did on this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MarkerOutcome {
    /// No CHARGE marker pending.
    NonePending,
    /// The marker anchored this batch (Heating/Stable). The caller mirrors
    /// `charge_detected = true` into `SystemStatus`.
    Anchored,
    /// The batch was already marked: the marker was consumed and ignored.
    Duplicate,
    /// Preheating: the marker is kept for the START / `PID;ON` that follows.
    KeptForRoast,
    /// No roast (Idle/Error): kept inside the grace period, dropped after it.
    /// The caller may restart the manual heat-session budget.
    OutsideRoast,
}

/// Lifecycle of one batch. Every event that ends or starts a batch has ONE
/// method here; the table below is the contract (unit-tested at the end of
/// this file). "detection" = `charge_detected`, `charge_time`,
/// `bt_charge_history`, `charge_history_tick_div`; "follower" = `stop_follower()`.
///
/// | Event (caller) | detection | pending marker | explicit | dropped | follower |
/// |---|---|---|---|---|---|
/// | `on_stop` (`stop_streaming`) | reset | cleared | cleared | cleared | stopped |
/// | `on_recovery` (`clear_emergency_explicit`) | reset | KEPT | KEPT | cleared | stopped |
/// | `on_new_roast` (START / `PID;ON` handoff) | reset | KEPT | cleared | cleared | stopped |
/// | `on_drop(roast)` (DROP) | reset except `charge_time`, only in a roast | cleared | cleared | SET | stopped |
/// | `on_preheat(already)` (PREHEAT) | — | cleared unless already preheating | — | — | — |
/// | `on_setpoint_override` (`PID;SV`) | — | — | — | — | stopped if ramping past the grace window; re-arm cancelled |
impl BatchState {
    /// Forget the charge-detection state of the previous batch.
    fn reset_detection(&mut self) {
        self.charge_detected = false;
        self.charge_time = None;
        self.bt_charge_history.clear();
        self.charge_history_tick_div = 0;
    }

    /// STOP (`stop_streaming`): the batch is over, nothing survives.
    pub(crate) fn on_stop(&mut self) {
        self.reset_detection();
        self.pending_charge = false;
        self.pending_charge_since = None;
        self.explicit_charge_seen = false;
        self.batch_dropped = false;
        self.stop_follower();
    }

    /// Latch recovery (`clear_emergency_explicit`, N2): the roast anchors go,
    /// a CHARGE marker sent while latched is KEPT for the recovered roast.
    pub(crate) fn on_recovery(&mut self) {
        self.reset_detection();
        self.batch_dropped = false;
        self.stop_follower();
    }

    /// New roast (START / `PID;ON` handoff). The pending marker is KEPT:
    /// Artisan's pidOnCHARGE may send CHARGE right before `PID;ON`.
    pub(crate) fn on_new_roast(&mut self) {
        self.explicit_charge_seen = false;
        self.batch_dropped = false;
        self.stop_follower();
        self.reset_detection();
    }

    /// DROP marker. In a roast, automatic detection re-arms for the next
    /// batch; `charge_time` stays as the budget anchor until the next charge.
    pub(crate) fn on_drop(&mut self, roast_active: bool) {
        self.stop_follower();
        self.pending_charge = false;
        self.pending_charge_since = None;
        self.explicit_charge_seen = false;
        // R-2: no RoR-follow for this drum until a CHARGE marker says there
        // are beans in it again.
        self.batch_dropped = true;
        if roast_active {
            self.charge_detected = false;
            self.bt_charge_history.clear();
            self.charge_history_tick_div = 0;
        }
    }

    /// PREHEAT (M-3, R-5): a marker that predates this PREHEAT belongs to an
    /// earlier session; a PREHEAT re-sent during Preheating keeps it.
    pub(crate) fn on_preheat(&mut self, already_preheating: bool) {
        if !already_preheating {
            self.pending_charge = false;
            self.pending_charge_since = None;
        }
    }

    /// `CHARGE` / `CHARGE;<grams>` command (command clock: records only).
    /// R-3: remembers the highest BT seen while this marker is pending.
    pub(crate) fn on_charge_command(&mut self, grams: Option<u16>, bean_temp: f32) {
        if grams.is_some() {
            self.batch_grams = grams;
        }
        if !self.pending_charge {
            self.pending_charge_bt = None;
        }
        if bean_temp.is_finite() {
            self.pending_charge_bt = Some(
                self.pending_charge_bt
                    .map_or(bean_temp, |b| b.max(bean_temp)),
            );
        }
        self.pending_charge = true;
    }

    /// `PID;SV`: an explicit setpoint ends a RoR ramp (P-17: not before the
    /// turning point; R-4: not inside the grace window) and cancels a
    /// pending re-arm (R-1).
    pub(crate) fn on_setpoint_override(&mut self) {
        if self
            .ror_follower
            .is_some_and(|f| f.ramping() && !f.in_sv_grace())
        {
            self.stop_follower();
        }
        self.ror_resume_pending = false;
    }

    /// Arm `follower` if none exists and the caller says arming is allowed
    /// (`RoasterControl::ror_can_arm`).
    pub(crate) fn arm_follower(&mut self, follower: RorFollower, can_arm: bool) {
        if self.ror_follower.is_none() && can_arm {
            self.ror_follower = Some(follower);
            info!("RoR-follow armed - waiting for the turning point");
        }
    }

    /// M-7 / R-1: `PID;ON` after an OT1 takeover re-arms RoR-follow only for
    /// a CHARGE marker applied during the takeover (`ror_resume_pending`).
    pub(crate) fn resume_follower(&mut self, can_arm: bool) {
        if self.ror_resume_pending {
            self.ror_resume_pending = false;
            self.arm_follower(RorFollower::resumed(), can_arm);
        }
    }

    /// End RoR-follow (the PID keeps its current setpoint).
    pub(crate) fn stop_follower(&mut self) {
        if self.ror_follower.is_some() {
            info!("RoR-follow stopped");
        }
        self.ror_follower = None;
        self.ror_target_c_per_min = 0.0;
        // R-1: whatever ended RoR-follow also cancels a pending re-arm.
        self.ror_resume_pending = false;
    }

    /// Apply a pending CHARGE marker on the control tick (tick time base).
    /// `roast_active` = Heating/Stable, `preheating` = Preheating.
    pub(crate) fn apply_pending_charge(
        &mut self,
        now: Instant,
        roast_active: bool,
        preheating: bool,
        bean_temp: f32,
        can_arm: bool,
    ) -> MarkerOutcome {
        if !self.pending_charge {
            return MarkerOutcome::NonePending;
        }
        let since = *self.pending_charge_since.get_or_insert(now);
        let in_grace = now.saturating_duration_since(since).as_secs() < CHARGE_MARKER_GRACE_SECS;
        if roast_active {
            self.pending_charge = false;
            self.pending_charge_since = None;
            // R-3: drop reference = the higher of the BT when CHARGE arrived
            // and the BT now (f32::max ignores a NaN operand).
            let charge_ref_bt = self
                .pending_charge_bt
                .take()
                .map_or(bean_temp, |b| b.max(bean_temp));
            if self.explicit_charge_seen {
                info!("CHARGE ignored - already marked for this batch (send DROP first)");
                return MarkerOutcome::Duplicate;
            }
            self.explicit_charge_seen = true;
            self.batch_dropped = false;
            self.charge_detected = true;
            self.charge_time = Some(now);
            self.bt_charge_history.clear();
            self.charge_history_tick_div = 0;
            info!("CHARGE marker applied - roast budget anchored to the charge");
            // DIFF E3: arm RoR-follow from the real charge. A follower that
            // is already ramping (late button press) is kept as it is.
            if !self.ror_follower.is_some_and(|f| f.ramping()) {
                self.ror_follower = None;
                self.arm_follower(RorFollower::with_charge_bt(charge_ref_bt), can_arm);
                // R-1: in manual mode nothing armed; remember the marker so
                // the next PID;ON re-arms RoR-follow for THIS batch.
                self.ror_resume_pending = self.ror_follower.is_none();
            }
            MarkerOutcome::Anchored
        } else if preheating {
            MarkerOutcome::KeptForRoast
        } else {
            if !in_grace {
                self.pending_charge = false;
                self.pending_charge_since = None;
                info!("CHARGE marker dropped - no roast started within the grace period");
            }
            MarkerOutcome::OutsideRoast
        }
    }

    /// Automatic `#CHARGE` detector, one call per control tick. Samples BT
    /// every `CHARGE_SAMPLE_TICK_DIV` ticks in a roast until a charge is
    /// detected; a drop above `CHARGE_DROP_THRESHOLD_C` across the 3 s window
    /// anchors the charge and (unless the batch was dropped, R-2) arms
    /// RoR-follow seeded with the BT before the drop (R-3). Returns the drop
    /// (°C) on the tick the charge is detected.
    pub(crate) fn sample_auto_charge(
        &mut self,
        now: Instant,
        bt: f32,
        roast_active: bool,
        can_arm: bool,
    ) -> Option<f32> {
        if !roast_active || self.charge_detected {
            return None;
        }
        self.charge_history_tick_div = self.charge_history_tick_div.saturating_add(1);
        if self.charge_history_tick_div < CHARGE_SAMPLE_TICK_DIV {
            return None;
        }
        self.charge_history_tick_div = 0;
        if bt > 50.0 {
            if self.bt_charge_history.len() >= 10 {
                let _ = self.bt_charge_history.pop_front();
            }
            let _ = self.bt_charge_history.push_back(bt);
            if self.bt_charge_history.len() >= 5 {
                let (front, _back) = self.bt_charge_history.as_slices();
                let first = front.first().copied().unwrap_or(bt);
                let drop = first - bt;
                if drop > CHARGE_DROP_THRESHOLD_C {
                    self.charge_detected = true;
                    self.charge_time = Some(now);
                    if !self.batch_dropped {
                        self.arm_follower(RorFollower::with_charge_bt(first), can_arm);
                    }
                    return Some(drop);
                }
            }
        }
        None
    }

    /// Advance RoR-follow by one control tick. `None` when there is no
    /// follower or no charge anchor; otherwise the generator's answer, with
    /// `ror_target_c_per_min` updated.
    pub(crate) fn follow_step(
        &mut self,
        profile: &RorProfile,
        now: Instant,
        bean_temp: f32,
    ) -> Option<RorStep> {
        let (Some(follower), Some(charge)) = (self.ror_follower.as_mut(), self.charge_time) else {
            return None;
        };
        let elapsed = now.saturating_duration_since(charge).as_micros() as f32 * 1e-6;
        let step = follower.step(profile, elapsed, bean_temp);
        self.ror_target_c_per_min = match step {
            RorStep::WaitingTurningPoint => 0.0,
            RorStep::Setpoint { target_ror, .. } => target_ror,
        };
        Some(step)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::constants::RorPoint;

    fn profile() -> RorProfile {
        let mut p = RorProfile::new();
        let _ = p.points.push(RorPoint {
            time_secs: 0,
            ror_c_per_min: 10.0,
        });
        p
    }

    fn t(secs: u64) -> Instant {
        Instant::from_secs(1000 + secs)
    }

    /// A batch in full swing: charge anchored, marker pending, follower ramping.
    fn busy() -> BatchState {
        let mut b = BatchState {
            charge_detected: true,
            charge_time: Some(t(0)),
            charge_history_tick_div: 1,
            pending_charge: true,
            pending_charge_since: Some(t(1)),
            pending_charge_bt: Some(180.0),
            explicit_charge_seen: true,
            batch_grams: Some(250),
            ror_follower: Some(RorFollower::resumed()),
            ror_target_c_per_min: 9.0,
            ..BatchState::default()
        };
        let _ = b.bt_charge_history.push_back(150.0);
        b
    }

    fn detection_reset(b: &BatchState) -> bool {
        !b.charge_detected
            && b.charge_time.is_none()
            && b.bt_charge_history.is_empty()
            && b.charge_history_tick_div == 0
    }

    #[test]
    fn on_stop_clears_the_batch() {
        let mut b = busy();
        b.on_stop();
        assert!(detection_reset(&b));
        assert!(!b.pending_charge && b.pending_charge_since.is_none());
        assert!(!b.explicit_charge_seen && !b.batch_dropped);
        assert!(b.ror_follower.is_none() && b.ror_target_c_per_min == 0.0);
        assert_eq!(
            b.batch_grams,
            Some(250),
            "the weight is reported, never cleared"
        );
    }

    #[test]
    fn on_recovery_keeps_marker_and_explicit_flag() {
        let mut b = busy();
        b.batch_dropped = true;
        b.on_recovery();
        assert!(detection_reset(&b));
        assert!(
            b.pending_charge,
            "a CHARGE sent while latched anchors the recovered roast (N2)"
        );
        assert!(b.explicit_charge_seen);
        assert!(!b.batch_dropped);
        assert!(b.ror_follower.is_none());
    }

    #[test]
    fn on_new_roast_keeps_the_pending_marker() {
        let mut b = busy();
        b.on_new_roast();
        assert!(detection_reset(&b));
        assert!(
            b.pending_charge,
            "pidOnCHARGE: CHARGE may arrive just before PID;ON"
        );
        assert!(!b.explicit_charge_seen && !b.batch_dropped);
        assert!(b.ror_follower.is_none());
    }

    #[test]
    fn on_drop_in_a_roast_rearms_detection_but_keeps_the_budget_anchor() {
        let mut b = busy();
        b.on_drop(true);
        assert!(!b.charge_detected && b.bt_charge_history.is_empty());
        assert!(b.charge_time.is_some(), "budget anchor survives DROP");
        assert!(b.batch_dropped && !b.explicit_charge_seen && !b.pending_charge);
        assert!(b.ror_follower.is_none());
        let mut idle = busy();
        idle.on_drop(false);
        assert!(
            idle.charge_detected,
            "outside a roast detection is left alone"
        );
    }

    #[test]
    fn on_preheat_keeps_the_marker_only_when_already_preheating() {
        let mut b = busy();
        b.on_preheat(true);
        assert!(b.pending_charge);
        b.on_preheat(false);
        assert!(!b.pending_charge && b.pending_charge_since.is_none());
    }

    #[test]
    fn on_charge_command_keeps_the_highest_bt_of_the_pending_marker() {
        let mut b = BatchState {
            pending_charge_bt: Some(300.0), // stale, no marker pending
            ..BatchState::default()
        };
        b.on_charge_command(Some(200), 190.0);
        assert_eq!(b.pending_charge_bt, Some(190.0), "stale value discarded");
        b.on_charge_command(None, 120.0);
        assert_eq!(b.pending_charge_bt, Some(190.0), "max while pending");
        assert_eq!(b.batch_grams, Some(200));
        b.on_charge_command(None, f32::NAN);
        assert_eq!(b.pending_charge_bt, Some(190.0), "NaN ignored");
        assert!(b.pending_charge);
    }

    #[test]
    fn setpoint_override_respects_turning_point_and_grace() {
        let p = profile();
        let mut waiting = BatchState {
            ror_follower: Some(RorFollower::new()),
            ..BatchState::default()
        };
        waiting.on_setpoint_override();
        assert!(
            waiting.ror_follower.is_some(),
            "P-17: before the turning point it is kept"
        );

        let mut f = RorFollower::resumed();
        let _ = f.step(&p, 200.0, 100.0); // ramp starts
        let mut in_grace = BatchState {
            ror_follower: Some(f),
            ..BatchState::default()
        };
        in_grace.on_setpoint_override();
        assert!(in_grace.ror_follower.is_some(), "R-4: grace window");

        for i in 0..20 {
            let _ = f.step(&p, 200.3 + i as f32 * 0.3, 100.0);
        }
        let mut late = BatchState {
            ror_follower: Some(f),
            ror_resume_pending: true,
            ..BatchState::default()
        };
        late.on_setpoint_override();
        assert!(late.ror_follower.is_none() && !late.ror_resume_pending);
    }

    #[test]
    fn arm_and_resume_follow_the_rules() {
        let mut b = BatchState::default();
        b.arm_follower(RorFollower::new(), false);
        assert!(b.ror_follower.is_none(), "arming not allowed");
        b.arm_follower(RorFollower::new(), true);
        assert!(b.ror_follower.is_some());
        b.stop_follower();
        b.resume_follower(true);
        assert!(
            b.ror_follower.is_none(),
            "no pending re-arm, nothing to resume"
        );
        b.ror_resume_pending = true;
        b.resume_follower(true);
        assert!(b.ror_follower.is_some() && !b.ror_resume_pending);
    }

    #[test]
    fn marker_in_a_roast_anchors_once_and_remembers_a_manual_takeover() {
        let mut b = BatchState::default();
        b.on_charge_command(None, 200.0);
        assert_eq!(
            b.apply_pending_charge(t(10), true, false, 95.0, false),
            MarkerOutcome::Anchored
        );
        assert!(b.charge_detected && b.charge_time == Some(t(10)) && b.explicit_charge_seen);
        assert!(
            b.ror_follower.is_none() && b.ror_resume_pending,
            "R-1: PID not in control"
        );
        b.on_charge_command(None, 95.0);
        assert_eq!(
            b.apply_pending_charge(t(11), true, false, 95.0, true),
            MarkerOutcome::Duplicate
        );
        assert_eq!(b.charge_time, Some(t(10)));
        assert_eq!(
            b.apply_pending_charge(t(12), true, false, 95.0, true),
            MarkerOutcome::NonePending
        );
    }

    #[test]
    fn marker_outside_a_roast_waits_for_the_grace_period() {
        let mut b = BatchState::default();
        b.on_charge_command(None, 200.0);
        assert_eq!(
            b.apply_pending_charge(t(0), false, true, 200.0, false),
            MarkerOutcome::KeptForRoast
        );
        let mut idle = BatchState::default();
        idle.on_charge_command(None, 200.0);
        assert_eq!(
            idle.apply_pending_charge(t(0), false, false, 200.0, false),
            MarkerOutcome::OutsideRoast
        );
        assert!(idle.pending_charge, "inside the grace period");
        assert_eq!(
            idle.apply_pending_charge(t(CHARGE_MARKER_GRACE_SECS), false, false, 200.0, false),
            MarkerOutcome::OutsideRoast
        );
        assert!(!idle.pending_charge, "dropped after the grace period");
    }

    #[test]
    fn auto_charge_fires_on_a_fast_drop_and_respects_a_drop_marker() {
        let run = |dropped: bool| {
            let mut b = BatchState {
                batch_dropped: dropped,
                ..BatchState::default()
            };
            let mut fired = None;
            for (i, bt) in [200.0, 199.0, 198.0, 190.0, 180.0, 170.0, 160.0]
                .iter()
                .enumerate()
            {
                for _ in 0..CHARGE_SAMPLE_TICK_DIV {
                    if let Some(d) = b.sample_auto_charge(t(i as u64), *bt, true, true) {
                        fired = Some(d);
                    }
                }
            }
            (fired, b)
        };
        let (fired, b) = run(false);
        assert!(fired.is_some_and(|d| d > CHARGE_DROP_THRESHOLD_C));
        assert!(b.charge_detected && b.charge_time.is_some() && b.ror_follower.is_some());
        let (fired, b) = run(true);
        assert!(
            fired.is_some() && b.ror_follower.is_none(),
            "R-2: no arming after DROP"
        );
        let mut idle = BatchState::default();
        assert_eq!(idle.sample_auto_charge(t(0), 100.0, false, true), None);
        assert_eq!(
            idle.charge_history_tick_div, 0,
            "not sampling outside a roast"
        );
    }

    #[test]
    fn follow_step_needs_a_follower_and_an_anchor() {
        let p = profile();
        let mut b = BatchState::default();
        assert_eq!(b.follow_step(&p, t(0), 100.0), None);
        b.ror_follower = Some(RorFollower::resumed());
        assert_eq!(b.follow_step(&p, t(0), 100.0), None, "no charge anchor");
        b.charge_time = Some(t(0));
        assert!(matches!(
            b.follow_step(&p, t(200), 100.0),
            Some(RorStep::Setpoint { .. })
        ));
        assert_eq!(b.ror_target_c_per_min, 10.0);
    }
}
