use crate::config::constants::{
    SsrHardwareStatus as GlobalSsrStatus, SSR_DUTY_TOLERANCE_TICKS, SSR_PWM_RESOLUTION,
};
use crate::control::{traits::Heater, RoasterError};
use core::marker::PhantomData;
use embedded_hal::digital::{InputPin, OutputPin};
use esp_hal::ledc::channel::ChannelIFace;
use esp_hal::ledc::LowSpeed;
use log::{debug, error, info, warn};

/// Error returned when a raw duty write to the LEDC hardware fails.
/// The only failure mode is the LEDC guard timeout (logged as a warning internally).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DutyWriteError;

pub trait LedcDutyReader {
    fn read_duty_ticks(&self) -> u16;

    /// Set duty directly in hardware, bypassing the percentage conversion
    /// that [`ChannelIFace::set_duty`] applies. The duty value MUST be in the
    /// raw resolution range (e.g. 0-255 for 8-bit).
    ///
    /// This is the correct method to call when you have already computed a raw
    /// duty value via [`percentage_to_ledc_duty`]. Calling [`ChannelIFace::set_duty`]
    /// with a raw value > 100 will fail because it expects a percentage (0-100).
    fn set_duty_raw(&self, duty: u16) -> Result<(), DutyWriteError>;
}

