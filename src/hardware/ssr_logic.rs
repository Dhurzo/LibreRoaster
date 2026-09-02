//! Pure decision logic for SSR heat-source detection and availability.
//!
//! Kept in its own un-gated module (compiled on BOTH host and embedded) so
//! the decision logic is covered by host unit tests.

#[cfg(not(feature = "no-heat-sense"))]
use crate::config::constants::SSR_PWM_RESOLUTION;
#[cfg(not(feature = "no-heat-sense"))]
use crate::hardware::heat_presence::{debounce_heat_absent, HeatPresenceOutcome};
use log::info;
#[cfg(not(feature = "no-heat-sense"))]
use log::{error, warn};

/// Error returned by SSR control operations.
///
/// Re-exported from `hardware::ssr` so existing callers (including the host
/// `ssr_stub`, which keeps its own copy) are unaffected.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SsrError {
    /// GPIO write (SSR pin) failed.
    OutputError { source: &'static str },
    /// GPIO read (detection pin) failed.
    InputError { source: &'static str },
    /// Heat source not detected despite commanded duty.
    HeatSourceNotDetected { source: &'static str },
    /// LEDC PWM write or duty verification failed.
    PwmError { source: &'static str },
}

impl embedded_hal::digital::Error for SsrError {
    fn kind(&self) -> embedded_hal::digital::ErrorKind {
        embedded_hal::digital::ErrorKind::Other
    }
}

/// Heat-source hardware availability state for an SSR channel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SsrHardwareStatus {
    /// Heat source present and responding to commanded duty.
    Available,
    /// Heater commanded but no heat detected (debounced).
    NotDetected,
    /// Detection pin error or stuck-on cross-check tripped.
    Error,
}

/// Number of consecutive "heater ON but no heat detected" samples (at
/// duty ≥ 50 %) before `hardware_status` latches to `Error`.
#[allow(dead_code)]
const HEAT_MISMATCH_MAX: u8 = 5;
/// `heat_mismatch_count`/`heat_present_count` thresholds are sampled every
/// control-loop tick (~330 ms on embedded: 210 ms MAX31856 wait + 100 ms
/// timer + overhead). `HEAT_MISMATCH_MAX = 5` therefore represents ≈ 1.7 s
/// ≈ 8.5 full PWM cycles at 5 Hz, which filters out the ~70 % of the cycle
/// that legitimately reads "no heat" at duty = 30 %. `HEAT_PRESENT_MISMATCH_MAX
/// = 10` (≈ 3.3 s) tolerates the residual heat of the metal mass and only fires
/// when the SSR is genuinely stuck on.
#[allow(dead_code)]
const HEAT_PRESENT_MISMATCH_MAX: u8 = 10;

/// Common status queries implemented by SSR control types.
pub trait StatusGetters {
    /// Return the current `SsrHardwareStatus`.
    fn get_hardware_status(&self) -> SsrHardwareStatus;
    /// True when the heat source is `Available`.
    fn is_heating_available(&self) -> bool;
    /// Return the last commanded raw duty in LEDC ticks.
    fn get_current_duty(&self) -> u16;
    /// True when the PWM output is enabled.
    fn is_pwm_enabled(&self) -> bool;
    /// Last measured duty delta (commanded vs readback) in ticks.
    fn last_lead_delta_ticks(&self) -> i16;
    /// Number of duty-retry attempts on the last set.
    fn last_retry_count(&self) -> u8;
}

/// Common state for SSR control implementations.
/// Embedded by both SsrControl and SsrControlSimple to eliminate code duplication.
pub struct SsrControlBase {
    pub(crate) hardware_status: SsrHardwareStatus,
    pub(crate) current_duty: u16,
    pub(crate) last_duty_delta_ticks: i16,
    pub(crate) retry_count: u8,
    pub(crate) is_pwm_enabled: bool,
    /// Consecutive `detect_heat_source` samples that read "no heat" while the
    /// duty is ≥ 50 %. Managed by `heat_presence::debounce_heat_absent`;
    /// only when it reaches `HEAT_ABSENT_DEBOUNCE` does the status flip to
    /// `NotDetected`.
    ///
    /// Bug audit 2026-08-02: a single OFF sample is ambiguous (the PWM OFF
    /// window at duty ≥ 50 % reads HIGH even when the SSR is conducting) —
    /// the previous one-sample flip latched `NotDetected` mid-roast, which
    /// forces the heater to 0 % and (because duty 0 falls below the
    /// observability gate) dead-locks the heater until power cycle.
    heat_absent_count: u8,
    #[allow(dead_code)]
    heat_mismatch_count: u8,
    /// Debounce counter for the "heat present while heater off" branch.
    /// The SSR is PWM at 5 Hz; with a metal heat mass, residual heat can keep
    /// the sensor reading hot long after the duty drops to zero. We require
    /// `HEAT_PRESENT_MISMATCH_MAX` consecutive mismatched samples before
    /// declaring the SSR stuck on, so a single transient does not trip the
    /// safety interlock mid-roast (the bug closed by this change).
    #[allow(dead_code)]
    heat_present_count: u8,
}

impl SsrControlBase {
    pub fn new() -> Self {
        // Boot-time status is `Available`: at duty 0 the SSR does not conduct
        // and the documentation-defined wiring (pin pulled HIGH when SSR off,
        // LOW when SSR conducts) makes a single sample at boot uninformative.
        // Treating the heater as available at boot is necessary so manual and
        // PID commands are not silently masked out by a false NotDetected latch
        // (caused by the dead-lock the report flags: 0% duty → pin HIGH →
        // NotDetected → output forced to 0 → 0% duty forever).
        SsrControlBase {
            hardware_status: SsrHardwareStatus::Available,
            current_duty: 0,
            last_duty_delta_ticks: 0,
            retry_count: 0,
            is_pwm_enabled: true,
            heat_absent_count: 0,
            heat_mismatch_count: 0,
            heat_present_count: 0,
        }
    }

    /// Re-arms the SSR availability state machine.
    pub fn rearm(&mut self) {
        if self.hardware_status != SsrHardwareStatus::Available {
            info!(
                "SSR hardware status re-armed by operator recovery (was {:?})",
                self.hardware_status
            );
        }
        self.hardware_status = SsrHardwareStatus::Available;
        self.heat_absent_count = 0;
        self.heat_mismatch_count = 0;
        self.heat_present_count = 0;
    }

    /// Detect heat source using a closure to read the detection pin.
    /// This eliminates duplicate code in SsrControl and SsrControlSimple.
    ///
    /// Observability gate: when `current_duty` is too low (belly of the 5 Hz
    /// PWM cycle shorter than the sample interval, including duty 0 at boot),
    /// the detection pin is uninformative — a HIGH sample does NOT mean "SSR
    /// stuck off", it means "we sampled during the OFF window". Skipping the
    /// state update for low duty windows prevents the boot-time dead-lock
    /// where the SSR could never become `Available` (0% duty → HIGH → NOTDET
    /// → output forced to 0 → never 50%+ duty → never Available).
    ///
    /// Debounce (bug audit 2026-08-02): the OFF → `NotDetected` transition is
    /// no longer a single-sample flip. At duty ≥ 50 % a HIGH sample is still
    /// ambiguous (it may be the PWM OFF window), so it only accumulates via
    /// `heat_presence::debounce_heat_absent`; the status flips after
    /// `HEAT_ABSENT_DEBOUNCE` consecutive samples (see the module docs for
    /// the run-bound argument that makes this aliasing-proof at the real tick
    /// cadence). A LOW sample, by contrast, is trustworthy evidence of
    /// current flow and restores `Available` immediately.
    pub fn detect_heat_source<F, E>(
        &mut self,
        _current_time: u32,
        read_pin: F,
    ) -> Result<(), SsrError>
    where
        F: FnMut() -> Result<bool, E>,
    {
        #[cfg(feature = "no-heat-sense")]
        {
            let _ = (_current_time, read_pin);
            return Ok(());
        }

        #[cfg(not(feature = "no-heat-sense"))]
        {
            let mut read_pin = read_pin;
            // ≥50% duty ≈ one full sample interval of conduction per PWM period at
            // 5 Hz vs. ~330 ms sampling; below that the pin read may legitimately
            // land in the OFF window even when the SSR is wired and functional.
            let min_observable_ticks = (1u32 << SSR_PWM_RESOLUTION) / 2;
            if (self.current_duty as u32) < min_observable_ticks {
                // Duty too low for the pin to be informative — and a low-power
                // stretch must not accumulate toward NotDetected.
                self.heat_absent_count = 0;
                // Bug H5 (2026-08-10): the latch was terminal. `Available` could
                // ONLY be re-written here on a `HeatDetected` outcome, which
                // requires passing the duty-observability gate — but while the
                // status is not `Available`, the control loop forces the output
                // to 0 % every tick (roaster_control.rs), which is below the
                // gate, so the pin was never read again and the heater stayed
                // dead until power cycle. A single LOW sample is trustworthy
                // evidence of current flow at ANY duty (the PWM OFF window can
                // only produce HIGH), so honour it here as a re-detection.
                if self.hardware_status != SsrHardwareStatus::Available
                    && matches!(read_pin(), Ok(true))
                {
                    info!("Heat source re-detected at low duty - clearing latch");
                    self.hardware_status = SsrHardwareStatus::Available;
                }
                return Ok(());
            }

            match read_pin() {
                Ok(is_detected) => {
                    let (new_count, outcome) =
                        debounce_heat_absent(self.heat_absent_count, is_detected, true);
                    self.heat_absent_count = new_count;
                    match outcome {
                        HeatPresenceOutcome::HeatDetected => {
                            if self.hardware_status != SsrHardwareStatus::Available {
                                info!("Heat source detected - SSR heating operational");
                                self.hardware_status = SsrHardwareStatus::Available;
                            }
                        }
                        HeatPresenceOutcome::HeatAbsent => {
                            if self.hardware_status == SsrHardwareStatus::Available {
                                warn!(
                                    "Heat source not detected - SSR commands work but no heat generated"
                                );
                                self.hardware_status = SsrHardwareStatus::NotDetected;
                            }
                        }
                        HeatPresenceOutcome::NoChange => {}
                    }
                    Ok(())
                }
                Err(_) => {
                    if self.hardware_status != SsrHardwareStatus::Error {
                        error!("SSR detection pin error - switching to error state");
                        self.hardware_status = SsrHardwareStatus::Error;
                    }
                    Err(SsrError::InputError {
                        source: "detection_pin_read_failed",
                    })
                }
            }
        }
    }

    /// Cross-check commanded duty against the detection pin to catch a stuck-on
    /// or never-heating SSR (no-op under `simulated-sensors`/`no-heat-sense`).
    pub fn cross_check_heat_detection<F, E>(
        &mut self,
        current_duty: u16,
        read_pin: F,
    ) -> Result<(), SsrError>
    where
        F: FnMut() -> Result<bool, E>,
    {
        #[cfg(any(feature = "simulated-sensors", feature = "no-heat-sense"))]
        {
            let _ = (current_duty, read_pin);
            return Ok(());
        }

        #[cfg(not(any(feature = "simulated-sensors", feature = "no-heat-sense")))]
        {
            let mut read_pin = read_pin;
            match read_pin() {
                Ok(is_detected) => {
                    let heat_detected = is_detected;

                    // Bug B22: with the SSR driven by a 5 Hz LEDC PWM (200 ms
                    // period) and `periodic_check` sampling once per ~100 ms
                    // control-loop tick, there are exactly 2 samples per PWM
                    // period in slowly drifting phase alignments. For duty
                    // <50 % the ON window is shorter than the sampling
                    // interval, so phase alignments exist where BOTH samples
                    // of a period land in the OFF window even though the SSR
                    // is functioning correctly. The previous `current_duty > 0`
                    // gate counted those legitimate low-power ticks as
                    // mismatches, reaching `HEAT_MISMATCH_MAX = 5` in ≤500 ms
                    // and latching `hardware_status = Error` mid-roast during
                    // normal low-power operation. Only declare a mismatch when
                    // the ON window is observably wide at this cadence
                    // (≥50 % duty = one full sample interval of ON per
                    // period), so the cross-check cannot alias with the PWM.
                    let min_observable_ticks = (1u32 << SSR_PWM_RESOLUTION) / 2;
                    let duty_observable = (current_duty as u32) >= min_observable_ticks;

                    if duty_observable && !heat_detected {
                        self.heat_mismatch_count = self.heat_mismatch_count.saturating_add(1);
                        warn!(
                            "Heat detection mismatch: heater ON (duty {}) but no heat detected (mismatch count: {})",
                            current_duty, self.heat_mismatch_count
                        );

                        if self.heat_mismatch_count >= HEAT_MISMATCH_MAX {
                            error!("Heat detection mismatch limit reached - SSR error");
                            self.hardware_status = SsrHardwareStatus::Error;
                            return Err(SsrError::HeatSourceNotDetected {
                                source: "heat_mismatch_limit_reached",
                            });
                        }
                    } else if current_duty == 0 && heat_detected {
                        // Residual heat after cut-off — the metal mass stays
                        // hot. Single-sample trips (the old behaviour) caused
                        // spurious SSR shutdowns mid-roast. Require
                        // HEAT_PRESENT_MISMATCH_MAX consecutive samples
                        // (≈ 2 s at 100 ms/tick) before declaring the SSR
                        // physically stuck on.
                        self.heat_present_count = self.heat_present_count.saturating_add(1);
                        warn!(
                            "Heat present with heater off (count: {}/{}) — possible SSR stuck-on",
                            self.heat_present_count, HEAT_PRESENT_MISMATCH_MAX
                        );
                        if self.heat_present_count >= HEAT_PRESENT_MISMATCH_MAX {
                            error!(
                                "SSR stuck-on detected: heat {} samples after heater off",
                                self.heat_present_count
                            );
                            self.hardware_status = SsrHardwareStatus::Error;
                            return Err(SsrError::HeatSourceNotDetected {
                                source: "ssr_stuck_on_detected",
                            });
                        }
                    } else {
                        self.heat_mismatch_count = 0;
                        self.heat_present_count = 0;
                    }

                    Ok(())
                }
                Err(_) => {
                    if self.hardware_status != SsrHardwareStatus::Error {
                        error!(
                            "SSR detection pin error during cross-check - switching to error state"
                        );
                        self.hardware_status = SsrHardwareStatus::Error;
                    }
                    Err(SsrError::InputError {
                        source: "detection_pin_read_failed_during_cross_check",
                    })
                }
            }
        }
    }
}

impl Default for SsrControlBase {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusGetters for SsrControlBase {
    fn get_hardware_status(&self) -> SsrHardwareStatus {
        self.hardware_status
    }

    fn is_heating_available(&self) -> bool {
        self.hardware_status == SsrHardwareStatus::Available
    }

    fn get_current_duty(&self) -> u16 {
        self.current_duty
    }

    fn is_pwm_enabled(&self) -> bool {
        self.is_pwm_enabled
    }

    fn last_lead_delta_ticks(&self) -> i16 {
        self.last_duty_delta_ticks
    }

    fn last_retry_count(&self) -> u8 {
        self.retry_count
    }
}

#[cfg(all(test, not(target_arch = "riscv32")))]
mod tests {
    use super::*;
    use crate::hardware::heat_presence::HEAT_ABSENT_DEBOUNCE;

    fn base_with_duty(duty: u16) -> SsrControlBase {
        let mut base = SsrControlBase::new();
        base.current_duty = duty;
        base
    }

    const DUTY_OBSERVABLE: u16 = (1u16 << (SSR_PWM_RESOLUTION - 1)) + 1;

    #[test]
    fn rearm_restores_available_from_not_detected() {
        let mut base = SsrControlBase::new();
        base.hardware_status = SsrHardwareStatus::NotDetected;
        base.heat_absent_count = 7;
        base.rearm();
        assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
        assert_eq!(base.heat_absent_count, 0);
    }

    #[test]
    fn rearm_restores_available_from_error_and_zeroes_all_counters() {
        let mut base = SsrControlBase::new();
        base.hardware_status = SsrHardwareStatus::Error;
        base.heat_absent_count = 3;
        base.heat_mismatch_count = 4;
        base.heat_present_count = 9;
        base.rearm();
        assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
        assert_eq!(base.heat_absent_count, 0);
        assert_eq!(base.heat_mismatch_count, 0);
        assert_eq!(base.heat_present_count, 0);
    }

    #[test]
    fn rearm_is_idempotent_when_available() {
        let mut base = SsrControlBase::new();
        base.rearm();
        base.rearm();
        assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
    }

    #[test]
    fn low_duty_gate_never_changes_status_without_low_sample() {
        let mut base = base_with_duty(100);
        base.hardware_status = SsrHardwareStatus::Available;
        for _ in 0..(HEAT_ABSENT_DEBOUNCE * 4) {
            let result = base.detect_heat_source(0, || Ok::<bool, ()>(false));
            assert!(result.is_ok());
            assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
            assert_eq!(base.heat_absent_count, 0);
        }
    }

    #[test]
    fn low_duty_low_sample_clears_latch() {
        let mut base = base_with_duty(100);
        base.hardware_status = SsrHardwareStatus::NotDetected;
        base.detect_heat_source(0, || Ok::<bool, ()>(true))
            .expect("detect must succeed");
        assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
    }

    #[test]
    fn not_detected_requires_debounce_consecutive_absent_samples() {
        let mut base = base_with_duty(DUTY_OBSERVABLE);
        for _ in 0..HEAT_ABSENT_DEBOUNCE - 1 {
            base.detect_heat_source(0, || Ok::<bool, ()>(false))
                .expect("detect must succeed");
            assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
        }
        base.detect_heat_source(0, || Ok::<bool, ()>(false))
            .expect("detect must succeed");
        assert_eq!(base.hardware_status, SsrHardwareStatus::NotDetected);
    }

    #[test]
    fn low_sample_restores_available_immediately() {
        let mut base = base_with_duty(DUTY_OBSERVABLE);
        base.hardware_status = SsrHardwareStatus::NotDetected;
        base.heat_absent_count = 4;
        base.detect_heat_source(0, || Ok::<bool, ()>(true))
            .expect("detect must succeed");
        assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
        assert_eq!(base.heat_absent_count, 0);
    }

    #[test]
    fn pin_read_error_latches_error_and_returns_input_error() {
        let mut base = base_with_duty(DUTY_OBSERVABLE);
        let result = base.detect_heat_source(0, || Err::<bool, ()>(()));
        assert!(matches!(
            result,
            Err(SsrError::InputError {
                source: "detection_pin_read_failed"
            })
        ));
        assert_eq!(base.hardware_status, SsrHardwareStatus::Error);
    }

    // `cross_check_heat_detection` is intentionally a no-op when the physical
    // heat-presence pin is absent (`simulated-sensors` / `no-heat-sense`), so
    // these latch tests only exercise the real-pin path.
    #[cfg(not(any(feature = "simulated-sensors", feature = "no-heat-sense")))]
    #[test]
    fn stuck_on_requires_ten_consecutive_heat_samples_at_zero_duty() {
        let mut base = base_with_duty(0);
        for _ in 0..9 {
            base.cross_check_heat_detection(0, || Ok::<bool, ()>(true))
                .expect("cross-check must not fail yet");
            assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
        }
        let result = base.cross_check_heat_detection(0, || Ok::<bool, ()>(true));
        assert!(matches!(
            result,
            Err(SsrError::HeatSourceNotDetected {
                source: "ssr_stuck_on_detected"
            })
        ));
        assert_eq!(base.hardware_status, SsrHardwareStatus::Error);
    }

    #[cfg(not(any(feature = "simulated-sensors", feature = "no-heat-sense")))]
    #[test]
    fn heat_mismatch_latches_error_after_five_samples() {
        let mut base = base_with_duty(DUTY_OBSERVABLE);
        for _ in 0..4 {
            base.cross_check_heat_detection(DUTY_OBSERVABLE, || Ok::<bool, ()>(false))
                .expect("cross-check must not fail yet");
            assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
        }
        let result = base.cross_check_heat_detection(DUTY_OBSERVABLE, || Ok::<bool, ()>(false));
        assert!(matches!(
            result,
            Err(SsrError::HeatSourceNotDetected {
                source: "heat_mismatch_limit_reached"
            })
        ));
        assert_eq!(base.hardware_status, SsrHardwareStatus::Error);
    }

    #[test]
    fn low_duty_cross_check_never_accumulates_mismatch() {
        let mut base = base_with_duty(100);
        for _ in 0..(HEAT_MISMATCH_MAX * 2) {
            base.cross_check_heat_detection(100, || Ok::<bool, ()>(false))
                .expect("cross-check must succeed");
            assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
        }
    }

    #[test]
    fn property_every_latched_state_is_recoverable() {
        for latched in [SsrHardwareStatus::NotDetected, SsrHardwareStatus::Error] {
            let mut via_rearm = SsrControlBase::new();
            via_rearm.hardware_status = latched;
            via_rearm.rearm();
            assert_eq!(via_rearm.hardware_status, SsrHardwareStatus::Available);

            let mut via_low_sample = base_with_duty(100);
            via_low_sample.hardware_status = latched;
            via_low_sample
                .detect_heat_source(0, || Ok::<bool, ()>(true))
                .expect("detect must succeed");
            assert_eq!(via_low_sample.hardware_status, SsrHardwareStatus::Available);
        }
    }
}
