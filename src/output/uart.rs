use crate::output::traits::{OutputError, SerialOutput};

/// Trait for UART communication channels (DIP compliance)
pub trait UartChannel {
    fn send(&self, data: &str) -> impl core::future::Future<Output = Result<(), OutputError>> + Send;
}

/// Default UART channel implementation using existing send_stream
pub struct DefaultUartChannel;

impl UartChannel for DefaultUartChannel {
    fn send(&self, data: &str) -> impl core::future::Future<Output = Result<(), OutputError>> + Send {
        async move {
            crate::hardware::uart::send_stream(data)
                .await
                .map_err(|_| OutputError::SerialComm)
        }
    }
}

pub struct UartPrinter<T: UartChannel> {
    enabled: bool,
    channel: T,
}

/// Builder for UartPrinter with dependency injection
pub struct UartPrinterBuilder {
    enabled: bool,
}

impl UartPrinterBuilder {
    pub fn new() -> Self {
        Self { enabled: true }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn build(self) -> UartPrinter<DefaultUartChannel> {
        UartPrinter::with_channel(DefaultUartChannel, self.enabled)
    }

    pub fn with_channel<T: UartChannel>(self, channel: T) -> UartPrinter<T> {
        UartPrinter::with_channel(channel, self.enabled)
    }
}

impl<T: UartChannel> UartPrinter<T> {
    pub fn with_channel(channel: T, enabled: bool) -> Self {
        Self { enabled, channel }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl UartPrinter<DefaultUartChannel> {
    pub fn new() -> Self {
        Self::with_channel(DefaultUartChannel, true)
    }
}

impl<T: UartChannel> SerialOutput for UartPrinter<T> {
    async fn print(&mut self, data: &str) -> Result<(), OutputError> {
        if !self.enabled {
            return Ok(());
        }
        self.channel.send(data).await
    }
}

impl Default for UartPrinter<DefaultUartChannel> {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for UartPrinterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock UART channel for testing purposes
#[cfg(test)]
pub struct MockUartChannel {
    pub sent_data: heapless::Vec<heapless::String<256>, 100>,
}

#[cfg(test)]
impl MockUartChannel {
    pub fn new() -> Self {
        Self {
            sent_data: heapless::Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.sent_data.clear();
    }

    pub fn get_sent_data(&self) -> &[heapless::String<256>] {
        &self.sent_data
    }
}

#[cfg(test)]
impl Default for MockUartChannel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl UartChannel for MockUartChannel {
    fn send(&mut self, data: &str) -> impl core::future::Future<Output = Result<(), OutputError>> + Send {
        async move {
            let _ = self.sent_data.push(heapless::String::<256>::try_from(data).unwrap_or_default());
            Ok(())
        }
    }
}