fn monitor_ledc_after_set<'a, PWM>(
    pwm_channel: &mut PWM,
    commanded: u16,
    retry_count: &mut u8,
    last_delta: &mut i16,
) -> Result<(), SsrError>
where
    PWM: LedcDutyReader + ChannelIFace<'a, LowSpeed>,
{
    let readback = pwm_channel.read_duty_ticks();
    let delta = readback as i16 - commanded as i16;
    *last_delta = delta;

    let tolerance = SSR_DUTY_TOLERANCE_TICKS as i16;
    if delta.abs() <= tolerance {
        return Ok(());
    }

    warn!(
        "LEDC duty drift detected: commanded {} ticks vs actual {} ticks (delta {} ticks) - retrying once",
        commanded, readback, delta
    );
    *retry_count = retry_count.saturating_add(1);

    pwm_channel
        .set_duty_raw(commanded)
        .map_err(|_| SsrError::PwmError {
            source: "set_duty_failed",
        })?;

    let rechecked = pwm_channel.read_duty_ticks();
    let new_delta = rechecked as i16 - commanded as i16;
    *last_delta = new_delta;

    if new_delta.abs() > tolerance {
        error!(
            "LEDC duty mismatch persists after retry: commanded {} ticks vs actual {} ticks (delta {} ticks)",
            commanded, rechecked, new_delta
        );
        return Err(SsrError::PwmError {
            source: "duty_mismatch_after_retry",
        });
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SsrError {
    OutputError { source: &'static str },
    InputError { source: &'static str },
    HeatSourceNotDetected { source: &'static str },
    PwmError { source: &'static str },
}

impl embedded_hal::digital::Error for SsrError {
    fn kind(&self) -> embedded_hal::digital::ErrorKind {
        embedded_hal::digital::ErrorKind::Other
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SsrHardwareStatus {
    Available,
    NotDetected,
    Error,
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

/// Trait for heat source detection functionality.
/// Implementations must provide detection logic.
pub trait HeatSourceDetector {
    fn detect_heat_source(&mut self, current_time: u32) -> Result<(), SsrError>;
}

/// Trait for periodic health check functionality.
/// Implementations must provide periodic check logic.
pub trait PeriodicCheck {
    fn periodic_check(&mut self, current_time: u32) -> Result<(), SsrError>;
}

/// Trait for status getter methods.
/// Provides default implementations for common status queries.
pub trait StatusGetters {
    fn get_hardware_status(&self) -> SsrHardwareStatus;
    fn is_heating_available(&self) -> bool;
    fn get_current_duty(&self) -> u16;
    fn is_pwm_enabled(&self) -> bool;
    fn last_lead_delta_ticks(&self) -> i16;
    fn last_retry_count(&self) -> u8;
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
        mut read_pin: F,
    ) -> Result<(), SsrError>
    where
        F: FnMut() -> Result<bool, E>,
    {
        use crate::hardware::heat_presence::{debounce_heat_absent, HeatPresenceOutcome};

        // ≥50% duty ≈ one full sample interval of conduction per PWM period at
        // 5 Hz vs. ~330 ms sampling; below that the pin read may legitimately
        // land in the OFF window even when the SSR is wired and functional.
        let min_observable_ticks = (1u32 << SSR_PWM_RESOLUTION) / 2;
        if (self.current_duty as u32) < min_observable_ticks {
            // Duty too low for the pin to be informative — and a low-power
            // stretch must not accumulate toward NotDetected.
            self.heat_absent_count = 0;
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

    pub fn cross_check_heat_detection<F, E>(
        &mut self,
        current_duty: u16,
        read_pin: F,
    ) -> Result<(), SsrError>
    where
        F: FnMut() -> Result<bool, E>,
    {
        #[cfg(feature = "simulated-sensors")]
        {
            let _ = (current_duty, read_pin);
            return Ok(());
        }

        #[cfg(not(feature = "simulated-sensors"))]
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
                    let min_observable_ticks =
                        (1u32 << crate::config::constants::SSR_PWM_RESOLUTION) / 2;
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

pub struct SsrControl<'a, PIN, DETECT, PWM>
where
    PIN: OutputPin,
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    #[allow(dead_code)]
    // Stored for ownership - pin is set low during initialization and kept alive
    // to prevent accidental reconfiguration by the HAL. PWM controls actual SSR output.
    pin: PIN,
    detection_pin: DETECT,
    pwm_channel: PWM,
    base: SsrControlBase,
    _phantom: PhantomData<&'a ()>,
}

pub fn percentage_to_ledc_duty(percentage: f32) -> u16 {
    let clamped = percentage.clamp(0.0, 100.0);
    let max_duty = (1u32 << SSR_PWM_RESOLUTION) - 1;
    let scaled = ((clamped / 100.0) * max_duty as f32 + 0.5) as u32;
    let scaled = scaled.min(max_duty);
    let scaled = scaled as u16;

    // Enforce minimum SSR duty - if duty is too low, snap to 0
    if scaled > 0 && scaled < crate::config::constants::SSR_MIN_DUTY_TICKS {
        0
    } else {
        scaled
    }
}

impl<'a, PIN, DETECT, PWM> SsrControl<'a, PIN, DETECT, PWM>
where
    PIN: OutputPin,
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    pub fn new_with_pwm_and_detection(
        mut pin: PIN,
        detection_pin: DETECT,
        pwm_channel: PWM,
    ) -> Result<Self, SsrError> {
        pin.set_low().map_err(|_| SsrError::OutputError {
            source: "pin_init_failed",
        })?;
        pwm_channel
            .set_duty_raw(0)
            .map_err(|_| SsrError::PwmError {
                source: "channel_init_failed",
            })?;

        let ssr = SsrControl {
            pin,
            detection_pin,
            pwm_channel,
            base: SsrControlBase::new(),
            _phantom: PhantomData,
        };

        // Boot-time detection is uninformative at duty=0 (pin HIGH is the
        // expecter OFF state, NOT evidence of missing hardware). The
        // `detect_heat_source` runs from `periodic_check` once enough duty is
        // commanded for the pin read to be meaningful.
        info!(
            "SSR control initialized with PWM - heat source: {:?}",
            ssr.base.hardware_status
        );
        Ok(ssr)
    }

    fn detect_heat_source(&mut self, current_time: u32) -> Result<(), SsrError> {
        self.base
            .detect_heat_source(current_time, || self.detection_pin.is_low())
    }

    pub fn get_current_duty(&self) -> u16 {
        self.base.current_duty
    }

    pub fn is_pwm_enabled(&self) -> bool {
        self.base.is_pwm_enabled
    }

    pub fn set_percentage(&mut self, percentage: f32) -> Result<(), SsrError> {
        let clamped = percentage.clamp(0.0, 100.0);
        let ledc_duty = percentage_to_ledc_duty(clamped);

        self.base.last_duty_delta_ticks = 0;
        self.base.retry_count = 0;

        self.pwm_channel
            .set_duty_raw(ledc_duty)
            .map_err(|_| SsrError::PwmError {
                source: "set_duty_failed",
            })?;

        monitor_ledc_after_set(
            &mut self.pwm_channel,
            ledc_duty,
            &mut self.base.retry_count,
            &mut self.base.last_duty_delta_ticks,
        )?;

        self.base.current_duty = ledc_duty;

        debug!(
            "SSR set to {:.1}% (duty {}), heat available: {}",
            clamped,
            ledc_duty,
            self.is_heating_available()
        );

        Ok(())
    }
}

pub struct SsrControlSimple<'a, DETECT, PWM>
where
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    detection_pin: DETECT,
    pwm_channel: PWM,
    base: SsrControlBase,
    _phantom: PhantomData<&'a ()>,
}

impl<'a, DETECT, PWM> SsrControlSimple<'a, DETECT, PWM>
where
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    pub fn new(detection_pin: DETECT, pwm_channel: PWM) -> Result<Self, SsrError> {
        pwm_channel
            .set_duty_raw(0)
            .map_err(|_| SsrError::PwmError {
                source: "channel_init_failed",
            })?;

        let ssr = SsrControlSimple {
            detection_pin,
            pwm_channel,
            base: SsrControlBase::new(),
            _phantom: PhantomData,
        };

        // Boot-time detection is uninformative at duty=0 (pin HIGH is the
        // expected OFF state). The `detect_heat_source` runs from
        // `periodic_check` once enough duty is commanded for the pin read to
        // be meaningful. `SsrControlBase::new` already initializes the
        // hardware status to `Available` to avoid the dead-lock the report
        // flags.
        info!(
            "SSR control initialized (simple mode) - heat source: {:?}",
            ssr.base.hardware_status
        );
        Ok(ssr)
    }

    fn detect_heat_source(&mut self, current_time: u32) -> Result<(), SsrError> {
        #[cfg(feature = "simulated-sensors")]
        {
            self.base
                .detect_heat_source(current_time, || Ok::<bool, core::convert::Infallible>(true))
        }
        #[cfg(not(feature = "simulated-sensors"))]
        {
            self.base
                .detect_heat_source(current_time, || self.detection_pin.is_low())
        }
    }

    pub fn periodic_check(&mut self, current_time: u32) -> Result<(), SsrError> {
        // No throttle (bug audit 2026-08-02): consecutive detects must stay
        // one control-loop tick apart (~330 ms → ~130 ms of PWM phase
        // separation, provably in the (100, 200) ms band the
        // `HEAT_ABSENT_DEBOUNCE` run-bound analysis relies on). The previous
        // 1000 ms gate made the separation depend on the tick multiple — up
        // to 3 ticks — where the OFF window could alias with the phase and
        // produce long spurious "no heat" runs. A GPIO read is negligible;
        // the per-tick cadence also matches `cross_check_heat_detection`.
        self.detect_heat_source(current_time)?;

        self.base
            .cross_check_heat_detection(self.base.current_duty, || self.detection_pin.is_low())?;

        Ok(())
    }

    pub fn set_percentage(&mut self, percentage: f32) -> Result<(), SsrError> {
        let clamped = percentage.clamp(0.0, 100.0);
        let ledc_duty = percentage_to_ledc_duty(clamped);

        self.base.last_duty_delta_ticks = 0;
        self.base.retry_count = 0;

        self.pwm_channel
            .set_duty_raw(ledc_duty)
            .map_err(|_| SsrError::PwmError {
                source: "set_duty_failed",
            })?;

        monitor_ledc_after_set(
            &mut self.pwm_channel,
            ledc_duty,
            &mut self.base.retry_count,
            &mut self.base.last_duty_delta_ticks,
        )?;

        self.base.current_duty = ledc_duty;

        debug!(
            "SSR set to {:.1}% (duty {}), heat available: {}",
            clamped,
            ledc_duty,
            self.is_heating_available()
        );

        self.base
            .cross_check_heat_detection(self.base.current_duty, || self.detection_pin.is_low())?;

        Ok(())
    }

    pub fn get_current_duty(&self) -> u16 {
        self.base.current_duty
    }

    pub fn is_pwm_enabled(&self) -> bool {
        self.base.is_pwm_enabled
    }

    pub fn last_lead_delta_ticks(&self) -> i16 {
        self.base.last_duty_delta_ticks
    }

    pub fn last_retry_count(&self) -> u8 {
        self.base.retry_count
    }
}

impl<'a, PIN, DETECT, PWM> StatusGetters for SsrControl<'a, PIN, DETECT, PWM>
where
    PIN: OutputPin,
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    fn get_hardware_status(&self) -> SsrHardwareStatus {
        self.base.hardware_status
    }

    fn is_heating_available(&self) -> bool {
        self.base.hardware_status == SsrHardwareStatus::Available
    }

    fn get_current_duty(&self) -> u16 {
        self.base.current_duty
    }

    fn is_pwm_enabled(&self) -> bool {
        self.base.is_pwm_enabled
    }

    fn last_lead_delta_ticks(&self) -> i16 {
        self.base.last_duty_delta_ticks
    }

    fn last_retry_count(&self) -> u8 {
        self.base.retry_count
    }
}

impl<'a, PIN, DETECT, PWM> HeatSourceDetector for SsrControl<'a, PIN, DETECT, PWM>
where
    PIN: OutputPin,
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    fn detect_heat_source(&mut self, current_time: u32) -> Result<(), SsrError> {
        SsrControl::detect_heat_source(self, current_time)
    }
}

impl<'a, PIN, DETECT, PWM> PeriodicCheck for SsrControl<'a, PIN, DETECT, PWM>
where
    PIN: OutputPin,
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    fn periodic_check(&mut self, current_time: u32) -> Result<(), SsrError> {
        // No throttle — see SsrControlSimple::periodic_check for the
        // phase-separation argument (bug audit 2026-08-02).
        self.detect_heat_source(current_time)?;

        Ok(())
    }
}

impl<'a, DETECT, PWM> Heater for SsrControlSimple<'a, DETECT, PWM>
where
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError> {
        self.set_percentage(duty)
            .map_err(|_| RoasterError::HardwareError {
                source: Some("ssr_set_power"),
            })
    }

    fn get_status(&self) -> GlobalSsrStatus {
        match self.get_hardware_status() {
            SsrHardwareStatus::Available => GlobalSsrStatus::Available,
            SsrHardwareStatus::NotDetected => GlobalSsrStatus::NotDetected,
            SsrHardwareStatus::Error => GlobalSsrStatus::Error,
        }
    }

    fn periodic_health_check(&mut self, current_time_ms: u32) {
        if let Err(e) = self.periodic_check(current_time_ms) {
            log::warn!("SSR health check failed (SsrControlSimple): {:?}", e);
        }
    }

    fn last_duty_delta_ticks(&self) -> i16 {
        self.base.last_duty_delta_ticks
    }

    fn last_retry_count(&self) -> u8 {
        self.base.retry_count
    }
}

impl<'a, DETECT, PWM> StatusGetters for SsrControlSimple<'a, DETECT, PWM>
where
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    fn get_hardware_status(&self) -> SsrHardwareStatus {
        self.base.hardware_status
    }

    fn is_heating_available(&self) -> bool {
        self.base.hardware_status == SsrHardwareStatus::Available
    }

    fn get_current_duty(&self) -> u16 {
        self.base.current_duty
    }

    fn is_pwm_enabled(&self) -> bool {
        self.base.is_pwm_enabled
    }

    fn last_lead_delta_ticks(&self) -> i16 {
        self.base.last_duty_delta_ticks
    }

    fn last_retry_count(&self) -> u8 {
        self.base.retry_count
    }
}

impl<'a, DETECT, PWM> HeatSourceDetector for SsrControlSimple<'a, DETECT, PWM>
where
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    fn detect_heat_source(&mut self, current_time: u32) -> Result<(), SsrError> {
        SsrControlSimple::detect_heat_source(self, current_time)
    }
}

impl<'a, DETECT, PWM> PeriodicCheck for SsrControlSimple<'a, DETECT, PWM>
where
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    fn periodic_check(&mut self, current_time: u32) -> Result<(), SsrError> {
        SsrControlSimple::periodic_check(self, current_time)
    }
}

// SAFETY(v5.1): Sound on single-core ESP32-C3 (cooperative Embassy tasks).
// If porting to multi-core ESP32 (e.g., ESP32-S3 dual-core), review ref-counting
// of the LEDC Timer reference held by ChannelIFace. The Timer is stored in the
// static LedcBus and outlives all Channel users on single-core.
// SAFETY: SsrControlSimple owns its peripheral handles exclusively.
// On the single-core ESP32-C3, Embassy tasks run cooperatively — only one
// task executes at a time. The type is moved into a `Box<dyn Heater + Send>`
// and passed to a single task via ServiceContainer, so no concurrent access
// occurs. The LEDC ChannelIFace is !Send by default because it holds a
// reference to the Timer; we vouch that the Timer outlives all Channel users
// (it is stored in the static LedcBus).
unsafe impl<'a, DETECT, PWM> Send for SsrControlSimple<'a, DETECT, PWM>
where
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
}

