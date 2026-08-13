// GPIO Pin Assignments for LibreRoaster ESP32-C3
// These pins are optimized for the ESP32-C3 capabilities and coffee roaster application
// Note: ESP32-C3 has strapping pins (GPIO2, GPIO8, GPIO9). GPIO2 must be avoided.
// GPIO9 is a strapping pin but is used for FAN PWM in this project;
// ensure the external fan driver does not force an invalid strap level during reset/boot.
//
// SPI2 (FSPI) IO MUX functions on ESP32-C3:
//   FSPICLK (SCK)  : GPIO6
//   FSPID   (MOSI) : GPIO7
//   FSPIQ   (MISO) : GPIO2 (strapping, avoid) → use GPIO5 via GPIO Matrix

pub const SPI_SCLK_PIN: u8 = 6;
pub const SPI_MOSI_PIN: u8 = 7;
pub const SPI_MISO_PIN: u8 = 5;
pub const THERMOCOUPLE_BT_CS_PIN: u8 = 4;
pub const THERMOCOUPLE_ET_CS_PIN: u8 = 3;
pub const SSR_CONTROL_PIN: u8 = 10;
pub const HEAT_DETECTION_PIN: u8 = 1;
pub const FAN_PWM_PIN: u8 = 9;
pub const STATUS_LED_PIN: u8 = 8;
pub const UART_TX_PIN: u8 = 21;
pub const UART_RX_PIN: u8 = 20;

pub const FAN_PWM_FREQUENCY_HZ: u32 = 25000;
/// SSR control cycle frequency in Hz.
///
/// 5 Hz = 200 ms period (20 AC half-cycles at 50 Hz mains). Compatible with
/// zero-cross SSRs (SSR-25DA, etc.). Minimum achievable on ESP32-C3 at 14-bit
/// resolution (APBClk 80 MHz, max LEDC divider 262 143, no REF_TICK available).
pub const SSR_CONTROL_CYCLE_HZ: u32 = 5;
pub const FAN_LEDC_CHANNEL: u8 = 0;
pub const SSR_LEDC_CHANNEL: u8 = 1;
pub const SSR_LEDC_TIMER: u8 = 0;
pub const FAN_LEDC_TIMER: u8 = 1;
/// SSR LEDC duty resolution in bits (14-bit → 16 384 steps).
pub const SSR_PWM_RESOLUTION: u8 = 14;
/// Fan LEDC duty resolution in bits (8-bit → 256 steps).
pub const FAN_PWM_RESOLUTION: u8 = 8;

/// Bug A (2026-08-03): minimum fan speed (%) enforced whenever the heater is
/// energized. The fan selector in `update_control` otherwise falls through to
/// `artisan_manual_fan()` — which defaults to 0.0 when the operator sent no
/// `OT2` / `FANPROFILE` — so a documented PID roast (SETTARGET+START, or
/// PREHEAT without a fan profile) ran the SSR at up to 100 % with ZERO airflow
/// every tick. The firmware's own standard treats 'no fan' as unsafe
/// (`stop_streaming`: "no fan means unsafe to continue"); this floor applies
/// that standard to the energizing path too. It is a one-way safety valve on
/// heater-on / fan-off only: any commanded fan at or above the floor passes
/// through untouched, and fan control below this value while the heater fires
/// is simply not permitted (an explicit `OT2 0` with heat on is overridden).
pub const FAN_MIN_SAFETY_PCT: f32 = 20.0;
/// Minimum interval between SSR duty updates in ms.
pub const SSR_CYCLE_GUARD_MS: u32 = 100;
/// Maximum allowed drift between commanded and actual LEDC duty (raw ticks).
/// At 14-bit, 128 ticks ≈ 0.78 % of full scale.
pub const SSR_DUTY_TOLERANCE_TICKS: u16 = 128;
/// Minimum non-zero duty in raw ticks. At 14-bit / 5 Hz, one full PWM
/// cycle is 200 ms; one AC half-cycle at 50 Hz mains is 10 ms. With a
/// non-zero-cross-synchronised LEDC output, a ~2.4 ms ON pulse (193 ticks,
/// the previous value) only coincides with a zero crossing ~25-30 % of the
/// time, so a zero-cross SSR fires erratically at the minimum commanded
/// power. Bug B28: raise the floor to one AC half-cycle (10 ms ≈ 820 ticks
/// at 14-bit / 5 Hz) so a zero-cross SSR reliably lands at least one
/// half-cycle of mains on every active PWM period, giving a deterministic
/// minimum delivered power. Use 1639 (a full 20 ms mains cycle) if DC bias
/// across the mains is a concern on the target SSR.
pub const SSR_MIN_DUTY_TICKS: u16 = 820;

pub const DEFAULT_TARGET_TEMP: f32 = 225.0;
// Bug M7 (2026-08-10): `MAX_SAFE_TEMP`/`MIN_TEMP` were dead — nothing
// applied them, and the REAL control-target range was a hand-written literal
// in `is_valid_target_temp` (below). A "maximum safe temperature" constant
// that no code enforces is worse than none: removed. `MAX_TEMP` now feeds
// `MAX_TARGET_TEMP`, so editing it actually changes what SETTARGET/PREHEAT/
// PROFILE accept.
pub const MAX_TEMP: f32 = 300.0;
pub const MIN_VALID_TEMP: f32 = -50.0;
pub const MAX_VALID_TEMP: f32 = 350.0;

