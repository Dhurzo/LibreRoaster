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
