//! Artisan Initialization State Machine
//!
//! NOTE: This module is COMMENTED OUT because Artisan Scope does not perform
//! a handshake sequence. Artisan simply sends and receives data without
//! requiring CHAN → UNITS → FILT initialization.
//!
//! The handshake logic is preserved here for future reference or if needed
//! for compatibility with other Artisan-compatible software that requires it.
//!
//! Original behavior:
//! Tracks the handshake sequence: CHAN → UNITS → FILT
//! States: Idle → ExpectingChan → ExpectingUnits → ExpectingFilt → Ready
//!
//! To re-enable: Remove the outer /* */ comments and uncomment the code below

/*

 use crate::config::ArtisanCommand;
 use crate::input::parser::ParseError;

 #[derive(Debug, Clone, Copy, PartialEq, Eq)]
 pub enum InitState {
    Idle,           // No handshake started
    ExpectingChan,  // Waiting for CHAN command
    ExpectingUnits, // Waiting for UNITS command
    ExpectingFilt,  // Waiting for FILT command
    Ready,          // Handshake complete, ready for operational commands
 }

 #[derive(Debug, Clone, Copy, PartialEq, Eq)]
 pub enum InitEvent {
    ChanReceived,
    UnitsReceived,
    FiltReceived,
    OperationalCommand,
 }

 #[derive(Debug, Clone, Copy)]
 pub struct ArtisanInitState {
    state: InitState,
    chan_value: Option<u16>,
    units_value: Option<bool>,
    filt_value: Option<u8>,
}

 impl ArtisanInitState {
     pub fn new() -> Self {
        Self {
            state: InitState::ExpectingChan,
            chan_value: None,
            units_value: None,
            filt_value: None,
        }
     }

     pub fn state(&self) -> InitState {
        self.state
    }

     pub fn is_ready(&self) -> bool {
        self.state == InitState::Ready
    }

     pub fn chan_value(&self) -> Option<u16> {
        self.chan_value
    }

     pub fn units_value(&self) -> Option<bool> {
        self.units_value
    }

     pub fn filt_value(&self) -> Option<u8> {
        self.filt_value
    }

     pub fn on_command(&mut self, command: ArtisanCommand) -> Result<InitEvent, ParseError> {
        match (self.state, command) {
            // CHAN command received - move to ExpectingUnits
            (InitState::ExpectingChan, ArtisanCommand::Chan(chan)) => {
                self.chan_value = Some(chan);
                self.state = InitState::ExpectingUnits;
                Ok(InitEvent::ChanReceived)
            }

            // UNITS command received - move to ExpectingFilt
            (InitState::ExpectingUnits, ArtisanCommand::Units(units)) => {
                self.units_value = Some(units);
                self.state = InitState::ExpectingFilt;
                Ok(InitEvent::UnitsReceived)
            }

            // FILT command received - initialization complete
            (InitState::ExpectingFilt, ArtisanCommand::Filt(filt)) => {
                self.filt_value = Some(filt);
                self.state = InitState::Ready;
                Ok(InitEvent::FiltReceived)
            }

            // Operational commands only valid in Ready state
            (_, ArtisanCommand::ReadStatus)
            | (_, ArtisanCommand::StartRoast)
            | (_, ArtisanCommand::SetHeater(_))
            | (_, ArtisanCommand::SetFan(_))
            | (_, ArtisanCommand::IncreaseHeater)
            | (_, ArtisanCommand::DecreaseHeater)
            | (_, ArtisanCommand::EmergencyStop) => {
                if self.state == InitState::Ready {
                    Ok(InitEvent::OperationalCommand)
                } else {
                    // Operational command received before initialization complete
                    Err(ParseError::UnknownCommand)
                }
            }

            // Wrong command for current state
            _ => Err(ParseError::UnknownCommand),
        }
    }

     pub fn reset(&mut self) {
        self.state = InitState::ExpectingChan;
        self.chan_value = None;
        self.units_value = None;
        self.filt_value = None;
    }
}

impl Default for ArtisanInitState {
    fn default() -> Self {
        Self::new()
    }
}

*/

#[cfg(test)]
mod tests {
    use crate::config::ArtisanCommand;

    #[test]
    fn test_handshake_disabled_operational_commands_work() {
        use crate::input::parser::parse_artisan_command;

        assert!(parse_artisan_command("READ").is_ok());
        assert!(parse_artisan_command("OT1 50").is_ok());
        assert!(parse_artisan_command("IO3 75").is_ok());
        assert!(parse_artisan_command("START").is_ok());
        assert!(parse_artisan_command("STOP").is_ok());
    }

    #[test]
    fn test_handshake_commands_parse_but_ignored() {
        use crate::input::parser::parse_artisan_command;

        assert!(parse_artisan_command("CHAN 1200").is_ok());
        assert!(parse_artisan_command("UNITS;C").is_ok());
        assert!(parse_artisan_command("FILT 5").is_ok());
    }
}
