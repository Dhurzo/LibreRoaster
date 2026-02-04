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
}
