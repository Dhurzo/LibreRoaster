//! Hardware abstraction layer for LibreRoaster ESP32-C3.
//!
//! # Single-core assumption
//!
//! All `unsafe impl Send` and `unsafe impl Sync` in this module rely on the
//! ESP32-C3 being a single-core RISC-V processor. If porting to a multi-core
//! variant (e.g., ESP32-S3), every `Send`/`Sync` impl must be revisited.

#[cfg(target_arch = "riscv32")]
pub mod fan;
#[cfg(not(target_arch = "riscv32"))]
#[path = "fan_host.rs"]
pub mod fan;

#[cfg(target_arch = "riscv32")]
pub mod init;
#[cfg(target_arch = "riscv32")]
pub mod ledc_bus;
pub mod ledc_guard;
pub mod max31856;
pub mod sensors;
pub mod shared_spi;
#[cfg(target_arch = "riscv32")]
pub mod ssr;
#[cfg(not(target_arch = "riscv32"))]
#[path = "ssr_stub.rs"]
pub mod ssr;

pub mod error_counters;
pub mod static_sync;
pub mod test_mocks;
pub mod uart;
#[cfg(target_arch = "riscv32")]
#[path = "usb_cdc/mod.rs"]
pub mod usb_cdc;
#[cfg(not(target_arch = "riscv32"))]
#[path = "usb_cdc/mod.rs"]
pub mod usb_cdc;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_traits_ecosystem_compatible() {
        // Verify hardware errors can be used with generic embedded-hal code
        use embedded_hal::digital::Error as DigitalError;
        use embedded_hal::spi::Error as SpiError;

        let spi_err = max31856::Max31856Error::CommunicationError { source: "test" };
        let _ = spi_err.kind();

        #[cfg(target_arch = "riscv32")]
        {
            use ssr::SsrError;
            let digital_err = SsrError::OutputError { source: "test" };
            let _ = digital_err.kind();
        }

        let fan_err = fan::FanError::InitializationError { source: "test" };
        let _ = fan_err.kind();
    }
}
