use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("GPIO error: {0}")]
    GpioError(#[from] esp_idf_hal::gpio::Error),

    #[error("WiFi error: {0}")]
    WifiError(String),

    #[error("HTTP server error: {0}")]
    ServerError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Peripheral error: Failed to take peripherals")]
    PeripheralError,

    #[error("Event loop error: Failed to take system event loop")]
    EventLoopError,

    #[error("NVS error: Failed to take NVS partition")]
    NvsError,

    #[error("Timer error: {0}")]
    TimerError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("LED controller error: {0}")]
    LedError(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl AppError {
    pub fn wifi<S: Into<String>>(msg: S) -> Self {
        Self::WifiError(msg.into())
    }

    pub fn server<S: Into<String>>(msg: S) -> Self {
        Self::ServerError(msg.into())
    }

    pub fn config<S: Into<String>>(msg: S) -> Self {
        Self::ConfigError(msg.into())
    }

    pub fn timer<S: Into<String>>(msg: S) -> Self {
        Self::TimerError(msg.into())
    }

    pub fn led<S: Into<String>>(msg: S) -> Self {
        Self::LedError(msg.into())
    }
}

pub type Result<T> = anyhow::Result<T, AppError>;