/// PID control loop sample time in milliseconds.
/// Sensor reads (~160ms) may exceed this interval, see stale-data guard in update_control().
pub const PID_SAMPLE_TIME_MS: u32 = 100;
/// MAX31856 thermocouple read time in milliseconds (SPI + conversion latency).
/// Exceeds PID_SAMPLE_TIME_MS; stale-data guard prevents PID from using old readings.
pub const TEMPERATURE_READ_INTERVAL_MS: u32 = 160;
/// MAX31856 one-shot conversion wait at 50 Hz notch filter, in milliseconds.
///
/// The datasheet specifies up to 185 ms for a 50 Hz-filtered conversion. We
/// add a margin (25 ms) to ensure the conversion-complete bit is set before
/// we read the result registers. Bug #B1: the previous wait used
/// `TEMPERATURE_READ_INTERVAL_MS` (160 ms), which is shorter than 50 Hz
/// conversion time — meaning each read could silently return the *previous*
/// conversion's temperature (stale data with no error indication).
// Bug L17 (2026-08-10): the comment claimed a "190 ms" margin for years
// while the constant is 210 ms — updated to describe the actual value.
pub const MAX31856_CONVERSION_TIME_MS: u64 = 210;

pub const OVERTEMP_THRESHOLD: f32 = 260.0;
pub const TEMP_VALIDITY_TIMEOUT_MS: u32 = 1000;

/// Bean temperature (°C) below which the post-STOP cooldown fan latch
/// releases (bug B3). Beans below this temperature are cool enough that
/// forced airflow is no longer safety-critical, so the operator may resume
/// manual fan control. While the cooldown latch is active the fan stays at
/// 100% every tick regardless of the manual setting or fan profile.
pub const COOLING_RELEASE_BEAN_TEMP_C: f32 = 60.0;

/// Soft RoR guard threshold in °C/s (0.5 °C/s = 30 °C/min). Rates between
/// this value and `MAX_BT_RATE_OF_RISE_HARD` — the band where aggressive
/// light-roast turnarounds legitimately live for a few seconds — latch only
/// after `ROR_SOFT_DEBOUNCE_LIMIT` consecutive ticks. Above this threshold
/// during active heating indicates a possible runaway heater, stuck SSR, or
/// thermocouple failure.
pub const MAX_BT_RATE_OF_RISE: f32 = 0.5;
/// Hard RoR guard threshold in °C/s (1.0 °C/s = 60 °C/min). No legitimate
/// roast phase sustains this: rates above it latch after the FAST debounce
/// (`ROR_EXCEEDED_CONSECUTIVE_LIMIT`). Audit A-TC4-D (2026-08-12): both
/// thresholds are provisional pending hardware calibration (HIL).
pub const MAX_BT_RATE_OF_RISE_HARD: f32 = 1.0;
/// Consecutive RoR exceedances required before emergency shutdown in the
/// HARD band (> `MAX_BT_RATE_OF_RISE_HARD`) — ~1 s at the ~310 ms control
/// cadence. Prevents false triggers from single-spike sensor glitches.
pub const ROR_EXCEEDED_CONSECUTIVE_LIMIT: u8 = 3;
/// Consecutive RoR exceedances required before emergency shutdown in the
/// SOFT band (`MAX_BT_RATE_OF_RISE`..=`MAX_BT_RATE_OF_RISE_HARD`) — ~3.7 s
/// sustained at the ~310 ms control cadence. Audit A-TC4-D (2026-08-12): a
/// brief light-roast turnaround spike stays tolerated; a sustained marginal
/// climb still aborts. Provisional pending hardware calibration (HIL).
pub const ROR_SOFT_DEBOUNCE_LIMIT: u8 = 12;

