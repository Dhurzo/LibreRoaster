use crate::config::{ArtisanCommand, FanProfile, ProfileSetpoint, RoastProfile};
use core::cell::RefCell;
use critical_section::Mutex;

static PARSED_PROFILE: Mutex<RefCell<Option<RoastProfile>>> = Mutex::new(RefCell::new(None));

/// Store a parsed profile for the command handler to consume.
pub fn store_profile(profile: RoastProfile) {
    critical_section::with(|cs| {
        *PARSED_PROFILE.borrow(cs).borrow_mut() = Some(profile);
    });
}

/// Consume the stored profile, returning it and clearing the slot.
pub fn take_profile() -> Option<RoastProfile> {
    critical_section::with(|cs| PARSED_PROFILE.borrow(cs).borrow_mut().take())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    UnknownCommand,
    InvalidValue,
    OutOfRange,
    EmptyCommand,
    /// Command exceeded maximum buffer size (256 bytes).
    CommandTooLong,
    /// Event queue overflowed while a partial command was in flight; the
    /// buffered bytes were flushed to prevent silent corruption.
    BufferOverflow,
}

impl ParseError {
    pub fn code(&self) -> &'static str {
        match self {
            ParseError::UnknownCommand => "unknown_command",
            ParseError::InvalidValue => "invalid_value",
            ParseError::OutOfRange => "out_of_range",
            ParseError::EmptyCommand => "invalid_value",
            ParseError::CommandTooLong => "command_too_long",
            ParseError::BufferOverflow => "buffer_overflow",
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            ParseError::UnknownCommand => "unknown_command",
            ParseError::InvalidValue => "invalid_value",
            ParseError::OutOfRange => "out_of_range",
            ParseError::EmptyCommand => "empty_command",
            ParseError::CommandTooLong => "command_too_long",
            ParseError::BufferOverflow => "buffer_overflow",
        }
    }
}

