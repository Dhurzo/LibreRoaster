use crate::config::ArtisanCommand;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    UnknownCommand,
    InvalidValue,
    OutOfRange,
    EmptyCommand,
}

impl ParseError {
    pub fn code(&self) -> &'static str {
        match self {
            ParseError::UnknownCommand => "unknown_command",
            ParseError::InvalidValue => "invalid_value",
            ParseError::OutOfRange => "out_of_range",
            ParseError::EmptyCommand => "invalid_value",
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            ParseError::UnknownCommand => "unknown_command",
            ParseError::InvalidValue => "invalid_value",
            ParseError::OutOfRange => "out_of_range",
            ParseError::EmptyCommand => "empty_command",
        }
    }
}

pub fn parse_artisan_command(command: &str) -> Result<ArtisanCommand, ParseError> {
    let trimmed = command.trim();

    if trimmed.is_empty() {
        return Err(ParseError::EmptyCommand);
    }

    // Try semicolon delimiter first (Artisan standard for init commands)
    if let Some((cmd, args)) = trimmed.split_once(';') {
        return match cmd.to_ascii_uppercase().as_str() {
            "CHAN" => args
                .trim()
                .parse::<u16>()
                .map(ArtisanCommand::Chan)
                .map_err(|_| ParseError::InvalidValue),
            "UNITS" => match args.trim() {
                "C" | "c" => Ok(ArtisanCommand::Units(false)),
                "F" | "f" => Ok(ArtisanCommand::Units(true)),
                _ => Err(ParseError::InvalidValue),
            },
            "FILT" => args
                .trim()
                .parse::<u8>()
                .map(ArtisanCommand::Filt)
                .map_err(|_| ParseError::InvalidValue),
            _ => Err(ParseError::UnknownCommand),
        };
    }

    // Fall back to space delimiter for operational commands
    let parts: heapless::Vec<&str, 4> = trimmed.split_whitespace().collect();

    match parts.as_slice() {
        ["READ"] => Ok(ArtisanCommand::ReadStatus),

        ["START"] => Ok(ArtisanCommand::StartRoast),

        ["OT1", value_str] => {
            let value = parse_percentage(value_str)?;
            Ok(ArtisanCommand::SetHeater(value))
        }

        ["IO3", value_str] => {
            let value = parse_percentage(value_str)?;
            Ok(ArtisanCommand::SetFan(value))
        }

        ["STOP"] => Ok(ArtisanCommand::EmergencyStop),

        ["UP" | "up"] => Ok(ArtisanCommand::IncreaseHeater),

        ["DOWN" | "down"] => Ok(ArtisanCommand::DecreaseHeater),

        // Partial commands (commands that require a value but don't have one)
        ["OT1"] | ["IO3"] => Err(ParseError::InvalidValue),

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_read_command() {
        let result = parse_artisan_command("READ");
        assert!(matches!(result, Ok(ArtisanCommand::ReadStatus)));
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
    fn test_parse_filt_command_invalid_value() {
        let result = parse_artisan_command("FILT;abc");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
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
        // Semicolon commands should not fallback to space delimiter
        let result = parse_artisan_command("CHAN;1200");
        assert!(matches!(result, Ok(ArtisanCommand::Chan(1200))));

        // Space-delimited should still work for existing commands
        let result = parse_artisan_command("OT1 75");
        assert!(matches!(result, Ok(ArtisanCommand::SetHeater(75))));
    }

    // Phase 18: Command & Response Protocol Tests

    /// TEST-18-02a: Verify UP command parses correctly
    #[test]
    fn test_parse_up_command() {
        let result = parse_artisan_command("UP");
        assert!(matches!(result, Ok(ArtisanCommand::IncreaseHeater)));
    }

    /// TEST-18-02b: Verify UP command is case-insensitive
    #[test]
    fn test_parse_up_command_case_insensitive() {
        let result = parse_artisan_command("up");
        assert!(matches!(result, Ok(ArtisanCommand::IncreaseHeater)));
    }

    /// TEST-18-02c: Verify DOWN command parses correctly
    #[test]
    fn test_parse_down_command() {
        let result = parse_artisan_command("DOWN");
        assert!(matches!(result, Ok(ArtisanCommand::DecreaseHeater)));
    }

    /// TEST-18-02d: Verify DOWN command is case-insensitive
    #[test]
    fn test_parse_down_command_case_insensitive() {
        let result = parse_artisan_command("down");
        assert!(matches!(result, Ok(ArtisanCommand::DecreaseHeater)));
    }

    /// TEST-18-05a: Verify empty command returns EmptyCommand error
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

    /// TEST-18-05c: Verify partial OT1 command (no value) returns InvalidValue error
    #[test]
    fn test_partial_ot1_command_returns_invalid_value() {
        let result = parse_artisan_command("OT1");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }

    /// TEST-18-05d: Verify partial IO3 command (no value) returns InvalidValue error
    #[test]
    fn test_partial_io3_command_returns_invalid_value() {
        let result = parse_artisan_command("IO3");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }

    /// TEST-18-05e: Verify extra whitespace is handled correctly
    #[test]
    fn test_extra_whitespace_handled_correctly() {
        let result = parse_artisan_command("OT1  50");
        assert!(matches!(result, Ok(ArtisanCommand::SetHeater(50))));
    }

    /// TEST-18-05f: Verify OT1 with value zero parses correctly
    #[test]
    fn test_parse_ot1_zero_value() {
        let result = parse_artisan_command("OT1 0");
        assert!(matches!(result, Ok(ArtisanCommand::SetHeater(0))));
    }

    /// TEST-18-05g: Verify OT1 with value 100 parses correctly
    #[test]
    fn test_parse_ot1_max_value() {
        let result = parse_artisan_command("OT1 100");
        assert!(matches!(result, Ok(ArtisanCommand::SetHeater(100))));
    }

    /// TEST-18-05h: Verify OT1 with value > 100 returns OutOfRange error
    #[test]
    fn test_parse_ot1_out_of_range() {
        let result = parse_artisan_command("OT1 150");
        assert!(matches!(result, Err(ParseError::OutOfRange)));
    }

    /// TEST-18-05i: Verify IO3 with value zero parses correctly
    #[test]
    fn test_parse_io3_zero_value() {
        let result = parse_artisan_command("IO3 0");
        assert!(matches!(result, Ok(ArtisanCommand::SetFan(0))));
    }

    /// TEST-18-05j: Verify IO3 with value 100 parses correctly
    #[test]
    fn test_parse_io3_max_value() {
        let result = parse_artisan_command("IO3 100");
        assert!(matches!(result, Ok(ArtisanCommand::SetFan(100))));
    }

    /// TEST-18-05k: Verify IO3 with value > 100 returns OutOfRange error
    #[test]
    fn test_parse_io3_out_of_range() {
        let result = parse_artisan_command("IO3 150");
        assert!(matches!(result, Err(ParseError::OutOfRange)));
    }
}