/// Bug P5 (2026-08-03): probe-stuck detector — the heater output at or above
/// this percentage arms the detector: a heater this hot must move the BT
/// probe. If a probe holds a flat temperature while the heater runs this hot
/// for `PROBE_STUCK_TIMEOUT_SECS`, the thermocouple is shorted or broken and
/// `emergency_shutdown("Probe stuck")` fires.
pub const PROBE_STUCK_HEATER_MIN_PCT: f32 = 50.0;
/// Bug P5: the BT reading must vary by more than this many °C to count as a
/// live probe. A shorted thermocouple reads a flat ~0 °C — a VALID
/// temperature with no MAX31856 fault bit; a broken probe holds any flat
/// value. 1 °C over 2 minutes is far less than any real probe moves at
/// ≥ 50 % heater.
pub const PROBE_STUCK_VARIATION_C: f32 = 1.0;
/// Bug P5: consecutive seconds of flat BT with the heater on before the
/// probe-stuck detector reacts. In firmware-PID mode this is the emergency
/// latch threshold; in manual / Artisan software-PID mode (Audit A-TC4-C,
/// 2026-08-12) it is the WIRE-WARNING threshold — the latch lands at
/// `PROBE_STUCK_MANUAL_LATCH_SECS` instead.
pub const PROBE_STUCK_TIMEOUT_SECS: u64 = 120;
/// Audit A-TC4-C (2026-08-12): in manual / Artisan software-PID mode the
/// probe-stuck detector is two-stage: the wire warning fires at
/// `PROBE_STUCK_TIMEOUT_SECS`; the emergency latch only after this many
/// consecutive seconds of flat BT with the heater on. A legitimately slow
/// finish can hold BT < 1 °C for 2 min at low duty, but 5 min of flat BT
/// with heat applied is never a healthy roast — the dead-probe backstop
/// (Bug S1) stays closed with worst-case exposure at 5 min, still far under
/// `MAX_ROAST_TIME_SECS`.
pub const PROBE_STUCK_MANUAL_LATCH_SECS: u64 = 300;
/// Bug P5 (2026-08-03): the detector disarms while the PID is legitimately
/// REGULATING within this many °C of the setpoint. A stable roast holds BT
/// nearly flat by design (that is the PID's job), and on a cold ambient /
/// big drum the equilibrium duty can sit at or above
/// `PROBE_STUCK_HEATER_MIN_PCT` — without this margin a healthy steady-state
/// roast would trip a false "Probe stuck" emergency. The stuck-probe
/// signature is a flat BT FAR from the target the loop is chasing (a shorted
/// thermocouple reads ~0 °C against a 200 °C setpoint); near the target,
/// flat BT is expected control behaviour. Manual mode (`pid_enabled =
/// false`) has no regulation target, so it stays fully armed.
pub const PROBE_STUCK_TARGET_MARGIN_C: f32 = 5.0;
pub const SSR_DETECTION_TIMEOUT_MS: u32 = 100;
/// Number of retry attempts to turn off the heater during emergency shutdown.
pub const EMERGENCY_HEATER_OFF_RETRIES: u8 = 3;
/// Number of retry attempts to force the fan to 100 % during emergency
/// shutdown / emergency stop.
///
/// Bug B-L / B-H (2026-08-04): the fan previously got a single attempt while
/// the heater got `EMERGENCY_HEATER_OFF_RETRIES`. A failed fan write with a
/// hot bean mass is the exact "no fan = unsafe to continue" condition, so
/// cooling gets the same retry discipline as heater cut-off.
pub const EMERGENCY_FAN_RETRIES: u8 = 3;

pub const BT_THERMOCOUPLE_OFFSET: f32 = 0.0;
pub const ET_THERMOCOUPLE_OFFSET: f32 = 0.0;

pub const DEFAULT_OUTPUT_INTERVAL_MS: u64 = 1000;
pub const MAX_CONSECUTIVE_SENSOR_ERRORS: u8 = 5;

/// Control loop is expected to feed the Task Watchdog at this cadence.
///
/// Bug M6 (2026-08-10): used to claim `100` (the loop-timer period), but the
/// real cadence is `CONTROL_LOOP_TICK_MS` — one tick additionally waits
/// `MAX31856_CONVERSION_TIME_MS` for the sensor conversion. The compile-time
/// margin assertion below must bound the REAL cadence, or a future change to
/// the conversion time (it has happened once) could reset the chip every tick
/// with nothing flagging it.
pub const WATCHDOG_FEED_INTERVAL_MS: u64 = CONTROL_LOOP_TICK_MS as u64;
/// HW RWDT stage-0 hold in RC_SLOW_CLK cycles, as programmed by
/// `safety::watchdog::init` (single source of truth — was a local literal).
pub const HW_WATCHDOG_STAGE0_CYCLES: u32 = 300_000;
/// RC slow clock frequency in Hz (ESP32-C3 internal ~136 kHz oscillator).
pub const RC_SLOW_CLK_HZ: u32 = 136_000;
/// Nominal RWDT stage-0 timeout in ms (no efuse `wdt_delay_sel` shift).
/// ≈ 300000 / 136000 s ≈ 2206 ms. The efuse shift (×2..×16) can only
/// SHORTEN the real timeout, so it is deliberately excluded from the margin
/// check — the assertion guards the longest case, which is the one that can
/// mask a runaway loop.
pub const HW_WATCHDOG_TIMEOUT_MS: u64 =
    HW_WATCHDOG_STAGE0_CYCLES as u64 * 1000 / RC_SLOW_CLK_HZ as u64;