pub fn parse_artisan_command(command: &str) -> Result<ArtisanCommand, ParseError> {
    let trimmed = command.trim();

    if trimmed.is_empty() {
        return Err(ParseError::EmptyCommand);
    }

    // Artisan/TC4 uses ';' as the delimiter for init/handshake commands
    // (CHAN;1200, UNITS;C, FILT;70, PID;SV;250, PROFILE;...) and a space for
    // operational commands (OT1 75, READ, STATUS). Some Artisan configurations
    // also send the operational form with a semicolon: `OT1;75`, `OT2;60`,
    // `IO3;50`. The previous code only handled the ';' delimiter for a fixed
    // whitelist (CHAN/UNITS/FILT/PROFILE/FANPROFILE/PID) and fell through to
    // `split_whitespace()` on the *unmodified* string for anything else — but
    // `split_whitespace` does not treat `;` as a delimiter, so `OT1;75`
    // produced a single token `["OT1;75"]` and was rejected as
    // `unknown_command`, even though the preceding comment claimed a
    // fall-through. Cero tests covered it.
    //
    // Fix: normalise the delimiter to a space BEFORE the init-command
    // dispatch. init commands still match (`"CHAN 1200"` parses identically to
    // `"CHAN;1200"` once we look for `split_once(' ')`), and `OT1;75` becomes
    // `OT1 75`, hitting the existing operational parser.
    let normalized: heapless::String<256> = {
        let mut s = heapless::String::new();
        for ch in trimmed.chars() {
            // Bug B8: the transport layer accepts lines up to 255 bytes
            // (`Vec<u8, 256>`, `CommandTooLong` only fires at ≥256) and
            // PROFILE/FANPROFILE commands routinely reach ~170 bytes with
            // `MAX_PROFILE_SETPOINTS = 16`. The previous `String<128>` plus
            // `let _ = s.push(ch)` silently dropped bytes past 128, splitting
            // a number in two (rejected later as `out_of_range`) or accepting
            // a truncated profile that pinned the roaster at an early
            // setpoint for the entire roast. Use a 256-byte buffer matching
            // the transport ceiling, and surface overflow as an explicit
            // `CommandTooLong` instead of swallowing it.
            let pushed = if ch == ';' { s.push(' ') } else { s.push(ch) };
            if pushed.is_err() {
                return Err(ParseError::CommandTooLong);
            }
        }
        s
    };
    let trimmed = normalized.as_str();
    debug_assert!(!trimmed.is_empty() || command.trim().is_empty());

    // Init commands with the normalised delimiter. We split on the first
    // space; CHAN/UNITS/FILT/etc. consume the remainder as their argument.
    if let Some((cmd, args)) = trimmed.split_once(' ') {
        let init_result: Option<Result<ArtisanCommand, ParseError>> =
            match cmd.to_ascii_uppercase().as_str() {
                "CHAN" => Some(
                    args.trim()
                        .parse::<u16>()
                        .map(ArtisanCommand::Chan)
                        .map_err(|_| ParseError::InvalidValue),
                ),
                "UNITS" => Some(match args.trim() {
                    "C" | "c" => Ok(ArtisanCommand::Units(false)),
                    "F" | "f" => Ok(ArtisanCommand::Units(true)),
                    _ => Err(ParseError::InvalidValue),
                }),
                "FILT" => {
                    // Artisan sends comma-separated filter values
                    // (e.g., "FILT;70,70,70,70") or a single value
                    // (e.g., "FILT;5"). The value is acknowledged but not
                    // used by the firmware — just extract the first token.
                    let val = args
                        .trim()
                        .split(',')
                        .next()
                        .unwrap_or("0")
                        .trim()
                        .parse::<u8>()
                        .unwrap_or(0);
                    Some(Ok(ArtisanCommand::Filt(val)))
                }
                "PROFILE" => Some(parse_profile_args(args.trim())),
                "FANPROFILE" => Some(parse_fan_profile_args(args.trim())),
                "PID" => Some(parse_pid_subcommand(args.trim())),
                // Unknown init command → fall through to operational parsing
                // on the normalised string (e.g. "OT1 75", "READ").
                _ => None,
            };

        if let Some(result) = init_result {
            return result;
        }
    }

    // Operational commands: parse the normalised command by spaces. take(4)
    // prevents heapless::Vec overflow on garbage input (>4 tokens).
    let parts: heapless::Vec<&str, 4> = trimmed.split_whitespace().take(4).collect();

    if parts.is_empty() {
        return Err(ParseError::UnknownCommand);
    }

    let cmd = parts[0];

    if (cmd.eq_ignore_ascii_case("STATUS") || cmd.eq_ignore_ascii_case("STAT")) && parts.len() == 1
    {
        Ok(ArtisanCommand::StatusReport)
    } else if cmd.eq_ignore_ascii_case("READ") && parts.len() == 1 {
        Ok(ArtisanCommand::ReadStatus)
    } else if cmd.eq_ignore_ascii_case("START") && parts.len() == 1 {
        Ok(ArtisanCommand::StartRoast)
    } else if cmd.eq_ignore_ascii_case("STOP") && parts.len() == 1 {
        Ok(ArtisanCommand::EmergencyStop)
    } else if cmd.eq_ignore_ascii_case("UP") && parts.len() == 1 {
        Ok(ArtisanCommand::IncreaseHeater)
    } else if cmd.eq_ignore_ascii_case("DOWN") && parts.len() == 1 {
        Ok(ArtisanCommand::DecreaseHeater)
    } else if cmd.eq_ignore_ascii_case("REG") && parts.len() == 1 {
        Ok(ArtisanCommand::RunRegression)
    } else if cmd.eq_ignore_ascii_case("OT1") {
        if parts.len() == 2 {
            let value = parse_percentage(parts[1])?;
            Ok(ArtisanCommand::SetHeater(value))
        } else {
            Err(ParseError::InvalidValue)
        }
    } else if cmd.eq_ignore_ascii_case("IO3") {
        if parts.len() == 2 {
            let value = parse_percentage(parts[1])?;
            Ok(ArtisanCommand::SetFan(value))
        } else {
            Err(ParseError::InvalidValue)
        }
    } else if cmd.eq_ignore_ascii_case("OT2") {
        if parts.len() == 2 {
            let (value, was_clamped) = parse_ot2_value(parts[1])?;
            Ok(ArtisanCommand::SetFanSpeed(value, was_clamped))
        } else {
            Err(ParseError::InvalidValue)
        }
    } else if cmd.eq_ignore_ascii_case("PIDGAIN") {
        if parts.len() == 4 {
            let kp = parse_float(parts[1])?;
            let ki = parse_float(parts[2])?;
            let kd = parse_float(parts[3])?;
            // Bug B21: PIDGAIN accepts kp/ki/kd via `parse_float`, which
            // happily parses "NaN"/"inf"/"-inf". A NaN gain yields a NaN MV
            // (the PID clamping passes NaN through unchanged, and the SSR
            // driver treats it as "anything"). Reject non-finite inputs here
            // so the operator gets `ERR out_of_range` instead of an
            // undefined heater command.
            if !kp.is_finite() || !ki.is_finite() || !kd.is_finite() {
                return Err(ParseError::OutOfRange);
            }
            Ok(ArtisanCommand::SetPidGain(kp, ki, kd))
        } else {
            Err(ParseError::InvalidValue)
        }
    } else if cmd.eq_ignore_ascii_case("#DUMP") && parts.len() == 1 {
        Ok(ArtisanCommand::DumpLog)
    } else if cmd.eq_ignore_ascii_case("PID,ON") {
        Ok(ArtisanCommand::StartRoast)
    } else if cmd.eq_ignore_ascii_case("PID,OFF") {
        Ok(ArtisanCommand::Stop)
    } else if cmd.to_ascii_uppercase().starts_with("PID,SV,") {
        // PID,SV,150 → same as SETTARGET 150
        let sv_str = &cmd[7..]; // After "PID,SV,"
        let target = sv_str
            .trim()
            .parse::<f32>()
            .map_err(|_| ParseError::InvalidValue)?;
        // Bug B9: drop the (50.0..=300.0) range check from the parser. The
        // value is in *display units* (°C or °F depending on UNITS) here; the
        // handler `handle_set_target_temp` converts to °C and validates the
        // converted value. A °F user with `PID;SV;385` (~196 °C, a normal
        // setpoint) was rejected here because 385 > 300, making PID roasts
        // impossible for anyone running Artisan in Fahrenheit. Keep only the
        // numeric sanity check.
        if !target.is_finite() {
            return Err(ParseError::InvalidValue);
        }
        Ok(ArtisanCommand::SetTargetTemp(target))
    } else if cmd.eq_ignore_ascii_case("PREHEAT") {
        if parts.len() == 2 {
            let temp = parse_float(parts[1])?;
            // Bug B9: same as PID;SV — the handler converts display units to
            // °C and validates the converted value, so the parser must not
            // apply a °C range here.
            if !temp.is_finite() {
                return Err(ParseError::InvalidValue);
            }
            Ok(ArtisanCommand::Preheat(temp))
        } else {
            Err(ParseError::InvalidValue)
        }
    } else if cmd.eq_ignore_ascii_case("SETTARGET") {
        if parts.len() == 2 {
            let target = parse_float(parts[1])?;
            // Bug B9: same as PID;SV — the handler validates after the
            // display→°C conversion; keep only the finite sanity check here.
            if !target.is_finite() {
                return Err(ParseError::InvalidValue);
            }
            Ok(ArtisanCommand::SetTargetTemp(target))
        } else {
            Err(ParseError::InvalidValue)
        }
    } else {
        Err(ParseError::UnknownCommand)
    }
}