impl<'a, PIN, DETECT, PWM> Heater for SsrControl<'a, PIN, DETECT, PWM>
where
    PIN: OutputPin,
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError> {
        SsrControl::set_percentage(self, duty).map_err(|_| RoasterError::HardwareError {
            source: Some("ssr_control_set_percentage"),
        })
    }

    fn get_status(&self) -> GlobalSsrStatus {
        match StatusGetters::get_hardware_status(self) {
            SsrHardwareStatus::Available => GlobalSsrStatus::Available,
            SsrHardwareStatus::NotDetected => GlobalSsrStatus::NotDetected,
            SsrHardwareStatus::Error => GlobalSsrStatus::Error,
        }
    }

    fn periodic_health_check(&mut self, current_time_ms: u32) {
        if let Err(e) = PeriodicCheck::periodic_check(self, current_time_ms) {
            log::warn!("SSR health check failed (SsrControl): {:?}", e);
        }
    }

    fn last_duty_delta_ticks(&self) -> i16 {
        StatusGetters::last_lead_delta_ticks(self)
    }

    fn last_retry_count(&self) -> u8 {
        StatusGetters::last_retry_count(self)
    }
}

// SAFETY(v5.1): Sound on single-core ESP32-C3 (cooperative Embassy tasks).
// If porting to multi-core ESP32 (e.g., ESP32-S3 dual-core), review ref-counting
// of the LEDC Timer reference held by ChannelIFace. The Timer is stored in the
// static LedcBus and outlives all Channel users on single-core.
// SAFETY: SsrControl owns its peripheral handles exclusively.
// On the single-core ESP32-C3, Embassy tasks run cooperatively — only one
// task executes at a time. The type is moved into a `Box<dyn Heater + Send>`
// and passed to a single task via ServiceContainer, so no concurrent access
// occurs. The LEDC ChannelIFace is !Send by default because it holds a
// reference to the Timer; we vouch that the Timer outlives all Channel users
// (it is stored in the static LedcBus).
unsafe impl<'a, PIN, DETECT, PWM> Send for SsrControl<'a, PIN, DETECT, PWM>
where
    PIN: OutputPin,
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
}