const _: () = assert!(
    WATCHDOG_FEED_INTERVAL_MS * 2 < HW_WATCHDOG_TIMEOUT_MS,
    "the tick must leave >=2x margin before the RWDT resets the chip"
);
pub const LEDC_GUARD_TIMEOUT_MS: u64 = 10;
/// Maximum idle time (ms) without any Artisan command before emergency shutdown.
/// During active roasting, Artisan sends periodic STATUS queries (~1s interval),
/// so 15s without any command indicates Artisan has crashed or disconnected.
pub const COMMS_IDLE_TIMEOUT_MS: u64 = 15000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RoasterState {
    Idle,
    Preheating,
    Heating,
    Stable,
    // Audit M-A7 (2026-08-11): `Cooling`, `Fault` and `EmergencyStop` removed
    // — zero references existed; every failure transition uses `Error`.
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArtisanCommand {
    ReadStatus,
    StatusReport,
    StartRoast,
    SetHeater(u8),
    SetFan(u8),
    SetFanSpeed(u8, bool),
    Stop,
    EmergencyStop,
    IncreaseHeater,
    DecreaseHeater,
    Chan(u16),
    Units(bool),
    Filt(u8),
    RunRegression,
    SetPidGain(f32, f32, f32),
    SetTargetTemp(f32),
    SetProfile,
    DumpLog,
    Preheat(f32),
    SetFanProfile,
    SetPidChannel(u8),            // PID;CHAN;2 — 1=ET, 2=BT (default)
    SetPidCycleTime(u32),         // PID;CT;1000 — cycle time in ms
    SetPidOutputLimits(f32, f32), // PID;LIMIT;0;100 — min/max output %
}

pub const MAX_PROFILE_SETPOINTS: usize = 16;
// Bug H3 (2026-08-10): the rate limiter in `drain_commands` DISCARDED
// commands beyond this budget (`try_receive` already removed them from the
// channel, then `continue` dropped them) — a burst of 12 commands with a
// `START` at position 12 silently lost the START. The bounded channel
// (ARTISAN_CMD_CHANNEL_SIZE = 16) already caps the work per tick; the extra
// budget only bought silent loss. Equalise so every command the channel can
// hold is also processed in the same tick (the emergency bypass stays).
pub const MAX_COMMANDS_PER_TICK: usize =
    crate::application::service_container::ARTISAN_CMD_CHANNEL_SIZE;
// Bug M2 (2026-07-25): the previous `20.0` was unreachable with a real BT
// probe (verified by simulation: a TC4-style drop is 2–3 °C/s, ≈ 6–9 °C in
// the 3 s sampling window). Drop the threshold to a probe-attainable value
// so `#CHARGE` fires reliably on the first real charge.
// Bug P10 (2026-08-03): `8.0` was still marginal — the real sampling window
// is `CHARGE_HISTORY_CAPACITY × CHARGE_SAMPLE_TICK_DIV × CONTROL_LOOP_TICK_MS`
// ≈ 3.1 s, so 8 °C demanded ≈ 2.6 °C/s sustained, at the very top of the
// typical 2–3 °C/s charge signature. `6.0` fires on a ≥ ~1.9 °C/s drop —
// comfortably inside the physical range with margin for a sluggish first
// charge.
pub const CHARGE_DROP_THRESHOLD_C: f32 = 6.0;
/// Bug B23: intended charge-detection window in seconds. The bean-drop
/// detector samples `bt_charge_history` (Deque<`CHARGE_HISTORY_CAPACITY`>)
/// once every `CHARGE_SAMPLE_TICK_DIV` control ticks (real cadence
/// `CONTROL_LOOP_TICK_MS`/tick), so the deque spans
/// `CHARGE_HISTORY_CAPACITY × CHARGE_SAMPLE_TICK_DIV × CONTROL_LOOP_TICK_MS`.
/// A >`CHARGE_DROP_THRESHOLD_C` °C BT drop in 3 s is the physical signature
/// of bean charge.
pub const CHARGE_DETECTION_WINDOW_S: u32 = 3;
/// Number of BT samples the charge-history deque holds. Forms half of the
/// `CHARGE_DETECTION_WINDOW_S` expression (see `CHARGE_SAMPLE_TICK_DIV`).
pub const CHARGE_HISTORY_CAPACITY: u32 = 10;
/// Control-loop period in milliseconds — the cadence of the `Timer::after`
/// in `control_loop_task`. NOT the full tick time: one tick additionally
/// waits `MAX31856_CONVERSION_TIME_MS` for the sensor conversion, so the
/// real cadence is `CONTROL_LOOP_TICK_MS`. This constant is deliberately
/// split from `CONTROL_LOOP_TICK_MS` so charge-window math uses the real
/// cadence (see `CHARGE_SAMPLE_TICK_DIV`).
pub const CONTROL_LOOP_PERIOD_MS: u32 = 100;
/// Real embedded control-loop cadence in milliseconds: the sensor
/// conversion wait (210 ms) plus the 100 ms post-tick timer plus small
/// overhead (command drain, telemetry emit) ≈ 330 ms.
/// Bug audit 2026-08-02: the charge-window derivation previously used
/// `CONTROL_LOOP_PERIOD_MS` (100 ms), so with `CHARGE_SAMPLE_TICK_DIV = 3`
/// the deque actually spanned 10 × 3 × 330 ms ≈ 9.9 s instead of the
/// intended 3 s — a real TC4 charge drop (2–3 °C/s) was diluted over the
/// window and `#CHARGE` could silently never fire.
pub const CONTROL_LOOP_TICK_MS: u32 = CONTROL_LOOP_PERIOD_MS + MAX31856_CONVERSION_TIME_MS as u32;
/// Bug B23 (V2-15): number of control ticks between charge-history samples.
/// Now DERIVED from `CHARGE_DETECTION_WINDOW_S` so the window is a single
/// source of truth — `WINDOW_S × 1000 ms/s = CAP × TICK_DIV × TICK_MS`,
/// hence `TICK_DIV = WINDOW_S × 1000 / (CAP × TICK_MS)`, floored at 1.
/// With (3, 10, 310) the result is `3000 / 3100 → 0 → 1` (a ≈ 3.1 s window
/// spanned by 10 samples taken once per tick ≈ 330 ms apart). Changing the
/// window without re-deriving the divisor no longer silently leaves the
/// deque sampling at the wrong cadence.
pub const CHARGE_SAMPLE_TICK_DIV: u8 = {
    let div = (CHARGE_DETECTION_WINDOW_S * 1000) / (CHARGE_HISTORY_CAPACITY * CONTROL_LOOP_TICK_MS);
    if div == 0 {
        1
    } else {
        div as u8
    }
};

/// Maximum allowed roast duration in seconds (30 minutes).
/// If exceeded during an active roast, emergency shutdown is triggered.
/// This is a safety backstop — Artisan normally controls roast duration via STOP.
pub const MAX_ROAST_TIME_SECS: u32 = 1800;

pub const PREHEAT_HOLD_TOLERANCE_C: f32 = 2.0;

/// Lower bound of the valid control-target range (°C).
/// Bug M7 (2026-08-10): extracted from the hand-written literal in
/// `is_valid_target_temp` so the applied range is a named constant.
pub const MIN_TARGET_TEMP: f32 = 50.0;
/// Upper bound of the valid control-target range (°C) — derived from
/// `MAX_TEMP` so editing the documented limit actually changes what
/// SETTARGET/PREHEAT/PROFILE accept (the old literal stayed 300 even after
/// lowering `MAX_TEMP`, leaving the "safety" edit half-done).
pub const MAX_TARGET_TEMP: f32 = MAX_TEMP;

/// Returns true if the given temperature is a valid control target.
/// Uses `MIN_TARGET_TEMP..=MAX_TARGET_TEMP` (50..=300°C) as the operational
/// range to match parser constraints (PROFILE, SETTARGET, PREHEAT all
/// require 50-300°C).
/// Note: This does NOT clamp to a safe ceiling — the safety layer
/// (OVERTEMP_THRESHOLD) handles emergency cutoff above 260°C. Artisan users
/// may intentionally target above 250°C for dark roasts.
pub fn is_valid_target_temp(temp: f32) -> bool {
    temp.is_finite() && (MIN_TARGET_TEMP..=MAX_TARGET_TEMP).contains(&temp)
}

/// A single setpoint in a roast profile: at time_secs → target temperature °C.
#[derive(Debug, Clone, Copy)]
pub struct ProfileSetpoint {
    pub time_secs: u32,
    pub temperature: f32,
}

/// Fan profile setpoint: at time_secs → fan speed %. Same format as temperature profile.
#[derive(Debug, Clone, Copy)]
pub struct FanSetpoint {
    pub time_secs: u32,
    pub fan_speed: u8,
}

pub struct FanProfile {
    pub setpoints: heapless::Vec<FanSetpoint, MAX_PROFILE_SETPOINTS>,
}

impl Default for FanProfile {
    fn default() -> Self {
        Self::new()
    }
}

impl FanProfile {
    pub fn new() -> Self {
        Self {
            setpoints: heapless::Vec::new(),
        }
    }
    pub fn target_at(&self, elapsed_secs: u32) -> Option<u8> {
        if self.setpoints.is_empty() {
            return None;
        }
        if elapsed_secs <= self.setpoints[0].time_secs {
            return Some(self.setpoints[0].fan_speed);
        }
        for i in 1..self.setpoints.len() {
            let prev = self.setpoints[i - 1];
            let curr = self.setpoints[i];
            if elapsed_secs <= curr.time_secs {
                let range = curr.time_secs - prev.time_secs;
                if range == 0 {
                    return Some(curr.fan_speed);
                }
                let frac = (elapsed_secs - prev.time_secs) as f32 / range as f32;
                let interp =
                    prev.fan_speed as f32 + (curr.fan_speed as f32 - prev.fan_speed as f32) * frac;
                return Some((interp + 0.5).clamp(0.0, 100.0) as u8);
            }
        }
        Some(self.setpoints[self.setpoints.len() - 1].fan_speed)
    }
}

/// Roast profile received from Artisan as a sequence of time-temperature setpoints.
/// Artisan sends: `PROFILE;0,50;120,150;300,200;480,225` meaning
/// at 0s→50°C, 120s→150°C, 300s→200°C, 480s→225°C.
/// The firmware interpolates linearly between setpoints during the roast.
pub struct RoastProfile {
    pub setpoints: heapless::Vec<ProfileSetpoint, MAX_PROFILE_SETPOINTS>,
}

impl Default for RoastProfile {
    fn default() -> Self {
        Self::new()
    }
}

impl RoastProfile {
    pub fn new() -> Self {
        Self {
            setpoints: heapless::Vec::new(),
        }
    }

    /// Compute target temperature at elapsed_secs using linear interpolation.
    /// Returns None if profile is empty.
    pub fn target_at(&self, elapsed_secs: u32) -> Option<f32> {
        if self.setpoints.is_empty() {
            return None;
        }
        // Before first setpoint: return first setpoint temp
        if elapsed_secs <= self.setpoints[0].time_secs {
            return Some(self.setpoints[0].temperature);
        }
        // Find bracketing setpoints
        for i in 1..self.setpoints.len() {
            let prev = self.setpoints[i - 1];
            let curr = self.setpoints[i];
            if elapsed_secs <= curr.time_secs {
                let range = curr.time_secs - prev.time_secs;
                if range == 0 {
                    return Some(curr.temperature);
                }
                let frac = (elapsed_secs - prev.time_secs) as f32 / range as f32;
                return Some(prev.temperature + (curr.temperature - prev.temperature) * frac);
            }
        }
        // After last setpoint: hold at final temperature
        Some(self.setpoints[self.setpoints.len() - 1].temperature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Fan profile interpolation ──────────────

    fn make_fan_profile(pairs: &[(u32, u8)]) -> FanProfile {
        let mut p = FanProfile::new();
        for &(t, s) in pairs {
            p.setpoints
                .push(FanSetpoint {
                    time_secs: t,
                    fan_speed: s,
                })
                .unwrap();
        }
        p
    }

    #[test]
    fn fan_profile_empty_returns_none() {
        let p = FanProfile::new();
        assert_eq!(p.target_at(0), None);
        assert_eq!(p.target_at(100), None);
    }

    #[test]
    fn fan_profile_before_first() {
        let p = make_fan_profile(&[(10, 30), (20, 80)]);
        assert_eq!(p.target_at(0), Some(30));
        assert_eq!(p.target_at(5), Some(30));
    }

    #[test]
    fn fan_profile_exact_setpoint() {
        let p = make_fan_profile(&[(0, 20), (60, 60), (120, 100)]);
        assert_eq!(p.target_at(0), Some(20));
        assert_eq!(p.target_at(60), Some(60));
        assert_eq!(p.target_at(120), Some(100));
    }

    #[test]
    fn fan_profile_interpolation_midpoint() {
        let p = make_fan_profile(&[(0, 0), (100, 100)]);
        assert_eq!(p.target_at(50), Some(50));
    }

    #[test]
    fn fan_profile_after_last_holds() {
        let p = make_fan_profile(&[(0, 20), (30, 80)]);
        assert_eq!(p.target_at(60), Some(80));
        assert_eq!(p.target_at(999), Some(80));
    }

    #[test]
    fn fan_profile_single_setpoint() {
        let p = make_fan_profile(&[(0, 50)]);
        assert_eq!(p.target_at(0), Some(50));
        assert_eq!(p.target_at(100), Some(50));
    }

    #[test]
    fn fan_profile_max_setpoints() {
        let mut p = FanProfile::new();
        for i in 0..MAX_PROFILE_SETPOINTS {
            p.setpoints
                .push(FanSetpoint {
                    time_secs: i as u32 * 10,
                    fan_speed: i as u8 * 6,
                })
                .unwrap();
        }
        assert_eq!(p.setpoints.len(), MAX_PROFILE_SETPOINTS);
        assert_eq!(p.target_at(0), Some(0));
        assert_eq!(
            p.target_at((MAX_PROFILE_SETPOINTS - 1) as u32 * 10),
            Some((MAX_PROFILE_SETPOINTS - 1) as u8 * 6)
        );
    }

    // ── RoastProfile interpolation ──────────────

    fn make_roast_profile(pairs: &[(u32, f32)]) -> RoastProfile {
        let mut p = RoastProfile::new();
        for &(t, temp) in pairs {
            p.setpoints
                .push(ProfileSetpoint {
                    time_secs: t,
                    temperature: temp,
                })
                .unwrap();
        }
        p
    }

    #[test]
    fn roast_profile_empty_returns_none() {
        let p = RoastProfile::new();
        assert_eq!(p.target_at(0), None);
        assert_eq!(p.target_at(100), None);
    }

    #[test]
    fn roast_profile_before_first_returns_first() {
        let p = make_roast_profile(&[(10, 150.0), (120, 200.0)]);
        assert_eq!(p.target_at(0), Some(150.0));
        assert_eq!(p.target_at(5), Some(150.0));
    }

    #[test]
    fn roast_profile_exact_setpoint() {
        let p = make_roast_profile(&[(0, 25.0), (120, 150.0), (300, 200.0)]);
        assert_eq!(p.target_at(0), Some(25.0));
        assert_eq!(p.target_at(120), Some(150.0));
        assert_eq!(p.target_at(300), Some(200.0));
    }

    #[test]
    fn roast_profile_interpolation_midpoint() {
        // (0s, 0°C) → (120s, 120°C): at 60s → 60°C
        let p = make_roast_profile(&[(0, 0.0), (120, 120.0)]);
        assert_eq!(p.target_at(60), Some(60.0));
    }

    #[test]
    fn roast_profile_interpolation_median() {
        // (120s, 150°C) → (300s, 200°C): at 210s → 175°C
        let p = make_roast_profile(&[(120, 150.0), (300, 200.0)]);
        let expected = 150.0 + (200.0 - 150.0) * (210.0 - 120.0) / (300.0 - 120.0);
        assert_eq!(p.target_at(210), Some(expected));
    }

    #[test]
    fn roast_profile_after_last_holds() {
        let p = make_roast_profile(&[(0, 20.0), (120, 200.0)]);
        assert_eq!(p.target_at(200), Some(200.0));
        assert_eq!(p.target_at(999), Some(200.0));
    }

    #[test]
    fn roast_profile_single_setpoint() {
        let p = make_roast_profile(&[(0, 200.0)]);
        assert_eq!(p.target_at(0), Some(200.0));
        assert_eq!(p.target_at(100), Some(200.0));
    }

    #[test]
    fn roast_profile_zero_interval_skips_division() {
        // Two setpoints at same time: interpolation skips division-by-zero
        let p = make_roast_profile(&[(0, 0.0), (0, 100.0)]);
        let result = p.target_at(0);
        assert!(result.is_some());
        // At exact first-setpoint time, returns first setpoint's temp
        assert_eq!(result, Some(0.0));
    }

    // ── is_valid_target_temp ────────────────────

    #[test]
    fn valid_target_temp_returns_true() {
        // Range is 50-300°C to match parser constraints
        assert!(is_valid_target_temp(50.0));
        assert!(is_valid_target_temp(200.0));
        assert!(is_valid_target_temp(300.0));
    }

    #[test]
    fn valid_target_temp_out_of_range_returns_false() {
        assert!(!is_valid_target_temp(49.9));
        assert!(!is_valid_target_temp(300.1));
    }

    #[test]
    fn valid_target_temp_nan_returns_false() {
        assert!(!is_valid_target_temp(f32::NAN));
    }

    #[test]
    fn valid_target_temp_infinite_returns_false() {
        assert!(!is_valid_target_temp(f32::INFINITY));
        assert!(!is_valid_target_temp(f32::NEG_INFINITY));
    }

    // ── TemperatureSettings ─────────────────────

    #[test]
    fn temperature_settings_default_is_celsius() {
        let ts = TemperatureSettings::new();
        assert_eq!(ts.get_scale(), TemperatureScale::Celsius);
        assert!(!ts.is_fahrenheit());
    }

    #[test]
    fn temperature_settings_set_scale() {
        let mut ts = TemperatureSettings::new();
        ts.set_scale(TemperatureScale::Fahrenheit);
        assert_eq!(ts.get_scale(), TemperatureScale::Fahrenheit);
        assert!(ts.is_fahrenheit());
    }

    #[test]
    fn temperature_settings_celsius_conversion() {
        let ts = TemperatureSettings::new();
        assert_eq!(ts.convert_to_display(100.0), 100.0);
        assert_eq!(ts.convert_to_display(0.0), 0.0);
        assert_eq!(ts.convert_to_display(-40.0), -40.0);
    }

    #[test]
    fn temperature_settings_fahrenheit_conversion() {
        let mut ts = TemperatureSettings::new();
        ts.set_scale(TemperatureScale::Fahrenheit);
        assert_eq!(ts.convert_to_display(0.0), 32.0);
        assert_eq!(ts.convert_to_display(100.0), 212.0);
        assert_eq!(ts.convert_to_display(-40.0), -40.0);
    }

    // ── SystemStatus default ────────────────────

    #[test]
    fn system_status_default_values() {
        let s = SystemStatus::default();
        assert_eq!(s.state, RoasterState::Idle);
        assert_eq!(s.bean_temp, 0.0);
        assert_eq!(s.env_temp, 0.0);
        assert_eq!(s.target_temp, DEFAULT_TARGET_TEMP);
        assert!(!s.pid_enabled);
        assert!(!s.artisan_control);
        assert!(!s.fault_condition);
        assert_eq!(s.ssr_hardware_status, SsrHardwareStatus::NotDetected);
        assert!(s.watchdog_feed_ok);
        assert_eq!(s.pid_channel, 2);
        assert_eq!(s.pid_cycle_time_ms, PID_SAMPLE_TIME_MS);
        assert_eq!(s.pid_output_min, 0.0);
        assert_eq!(s.pid_output_max, 100.0);
    }

    // ── Safety thresholds ───────────────────────

    #[test]
    fn safety_thresholds_are_sane() {
        const _: () = {
            assert!(OVERTEMP_THRESHOLD <= MAX_TEMP);
            assert!(MAX_TEMP > DEFAULT_TARGET_TEMP);
            assert!(MAX_TARGET_TEMP == MAX_TEMP);
            assert!(MIN_TARGET_TEMP < MAX_TARGET_TEMP);
            assert!(MAX_BT_RATE_OF_RISE > 0.0);
            assert!(MAX_ROAST_TIME_SECS > 0);
        };
        // Bug M6 (2026-08-10): the feed interval and the HW timeout are now
        // the REAL values (tick cadence vs programmed RWDT hold), so this
        // assertion can actually fail — a tick longer than half the RWDT
        // timeout would reset the chip before the loop re-feeds it.
        const {
            assert!(WATCHDOG_FEED_INTERVAL_MS * 2 < HW_WATCHDOG_TIMEOUT_MS);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RoasterCommand {
    StartRoast(f32),
    StopRoast,
    SetTemperature(f32),
    EmergencyStop,
    Reset,
    SetHeaterManual(u8),
    SetFanManual(u8),
    ArtisanEmergencyStop,
    IncreaseHeater,
    DecreaseHeater,
    SetUnits(bool), // true = Fahrenheit, false = Celsius
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SsrHardwareStatus {
    Available,
    #[default]
    NotDetected,
    Error,
}

/// Temperature scale preference for Artisan protocol
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TemperatureScale {
    #[default]
    Celsius,
    Fahrenheit,
}

/// Temperature settings storage
/// Tracks temperature scale preference without applying conversion
#[derive(Debug, Clone, Copy, Default)]
pub struct TemperatureSettings {
    scale: TemperatureScale,
}

impl TemperatureSettings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_scale(&self) -> TemperatureScale {
        self.scale
    }

    pub fn set_scale(&mut self, scale: TemperatureScale) {
        self.scale = scale;
    }

    /// Check if scale is Fahrenheit
    pub fn is_fahrenheit(&self) -> bool {
        matches!(self.scale, TemperatureScale::Fahrenheit)
    }

    /// Convert temperature from Celsius to display units.
    /// If scale is Fahrenheit: °F = °C × 9.0/5.0 + 32.0
    /// If scale is Celsius: return temp_c unchanged
    pub fn convert_to_display(&self, temp_c: f32) -> f32 {
        match self.scale {
            TemperatureScale::Fahrenheit => temp_c * 9.0 / 5.0 + 32.0,
            TemperatureScale::Celsius => temp_c,
        }
    }

    /// Convert temperature from display units to Celsius (project-internal SI).
    /// If scale is Fahrenheit: °C = (°F − 32) × 5/9
    /// If scale is Celsius: return temp unchanged
    ///
    /// Used when receiving a setpoint over the serial protocol from Artisan:
    /// Artisan reports the setpoint in its own display units, so when it is in
    /// °F mode the firmware must convert the value to °C *before* validating
    /// and storing it as `target_temp`. Storing the raw Fahrenheit value as
    /// Celsius (the previous behaviour: `PID;SV;250` with units = °F was read
    /// as 250 °C and the PID chased a 250 °C target) is a critical-safety bug.
    pub fn convert_from_display(&self, temp: f32) -> f32 {
        match self.scale {
            TemperatureScale::Fahrenheit => (temp - 32.0) * 5.0 / 9.0,
            TemperatureScale::Celsius => temp,
        }
    }
}

/// Current roast state and instrumentation telemetry.
///
/// **Future consideration**: Split into `CoreRoastStatus` (state, temps, outputs —
/// needed every tick) and `InstrumentationSnapshot` (watchdog, PID internals,
/// latency metrics — polled on demand) to reduce stack pressure in the control loop.
#[derive(Debug, Clone, Copy)]
pub struct SystemStatus {
    pub state: RoasterState,
    pub bean_temp: f32,
    pub env_temp: f32,
    pub ambient_temp: f32,
    pub target_temp: f32,
    pub ssr_output: f32,
    pub fan_output: f32,
    pub pid_enabled: bool,
    pub artisan_control: bool,
    pub fault_condition: bool,
    pub ssr_hardware_status: SsrHardwareStatus,
    pub ssr_last_duty_delta_ticks: i16,
    pub ssr_retry_count: u8,
    pub ssr_cycle_guard_busy_until_ms: u64,
    pub watchdog_feed_ok: bool,
    pub watchdog_last_failure: Option<&'static str>,
    pub watchdog_consecutive_failures: u8,
    pub ledc_guard_timeouts: u16,
    pub overtemp_regression_active: bool,
    pub pv: f32,
    pub mv: f32,
    pub integrator_value: f32,
    pub derivative_rate: f32,
    pub saturation_active: bool,
    pub integrator_clamped: bool,
    pub derivative_available: bool,
    pub command_latency_us: u32,
    pub max_command_latency_us: u32,
    pub temperature_settings: TemperatureSettings,
    pub charge_detected: bool,
    pub pid_channel: u8,        // 1=ET, 2=BT, default=2
    pub pid_cycle_time_ms: u32, // default=100 (PID_SAMPLE_TIME_MS)
    pub pid_output_min: f32,    // default=0.0
    pub pid_output_max: f32,    // default=100.0
    /// Millis-since-boot timestamp of the last received Artisan command.
    /// Used by the comms idle timeout safety check. 0 = no command yet.
    pub last_command_received_at_ms: u64,
    /// Bug DRA-7 (2026-07-26): Artisan `CHAN` polling-rate request (Hz),
    /// recorded by `handle_chan`. Informational for now — the telemetry
    /// emitter keeps its own 1 Hz cadence.
    pub chan_poll_rate_hz: u16,
    /// Bug DRA-7 (2026-07-26): Artisan `FILT` filter request, recorded by
    /// `handle_filt`. The firmware applies its own internal EMA alpha; the
    /// host's request is preserved for observability.
    pub requested_filter: u8,
}

impl Default for SystemStatus {
    fn default() -> Self {
        Self {
            state: RoasterState::Idle,
            bean_temp: 0.0,
            env_temp: 0.0,
            ambient_temp: 0.0,
            target_temp: DEFAULT_TARGET_TEMP,
            ssr_output: 0.0,
            fan_output: 0.0,
            pid_enabled: false,
            artisan_control: false,
            fault_condition: false,
            ssr_hardware_status: SsrHardwareStatus::NotDetected,
            ssr_last_duty_delta_ticks: 0,
            ssr_retry_count: 0,
            ssr_cycle_guard_busy_until_ms: 0,
            watchdog_feed_ok: true,
            watchdog_last_failure: None,
            watchdog_consecutive_failures: 0,
            ledc_guard_timeouts: 0,
            overtemp_regression_active: false,
            pv: 0.0,
            mv: 0.0,
            integrator_value: 0.0,
            derivative_rate: 0.0,
            saturation_active: false,
            integrator_clamped: false,
            derivative_available: false,
            command_latency_us: 0,
            max_command_latency_us: 0,
            temperature_settings: TemperatureSettings::new(),
            charge_detected: false,
            pid_channel: 2,
            pid_cycle_time_ms: PID_SAMPLE_TIME_MS,
            pid_output_min: 0.0,
            pid_output_max: 100.0,
            last_command_received_at_ms: 0,
            chan_poll_rate_hz: 0,
            requested_filter: 0,
        }
    }
}