fn parse_pid_subcommand(args: &str) -> Result<ArtisanCommand, ParseError> {
    // Accept both ';' and ' ' as segment delimiters: the caller pre-normalises
    // ';' to ' ' for some paths, so we split on either to stay robust under
    // both `PID;SV;250` and `PID SV 250` style inputs.
    let parts: heapless::Vec<&str, 8> = args.split([';', ' ']).take(8).collect();
    if parts.is_empty() {
        return Err(ParseError::InvalidValue);
    }

    match parts[0].trim().to_ascii_uppercase().as_str() {
        "ON" => Ok(ArtisanCommand::StartRoast),
        "OFF" => Ok(ArtisanCommand::Stop),
        "SV" => {
            if parts.len() < 2 {
                return Err(ParseError::InvalidValue);
            }
            let target = parts[1]
                .trim()
                .parse::<f32>()
                .map_err(|_| ParseError::InvalidValue)?;
            // Bug B9: drop the (50.0..=300.0) range check — the value is in
            // display units here; the handler converts to °C and validates.
            if !target.is_finite() {
                return Err(ParseError::InvalidValue);
            }
            Ok(ArtisanCommand::SetTargetTemp(target))
        }
        "T" => {
            // Bug DRH-3 (2026-07-26): `parts.len() < 4` silently accepted
            // extra tokens after kd (`parts[4..]` ignored). Require exactly
            // `PID;T;kp;ki;kd` so a malformed command is rejected loudly
            // instead of partially applied.
            if parts.len() != 4 {
                return Err(ParseError::InvalidValue);
            }
            let kp = parts[1]
                .trim()
                .parse::<f32>()
                .map_err(|_| ParseError::InvalidValue)?;
            let ki = parts[2]
                .trim()
                .parse::<f32>()
                .map_err(|_| ParseError::InvalidValue)?;
            let kd = parts[3]
                .trim()
                .parse::<f32>()
                .map_err(|_| ParseError::InvalidValue)?;
            // Bug B21: `f32::from_str("NaN")/("inf")/("-inf")` parses cleanly
            // into f32 — a NaN/Inf PID gain would propagate into
            // `compute_output` and yield a NaN MV every tick, which the SSR
            // driver then attempts to clamp, leaving heater power undefined.
            // PROTO-1 already fixed the same class on PID;LIMIT; here we also
            // apply the is_finite() check to PID;T and (below) PIDGAIN.
            if !kp.is_finite() || !ki.is_finite() || !kd.is_finite() {
                return Err(ParseError::OutOfRange);
            }
            if kp < 0.0 || ki < 0.0 || kd < 0.0 {
                return Err(ParseError::OutOfRange);
            }
            Ok(ArtisanCommand::SetPidGain(kp, ki, kd))
        }
        "CHAN" => {
            if parts.len() < 2 {
                return Err(ParseError::InvalidValue);
            }
            let ch = parts[1]
                .trim()
                .parse::<u8>()
                .map_err(|_| ParseError::InvalidValue)?;
            // Bug P12 (2026-08-03): accept only `1..=2` — the firmware has
            // exactly two thermocouples (1 = ET, 2 = BT). The previous `1..=4`
            // accepted `PID;CHAN;3|4` and silently executed them as BT (the PV
            // selector treats anything != 1 as BT), leaving the operator with
            // no error and a control input they did not intend.
            if !(1..=2).contains(&ch) {
                return Err(ParseError::OutOfRange);
            }
            Ok(ArtisanCommand::SetPidChannel(ch))
        }
        "CT" => {
            if parts.len() < 2 {
                return Err(ParseError::InvalidValue);
            }
            let ms = parts[1]
                .trim()
                .parse::<u32>()
                .map_err(|_| ParseError::InvalidValue)?;
            if ms < 10 {
                return Err(ParseError::OutOfRange);
            }
            Ok(ArtisanCommand::SetPidCycleTime(ms))
        }
        "LIMIT" => {
            if parts.len() < 3 {
                return Err(ParseError::InvalidValue);
            }
            let min = parts[1]
                .trim()
                .parse::<f32>()
                .map_err(|_| ParseError::InvalidValue)?;
            let max = parts[2]
                .trim()
                .parse::<f32>()
                .map_err(|_| ParseError::InvalidValue)?;
            // PROTO-1: Reject NaN/Inf which would cause PID compute_output to panic
            if !min.is_finite() || !max.is_finite() {
                return Err(ParseError::InvalidValue);
            }
            Ok(ArtisanCommand::SetPidOutputLimits(min, max))
        }
        _ => Err(ParseError::UnknownCommand),
    }
}

fn parse_percentage(value_str: &str) -> Result<u8, ParseError> {
    let value = value_str
        .parse::<u8>()
        .map_err(|_| ParseError::InvalidValue)?;

    if value <= 100 {
        Ok(value)
    } else {
        Err(ParseError::OutOfRange)
    }
}

fn parse_float(value_str: &str) -> Result<f32, ParseError> {
    value_str
        .parse::<f32>()
        .map_err(|_| ParseError::InvalidValue)
}

/// Parse OT2 fan speed value with decimal support.
///
/// OT2 clamps out-of-range fan values to the `[0, 100]` range and reports
/// the clamping back to the caller via `was_clamped=true` so the control
/// layer can emit an `ERR OT2_CLAMPED` notification.
///
/// Bug L10 (2026-07-25): earlier drafts claimed that out-of-range OT2
/// triggers a heater safety cutoff. That diverged from the implementation
/// in `roaster_control.rs::handle_set_fan_speed` (Spec F4.8: OT2 is a
/// fan-override command and must NOT change the heater or PID state). Docs
/// and this doc-comment now describe what the code actually does: clamp
/// the fan, leave the heater alone, notify the host.
///
/// - Decimals are rounded to nearest integer
/// - Values are silently clamped to 0-100 range
/// - Negative values clamp to 0
/// - Returns (clamped_value, was_clamped)
fn parse_ot2_value(value_str: &str) -> Result<(u8, bool), ParseError> {
    let value = value_str
        .parse::<f32>()
        .map_err(|_| ParseError::InvalidValue)?;

    // M6: NaN / Inf parse as a valid f32, but for a safety actuator (cooling
    // fan) they must be rejected outright — clamping `(NaN+0.5) as i32` would
    // saturate to 0 and silently issue `SetFanSpeed(0, true)` with the heater
    // still energised. Sister paths (PIDGAIN/PID;T/PID;LIMIT/SV/SETTARGET/
    // PREHEAT) already do this; bring OT2 in line.
    if !value.is_finite() {
        return Err(ParseError::InvalidValue);
    }

    let was_clamped = !(0.0..=100.0).contains(&value);

    // Round to nearest integer (0.5 rounds up)
    let rounded = if value >= 0.0 {
        (value + 0.5) as i32
    } else {
        (value - 0.5) as i32
    };

    let clamped = rounded.clamp(0, 100) as u8;
    Ok((clamped, was_clamped))
}

