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

use crate::control::ror_follow::RorFollower;
use embassy_time::Instant;

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