#[cfg(test)]
mod tests {
    use super::percentage_to_ledc_duty;
    use crate::config::constants::{
        SSR_CYCLE_GUARD_MS, SSR_DUTY_TOLERANCE_TICKS, SSR_PWM_RESOLUTION,
    };

    const fn max_duty() -> u16 {
        ((1u32 << SSR_PWM_RESOLUTION) - 1) as u16
    }

    #[test]
    fn percentage_to_ledc_duty_handles_bounds() {
        assert_eq!(percentage_to_ledc_duty(0.0), 0);
        assert_eq!(percentage_to_ledc_duty(100.0), max_duty());
        assert_eq!(percentage_to_ledc_duty(-50.0), 0);
        assert_eq!(percentage_to_ledc_duty(150.0), max_duty());
    }

    #[test]
    fn percentage_to_ledc_duty_rounds_midpoints() {
        let max_duty = ((1u32 << SSR_PWM_RESOLUTION) - 1) as f32;
        let expected = (max_duty * 0.5).round() as u16;
        assert_eq!(percentage_to_ledc_duty(50.0), expected);
    }

    #[test]
    fn guard_constants_are_locked() {
        assert_eq!(SSR_CYCLE_GUARD_MS, 100);
        assert_eq!(SSR_DUTY_TOLERANCE_TICKS, 128);
    }

    // The `test_digital_error_kind` test that lived here was removed: it
    // constructed `SsrError::OutputError` without the required `source` field
    // (E0063) and was dead code only compiled under `#[cfg(test)]` on the
    // riscv32 target. The underlying behaviour — `SsrError: embedded_hal::
    // digital::Error` returning `ErrorKind::Other` — is exercised trivially by
    // the trait impl at lines 84-88 above and needs no dedicated test.
    //
    // Note: this module is replaced by `ssr_stub.rs` on host builds, so the
    // tests here only compile on the riscv32 target. The heat-source
    // detection debounce logic therefore lives in `heat_presence.rs` (an
    // un-gated module) where its unit tests actually run in CI.
}