fn parse_profile_args(args: &str) -> Result<ArtisanCommand, ParseError> {
    let mut profile = RoastProfile::new();
    // Accept both ';' and ' ' as segment delimiters (the caller may pre-
    // normalise ';' to ' '). Inner point format is `time,temp` (comma).
    for segment in args.split([';', ' ']) {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let mut parts = segment.splitn(2, ',');
        let time_str = parts.next().ok_or(ParseError::InvalidValue)?;
        let temp_str = parts.next().ok_or(ParseError::InvalidValue)?;

        let time_secs: u32 = time_str
            .trim()
            .parse()
            .map_err(|_| ParseError::InvalidValue)?;
        let temperature: f32 = temp_str
            .trim()
            .parse()
            .map_err(|_| ParseError::InvalidValue)?;

        // Bug A4 (2026-07-25): the previous range check (50.0..=300.0) was
        // applied to the RAW numeric value (whichever scale the host sent),
        // but `handle_set_profile` converts to °C with
        // `convert_from_display` first and validates in °C. With UNITS=F the
        // raw °F values for any real roast easily exceed 300 (e.g. 400 °F
        // ≈ 204 °C — a typical drop-BT) and got rejected with
        // `ERR out_of_range` before the converter ever ran. Only reject
        // numerical garbage here (NaN/Inf); range check belongs on the
        // converted value in the handler.
        if !temperature.is_finite() {
            return Err(ParseError::InvalidValue);
        }

        profile
            .setpoints
            .push(ProfileSetpoint {
                time_secs,
                temperature,
            })
            .map_err(|_| ParseError::OutOfRange)?;
    }

    if profile.setpoints.is_empty() {
        return Err(ParseError::EmptyCommand);
    }

    store_profile(profile);
    Ok(ArtisanCommand::SetProfile)
}

fn parse_fan_profile_args(args: &str) -> Result<ArtisanCommand, ParseError> {
    use crate::config::FanSetpoint;
    let mut profile = FanProfile::new();
    for segment in args.split([';', ' ']) {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let mut parts = segment.splitn(2, ',');
        let time_secs: u32 = parts
            .next()
            .ok_or(ParseError::InvalidValue)?
            .trim()
            .parse()
            .map_err(|_| ParseError::InvalidValue)?;
        let fan_speed: u8 = parts
            .next()
            .ok_or(ParseError::InvalidValue)?
            .trim()
            .parse()
            .map_err(|_| ParseError::InvalidValue)?;
        if fan_speed > 100 {
            return Err(ParseError::OutOfRange);
        }
        profile
            .setpoints
            .push(FanSetpoint {
                time_secs,
                fan_speed,
            })
            .map_err(|_| ParseError::OutOfRange)?;
    }
    if profile.setpoints.is_empty() {
        return Err(ParseError::EmptyCommand);
    }
    crate::input::parser::fan_profile_store(profile);
    Ok(ArtisanCommand::SetFanProfile)
}

static PARSED_FAN_PROFILE: Mutex<RefCell<Option<FanProfile>>> = Mutex::new(RefCell::new(None));
pub fn fan_profile_store(profile: FanProfile) {
    critical_section::with(|cs| *PARSED_FAN_PROFILE.borrow(cs).borrow_mut() = Some(profile));
}
pub fn fan_profile_take() -> Option<FanProfile> {
    critical_section::with(|cs| PARSED_FAN_PROFILE.borrow(cs).borrow_mut().take())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(test)]
    mod proptest_tests {
        #![allow(clippy::unwrap_used)]

        use crate::config::ArtisanCommand;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn parse_never_panics(input: String) {
                let _result = super::parse_artisan_command(&input);
            }

            #[test]
            fn empty_and_whitespace_commands(
                whitespace in prop::collection::vec(prop_oneof![
                    Just(' '),
                    Just('\t'),
                    Just('\n'),
                    Just('\r')
                ], 0..20)
            ) {
                let input: String = whitespace.iter().collect();
                let result = super::parse_artisan_command(&input);
                assert!(matches!(result, Err(super::ParseError::EmptyCommand)));
            }
        }

        proptest! {
            #[test]
            fn known_commands_parse_correctly(index in 0u32..100) {
                let command_table = vec![
                    ("READ", ArtisanCommand::ReadStatus),
                    ("STATUS", ArtisanCommand::StatusReport),
                    ("STAT", ArtisanCommand::StatusReport),
                    ("START", ArtisanCommand::StartRoast),
                    ("STOP", ArtisanCommand::EmergencyStop),
                    ("UP", ArtisanCommand::IncreaseHeater),
                    ("DOWN", ArtisanCommand::DecreaseHeater),
                    ("REG", ArtisanCommand::RunRegression),
                    ("#DUMP", ArtisanCommand::DumpLog),
                    ("OT1 0", ArtisanCommand::SetHeater(0)),
                    ("OT1 50", ArtisanCommand::SetHeater(50)),
                    ("OT1 100", ArtisanCommand::SetHeater(100)),
                    ("IO3 0", ArtisanCommand::SetFan(0)),
                    ("IO3 50", ArtisanCommand::SetFan(50)),
                    ("IO3 100", ArtisanCommand::SetFan(100)),
                    ("OT2 0", ArtisanCommand::SetFanSpeed(0, false)),
                    ("OT2 50", ArtisanCommand::SetFanSpeed(50, false)),
                    ("OT2 100", ArtisanCommand::SetFanSpeed(100, false)),
                    ("OT2 150", ArtisanCommand::SetFanSpeed(100, true)),
                    ("SETTARGET 150", ArtisanCommand::SetTargetTemp(150.0)),
                    ("SETTARGET 210.5", ArtisanCommand::SetTargetTemp(210.5)),
                    ("PREHEAT 100", ArtisanCommand::Preheat(100.0)),
                    ("PREHEAT 200.5", ArtisanCommand::Preheat(200.5)),
                    ("PIDGAIN 1.0 0.5 0.1", ArtisanCommand::SetPidGain(1.0, 0.5, 0.1)),
                    ("CHAN;0", ArtisanCommand::Chan(0)),
                    ("CHAN;999", ArtisanCommand::Chan(999)),
                    ("UNITS;C", ArtisanCommand::Units(false)),
                    ("UNITS;F", ArtisanCommand::Units(true)),
                    ("FILT;5", ArtisanCommand::Filt(5)),
                    ("FILT;70,70,70,70", ArtisanCommand::Filt(70)),
                    ("PID;ON", ArtisanCommand::StartRoast),
                    ("PID;OFF", ArtisanCommand::Stop),
                    ("read", ArtisanCommand::ReadStatus),
                    ("status", ArtisanCommand::StatusReport),
                    ("ot1 75", ArtisanCommand::SetHeater(75)),
                    ("Up", ArtisanCommand::IncreaseHeater),
                ];

                let (input, _expected_command) = command_table[index as usize % command_table.len()];
                let result = super::parse_artisan_command(input);
                assert!(matches!(result, Ok(_expected_command)));
            }
        }
    }

    #[test]
    fn test_parse_read_command() {
        let result = parse_artisan_command("READ");
        assert!(matches!(result, Ok(ArtisanCommand::ReadStatus)));
    }

    #[test]
    fn test_parse_status_command() {
        let result = parse_artisan_command("STATUS");
        assert!(matches!(result, Ok(ArtisanCommand::StatusReport)));
    }

    #[test]
    fn test_parse_stat_command_alias() {
        let result = parse_artisan_command("STAT");
        assert!(matches!(result, Ok(ArtisanCommand::StatusReport)));
    }

    #[test]
    fn test_parse_start_command() {
        let result = parse_artisan_command("START");
        assert!(matches!(result, Ok(ArtisanCommand::StartRoast)));
    }

    #[test]
    fn test_parse_ot1_command() {
        let result = parse_artisan_command("OT1 75");
        assert!(matches!(result, Ok(ArtisanCommand::SetHeater(75))));
    }

    #[test]
    fn test_parse_io3_command() {
        let result = parse_artisan_command("IO3 50");
        assert!(matches!(result, Ok(ArtisanCommand::SetFan(50))));
    }

    #[test]
    fn test_parse_stop_command() {
        let result = parse_artisan_command("STOP");
        assert!(matches!(result, Ok(ArtisanCommand::EmergencyStop)));
    }

    #[test]
    fn test_parse_regression_command() {
        let result = parse_artisan_command("REG");
        assert!(matches!(result, Ok(ArtisanCommand::RunRegression)));
    }

    #[test]
    fn test_invalid_command() {
        let result = parse_artisan_command("INVALID");
        assert!(matches!(result, Err(ParseError::UnknownCommand)));
    }

    #[test]
    fn test_invalid_value() {
        let result = parse_artisan_command("OT1 abc");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }

    #[test]
    fn test_out_of_range_value() {
        let result = parse_artisan_command("OT1 150");
        assert!(matches!(result, Err(ParseError::OutOfRange)));
    }

    #[test]
    fn test_empty_command() {
        let result = parse_artisan_command("");
        assert!(matches!(result, Err(ParseError::EmptyCommand)));
    }

    #[test]
    fn test_parse_ot1_zero() {
        let result = parse_artisan_command("OT1 0");
        assert!(matches!(result, Ok(ArtisanCommand::SetHeater(0))));
    }

    #[test]
    fn test_parse_ot1_max() {
        let result = parse_artisan_command("OT1 100");
        assert!(matches!(result, Ok(ArtisanCommand::SetHeater(100))));
    }

    #[test]
    fn test_parse_io3_zero() {
        let result = parse_artisan_command("IO3 0");
        assert!(matches!(result, Ok(ArtisanCommand::SetFan(0))));
    }

    #[test]
    fn test_parse_io3_max() {
        let result = parse_artisan_command("IO3 100");
        assert!(matches!(result, Ok(ArtisanCommand::SetFan(100))));
    }

    #[test]
    fn test_parse_io3_invalid_above() {
        let result = parse_artisan_command("IO3 150");
        assert!(matches!(result, Err(ParseError::OutOfRange)));
    }

    // Initialization handshake command tests (Phase 17)

    #[test]
    fn test_parse_chan_command() {
        let result = parse_artisan_command("CHAN;1200");
        assert!(matches!(result, Ok(ArtisanCommand::Chan(1200))));
    }

    #[test]
    fn test_parse_chan_command_lowercase() {
        let result = parse_artisan_command("chan;1200");
        assert!(matches!(result, Ok(ArtisanCommand::Chan(1200))));
    }

    #[test]
    fn test_parse_chan_command_mixed_case() {
        let result = parse_artisan_command("Chan;9999");
        assert!(matches!(result, Ok(ArtisanCommand::Chan(9999))));
    }

    #[test]
    fn test_parse_chan_command_invalid_value() {
        let result = parse_artisan_command("CHAN;abc");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }

    #[test]
    fn test_parse_units_command_celsius() {
        let result = parse_artisan_command("UNITS;C");
        assert!(matches!(result, Ok(ArtisanCommand::Units(false))));
    }

    #[test]
    fn test_parse_units_command_fahrenheit() {
        let result = parse_artisan_command("UNITS;F");
        assert!(matches!(result, Ok(ArtisanCommand::Units(true))));
    }

    #[test]
    fn test_parse_units_command_lowercase() {
        let result = parse_artisan_command("units;f");
        assert!(matches!(result, Ok(ArtisanCommand::Units(true))));
    }

    #[test]
    fn test_parse_units_command_invalid() {
        let result = parse_artisan_command("UNITS;K");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }

    #[test]
    fn test_parse_filt_command() {
        let result = parse_artisan_command("FILT;5");
        assert!(matches!(result, Ok(ArtisanCommand::Filt(5))));
    }

    #[test]
    fn test_parse_filt_command_lowercase() {
        let result = parse_artisan_command("filt;3");
        assert!(matches!(result, Ok(ArtisanCommand::Filt(3))));
    }

    #[test]
    fn test_parse_filt_command_non_numeric_falls_back_to_zero() {
        // Non-numeric FILT values gracefully fall back to 0
        let result = parse_artisan_command("FILT;abc");
        assert!(matches!(result, Ok(ArtisanCommand::Filt(0))));
    }

    #[test]
    fn test_parse_filt_command_multi_value() {
        // Artisan sends comma-separated filter values: "FILT;70,70,70,70"
        let result = parse_artisan_command("FILT;70,70,70,70");
        assert!(matches!(result, Ok(ArtisanCommand::Filt(70))));
    }

    #[test]
    fn test_parse_filt_command_multi_value_extracts_first() {
        // Only the first comma-separated value matters
        let result = parse_artisan_command("FILT;80,90,100,110");
        assert!(matches!(result, Ok(ArtisanCommand::Filt(80))));
    }

    #[test]
    fn test_parse_filt_command_with_whitespace() {
        let result = parse_artisan_command("FILT; 7 ");
        assert!(matches!(result, Ok(ArtisanCommand::Filt(7))));
    }

    #[test]
    fn test_parse_chan_unknown_command() {
        let result = parse_artisan_command("UNKNOWN;123");
        assert!(matches!(result, Err(ParseError::UnknownCommand)));
    }

    #[test]
    fn test_semicolon_with_space_fallback() {
        let result = parse_artisan_command("CHAN;1200");
        assert!(matches!(result, Ok(ArtisanCommand::Chan(1200))));

        let result = parse_artisan_command("OT1 75");
        assert!(matches!(result, Ok(ArtisanCommand::SetHeater(75))));
    }

    #[test]
    fn test_parse_up_command() {
        let result = parse_artisan_command("UP");
        assert!(matches!(result, Ok(ArtisanCommand::IncreaseHeater)));
    }

    #[test]
    fn test_parse_up_command_case_insensitive() {
        let result = parse_artisan_command("up");
        assert!(matches!(result, Ok(ArtisanCommand::IncreaseHeater)));
    }

    #[test]
    fn test_parse_down_command() {
        let result = parse_artisan_command("DOWN");
        assert!(matches!(result, Ok(ArtisanCommand::DecreaseHeater)));
    }

    #[test]
    fn test_parse_down_command_case_insensitive() {
        let result = parse_artisan_command("down");
        assert!(matches!(result, Ok(ArtisanCommand::DecreaseHeater)));
    }

    #[test]
    fn test_empty_command_returns_empty_command_error() {
        let result = parse_artisan_command("");
        assert!(matches!(result, Err(ParseError::EmptyCommand)));
    }

    /// TEST-18-05b: Verify whitespace-only command returns EmptyCommand error
    #[test]
    fn test_whitespace_command_returns_empty_command_error() {
        let result = parse_artisan_command("   ");
        assert!(matches!(result, Err(ParseError::EmptyCommand)));
    }

    #[test]
    fn test_partial_ot1_command_returns_invalid_value() {
        let result = parse_artisan_command("OT1");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }

    #[test]
    fn test_partial_io3_command_returns_invalid_value() {
        let result = parse_artisan_command("IO3");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }

    #[test]
    fn test_extra_whitespace_handled_correctly() {
        let result = parse_artisan_command("OT1  50");
        assert!(matches!(result, Ok(ArtisanCommand::SetHeater(50))));
    }

    #[test]
    fn test_parse_ot1_zero_value() {
        let result = parse_artisan_command("OT1 0");
        assert!(matches!(result, Ok(ArtisanCommand::SetHeater(0))));
    }

    #[test]
    fn test_parse_ot1_max_value() {
        let result = parse_artisan_command("OT1 100");
        assert!(matches!(result, Ok(ArtisanCommand::SetHeater(100))));
    }

    #[test]
    fn test_parse_ot1_out_of_range() {
        let result = parse_artisan_command("OT1 150");
        assert!(matches!(result, Err(ParseError::OutOfRange)));
    }

    #[test]
    fn test_parse_io3_zero_value() {
        let result = parse_artisan_command("IO3 0");
        assert!(matches!(result, Ok(ArtisanCommand::SetFan(0))));
    }

    #[test]
    fn test_parse_io3_max_value() {
        let result = parse_artisan_command("IO3 100");
        assert!(matches!(result, Ok(ArtisanCommand::SetFan(100))));
    }

    #[test]
    fn test_parse_io3_out_of_range() {
        let result = parse_artisan_command("IO3 150");
        assert!(matches!(result, Err(ParseError::OutOfRange)));
    }

    // OT2 Command Tests

    #[test]
    fn test_parse_ot2_command_basic() {
        let result = parse_artisan_command("OT2 75");
        assert!(matches!(result, Ok(ArtisanCommand::SetFanSpeed(75, false))));
    }

    #[test]
    fn test_parse_ot2_command_lowercase() {
        let result = parse_artisan_command("ot2 50");
        assert!(matches!(result, Ok(ArtisanCommand::SetFanSpeed(50, false))));
    }

    #[test]
    fn test_parse_ot2_decimal_rounds_up() {
        let result = parse_artisan_command("OT2 50.5");
        assert!(matches!(result, Ok(ArtisanCommand::SetFanSpeed(51, false))));
    }

    #[test]
    fn test_parse_ot2_decimal_rounds_down() {
        let result = parse_artisan_command("OT2 50.4");
        assert!(matches!(result, Ok(ArtisanCommand::SetFanSpeed(50, false))));
    }

    #[test]
    fn test_parse_ot2_clamped_above_max() {
        let result = parse_artisan_command("OT2 150");
        assert!(matches!(result, Ok(ArtisanCommand::SetFanSpeed(100, true))));
    }

    #[test]
    fn test_parse_ot2_clamped_negative() {
        let result = parse_artisan_command("OT2 -5");
        assert!(matches!(result, Ok(ArtisanCommand::SetFanSpeed(0, true))));
    }

    #[test]
    fn test_parse_ot2_zero() {
        let result = parse_artisan_command("OT2 0");
        assert!(matches!(result, Ok(ArtisanCommand::SetFanSpeed(0, false))));
    }

    #[test]
    fn test_parse_ot2_max() {
        let result = parse_artisan_command("OT2 100");
        assert!(matches!(
            result,
            Ok(ArtisanCommand::SetFanSpeed(100, false))
        ));
    }

    #[test]
    fn test_parse_ot2_invalid_value() {
        let result = parse_artisan_command("OT2 abc");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }

    #[test]
    fn test_parse_ot2_partial_command() {
        let result = parse_artisan_command("OT2");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }

    #[test]
    fn test_parse_pidgain_command() {
        let result = parse_artisan_command("PIDGAIN 2.0 0.25 0.05");
        assert!(matches!(
            result,
            Ok(ArtisanCommand::SetPidGain(2.0, 0.25, 0.05))
        ));
    }

    #[test]
    fn test_parse_pidgain_case_insensitive() {
        let result = parse_artisan_command("pidgain 1.5 0.3 0.1");
        assert!(matches!(
            result,
            Ok(ArtisanCommand::SetPidGain(1.5, 0.3, 0.1))
        ));
    }

    #[test]
    fn test_parse_pidgain_invalid_value() {
        let result = parse_artisan_command("PIDGAIN abc 0.25 0.05");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }

    #[test]
    fn test_parse_pidgain_partial() {
        let result = parse_artisan_command("PIDGAIN 2.0 0.25");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }

    #[test]
    fn test_parse_settarget_command() {
        let result = parse_artisan_command("SETTARGET 200");
        assert!(matches!(result, Ok(ArtisanCommand::SetTargetTemp(200.0))));
    }

    #[test]
    fn test_parse_settarget_decimal() {
        let result = parse_artisan_command("SETTARGET 210.5");
        assert!(matches!(result, Ok(ArtisanCommand::SetTargetTemp(210.5))));
    }

    #[test]
    fn test_parse_settarget_out_of_range() {
        // Bug B9: the parser must NOT range-check display-unit setpoints —
        // the handler validates after the °F→°C conversion. 350 °F is well
        // within °C target range (~177 °C), and the parser must pass it.
        let result = parse_artisan_command("SETTARGET 350");
        assert!(matches!(result, Ok(ArtisanCommand::SetTargetTemp(v))
            if (v - 350.0).abs() < f32::EPSILON));
    }

    #[test]
    fn test_parse_settarget_too_low() {
        // Bug B9: see `test_parse_settarget_out_of_range`. A small value
        // like 40 °F (~4 °C) is parsed successfully; the handler decides
        // whether the converted target is in the operational °C window.
        let result = parse_artisan_command("SETTARGET 40");
        assert!(matches!(result, Ok(ArtisanCommand::SetTargetTemp(v))
            if (v - 40.0).abs() < f32::EPSILON));
    }

    // ── PREHEAT command edge cases ────────────

    #[test]
    fn test_preheat_basic() {
        assert!(matches!(
            parse_artisan_command("PREHEAT 180"),
            Ok(ArtisanCommand::Preheat(180.0))
        ));
    }

    #[test]
    fn test_preheat_decimal() {
        assert!(matches!(
            parse_artisan_command("PREHEAT 210.5"),
            Ok(ArtisanCommand::Preheat(210.5))
        ));
    }

    #[test]
    fn test_preheat_min() {
        assert!(matches!(
            parse_artisan_command("PREHEAT 50"),
            Ok(ArtisanCommand::Preheat(50.0))
        ));
    }

    #[test]
    fn test_preheat_max() {
        assert!(matches!(
            parse_artisan_command("PREHEAT 300"),
            Ok(ArtisanCommand::Preheat(300.0))
        ));
    }

    #[test]
    fn test_preheat_too_low() {
        // Bug B9: parser passes the value through; the handler validates
        // after the display→°C conversion.
        assert!(matches!(
            parse_artisan_command("PREHEAT 40"),
            Ok(ArtisanCommand::Preheat(40.0))
        ));
    }

    #[test]
    fn test_preheat_too_high() {
        // Bug B9: parser passes the value through (e.g. 350 °F ≈ 177 °C
        // is a perfectly normal preheat). Handler validates post-conversion.
        assert!(matches!(
            parse_artisan_command("PREHEAT 350"),
            Ok(ArtisanCommand::Preheat(350.0))
        ));
    }

    #[test]
    fn test_preheat_no_value() {
        assert!(matches!(
            parse_artisan_command("PREHEAT"),
            Err(ParseError::InvalidValue)
        ));
    }

    #[test]
    fn test_preheat_invalid() {
        assert!(matches!(
            parse_artisan_command("PREHEAT abc"),
            Err(ParseError::InvalidValue)
        ));
    }

    // ── FANPROFILE command edge cases ──────────

    #[test]
    fn test_fanprofile_basic() {
        assert!(matches!(
            parse_artisan_command("FANPROFILE;0,20;60,50;120,100"),
            Ok(ArtisanCommand::SetFanProfile)
        ));
    }

    #[test]
    fn test_fanprofile_single_setpoint() {
        assert!(matches!(
            parse_artisan_command("FANPROFILE;0,30"),
            Ok(ArtisanCommand::SetFanProfile)
        ));
    }

    #[test]
    fn test_fanprofile_empty() {
        assert!(matches!(
            parse_artisan_command("FANPROFILE;"),
            Err(ParseError::EmptyCommand)
        ));
    }

    #[test]
    fn test_fanprofile_out_of_range() {
        assert!(matches!(
            parse_artisan_command("FANPROFILE;0,150"),
            Err(ParseError::OutOfRange)
        ));
    }

    #[test]
    fn test_fanprofile_invalid_format() {
        assert!(matches!(
            parse_artisan_command("FANPROFILE;abc,def"),
            Err(ParseError::InvalidValue)
        ));
    }

    // ── TC4 PID commands ──────────────────────

    #[test]
    fn test_pid_on_maps_to_start() {
        assert!(matches!(
            parse_artisan_command("PID,ON"),
            Ok(ArtisanCommand::StartRoast)
        ));
    }

    #[test]
    fn test_pid_off_maps_to_stop() {
        assert!(matches!(
            parse_artisan_command("PID,OFF"),
            Ok(ArtisanCommand::Stop)
        ));
    }

    #[test]
    fn test_pid_sv_maps_to_settarget() {
        assert!(matches!(
            parse_artisan_command("PID,SV,150"),
            Ok(ArtisanCommand::SetTargetTemp(150.0))
        ));
        assert!(matches!(
            parse_artisan_command("PID,SV,210.5"),
            Ok(ArtisanCommand::SetTargetTemp(210.5))
        ));
    }

    #[test]
    fn test_pid_sv_case_insensitive() {
        assert!(matches!(
            parse_artisan_command("pid,sv,200"),
            Ok(ArtisanCommand::SetTargetTemp(200.0))
        ));
    }

    #[test]
    fn test_pid_sv_out_of_range() {
        // Bug B9: parser no longer range-checks display-unit setpoints.
        // 40 °F (~4 °C) and 350 °F (~177 °C) are both accepted; the handler
        // validates after the °F→°C conversion.
        assert!(matches!(
            parse_artisan_command("PID,SV,40"),
            Ok(ArtisanCommand::SetTargetTemp(40.0))
        ));
        assert!(matches!(
            parse_artisan_command("PID,SV,350"),
            Ok(ArtisanCommand::SetTargetTemp(350.0))
        ));
    }

    // ── PID semicolon command tests ──────────

    #[test]
    fn test_pid_semicolon_on() {
        assert!(matches!(
            parse_artisan_command("PID;ON"),
            Ok(ArtisanCommand::StartRoast)
        ));
    }

    #[test]
    fn test_pid_semicolon_off() {
        assert!(matches!(
            parse_artisan_command("PID;OFF"),
            Ok(ArtisanCommand::Stop)
        ));
    }

    #[test]
    fn test_pid_semicolon_sv() {
        assert!(matches!(
            parse_artisan_command("PID;SV;150"),
            Ok(ArtisanCommand::SetTargetTemp(v)) if (v - 150.0).abs() < f32::EPSILON
        ));
    }

    #[test]
    fn test_pid_semicolon_sv_decimal() {
        assert!(matches!(
            parse_artisan_command("PID;SV;210.5"),
            Ok(ArtisanCommand::SetTargetTemp(v)) if (v - 210.5).abs() < f32::EPSILON
        ));
    }

    #[test]
    fn test_pid_semicolon_sv_out_of_range() {
        // Bug B9: parser no longer range-checks display-unit setpoints.
        // 40 °F is parsed successfully; the handler validates post-conversion.
        assert!(matches!(
            parse_artisan_command("PID;SV;40"),
            Ok(ArtisanCommand::SetTargetTemp(40.0))
        ));
    }

    #[test]
    fn test_pid_semicolon_t() {
        let result = parse_artisan_command("PID;T;2.0;0.5;1.0");
        assert!(matches!(result, Ok(ArtisanCommand::SetPidGain(kp, ki, kd))
            if (kp - 2.0).abs() < f32::EPSILON && (ki - 0.5).abs() < f32::EPSILON && (kd - 1.0).abs() < f32::EPSILON
        ));
    }

    #[test]
    fn test_pid_semicolon_t_invalid() {
        assert!(matches!(
            parse_artisan_command("PID;T;abc"),
            Err(ParseError::InvalidValue)
        ));
    }

    #[test]
    fn test_pid_semicolon_t_negative() {
        assert!(matches!(
            parse_artisan_command("PID;T;-1;0.5;1.0"),
            Err(ParseError::OutOfRange)
        ));
    }

    #[test]
    fn test_pid_semicolon_chan() {
        assert!(matches!(
            parse_artisan_command("PID;CHAN;2"),
            Ok(ArtisanCommand::SetPidChannel(2))
        ));
    }

    #[test]
    fn test_pid_semicolon_chan_et() {
        assert!(matches!(
            parse_artisan_command("PID;CHAN;1"),
            Ok(ArtisanCommand::SetPidChannel(1))
        ));
    }

    #[test]
    fn test_pid_semicolon_chan_invalid() {
        assert!(matches!(
            parse_artisan_command("PID;CHAN;5"),
            Err(ParseError::OutOfRange)
        ));
    }

    #[test]
    fn test_pid_semicolon_chan_3_and_4_rejected() {
        // Bug P12: the firmware has exactly two thermocouples (1 = ET, 2 =
        // BT). The previous `1..=4` accepted 3|4 and silently executed them
        // as BT (the PV selector treats anything != 1 as BT) — reject loudly.
        assert!(matches!(
            parse_artisan_command("PID;CHAN;3"),
            Err(ParseError::OutOfRange)
        ));
        assert!(matches!(
            parse_artisan_command("PID;CHAN;4"),
            Err(ParseError::OutOfRange)
        ));
        assert!(matches!(
            parse_artisan_command("PID;CHAN;0"),
            Err(ParseError::OutOfRange)
        ));
    }

    #[test]
    fn test_pid_semicolon_ct() {
        assert!(matches!(
            parse_artisan_command("PID;CT;1000"),
            Ok(ArtisanCommand::SetPidCycleTime(1000))
        ));
    }

    #[test]
    fn test_pid_semicolon_ct_too_low() {
        assert!(matches!(
            parse_artisan_command("PID;CT;5"),
            Err(ParseError::OutOfRange)
        ));
    }

    #[test]
    fn test_pid_semicolon_limit() {
        assert!(matches!(
            parse_artisan_command("PID;LIMIT;0;100"),
            Ok(ArtisanCommand::SetPidOutputLimits(min, max)) if min == 0.0 && max == 100.0
        ));
    }

    #[test]
    fn test_pid_semicolon_limit_custom() {
        assert!(matches!(
            parse_artisan_command("PID;LIMIT;20;80"),
            Ok(ArtisanCommand::SetPidOutputLimits(min, max)) if min == 20.0 && max == 80.0
        ));
    }

    #[test]
    fn test_pid_semicolon_unknown_sub() {
        assert!(matches!(
            parse_artisan_command("PID;UNKNOWN"),
            Err(ParseError::UnknownCommand)
        ));
    }

    #[test]
    fn test_pid_comma_still_works_on() {
        assert!(matches!(
            parse_artisan_command("PID,ON"),
            Ok(ArtisanCommand::StartRoast)
        ));
    }

    #[test]
    fn test_pid_comma_still_works_off() {
        assert!(matches!(
            parse_artisan_command("PID,OFF"),
            Ok(ArtisanCommand::Stop)
        ));
    }

    /// Bug #4 regression: Artisan's default slider syntax uses a semicolon,
    /// not a space, for the operational commands `OT1`, `OT2`, `IO3`. The
    /// previous parser rejected them with `ERR unknown_command` because it
    /// only let `;` through for the init-command whitelist (CHAN/UNITS/FILT/
    /// PROFILE/FANPROFILE/PID) and then `split_whitespace` left "OT1;75" as
    /// a single token. These tests pin the fix that normalises `;`→` ` first.
    #[test]
    fn test_ot1_semicolon_parses_as_set_heater() {
        assert_eq!(
            parse_artisan_command("OT1;75"),
            Ok(ArtisanCommand::SetHeater(75))
        );
    }

    #[test]
    fn test_ot2_semicolon_parses_as_set_fan_speed() {
        assert_eq!(
            parse_artisan_command("OT2;60"),
            Ok(ArtisanCommand::SetFanSpeed(60, false))
        );
    }

    #[test]
    fn test_io3_semicolon_parses_as_set_fan() {
        assert_eq!(
            parse_artisan_command("IO3;50"),
            Ok(ArtisanCommand::SetFan(50))
        );
    }

    /// Bug #4 regression: PID sub-commands must still parse after the `;`
    /// normalisation — `PID;SV;250` becomes `PID SV 250`, so the pid
    /// sub-parser must split on either delimiter.
    #[test]
    fn test_pid_sv_semicolon_parses_as_set_target() {
        assert_eq!(
            parse_artisan_command("PID;SV;250"),
            Ok(ArtisanCommand::SetTargetTemp(250.0))
        );
    }

    #[test]
    fn test_chan_semicolon_still_works() {
        assert_eq!(
            parse_artisan_command("CHAN;1200"),
            Ok(ArtisanCommand::Chan(1200))
        );
    }

    #[test]
    fn test_units_semicolon_still_works() {
        assert_eq!(
            parse_artisan_command("UNITS;F"),
            Ok(ArtisanCommand::Units(true))
        );
    }
}
