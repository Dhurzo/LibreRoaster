//! Hardware abstraction layer for LibreRoaster ESP32-C3.
//!
//! # Single-core assumption
//!
//! All `unsafe impl Send` and `unsafe impl Sync` in this module rely on the
//! ESP32-C3 being a single-core RISC-V processor. If porting to a multi-core
//! variant (e.g., ESP32-S3), every `Send`/`Sync` impl must be revisited.

#[cfg(target_arch = "riscv32")]
/// Fan actuator (LEDC PWM) control.
pub mod fan;
#[cfg(not(target_arch = "riscv32"))]
#[path = "fan_host.rs"]
/// Fan actuator (LEDC PWM) control — host build.
pub mod fan;

/// Heat-source detection debounce for the SSR current-sense pin.
pub mod heat_presence;
#[cfg(target_arch = "riscv32")]
/// Hardware initialization (SPI, sensors, actuators, USB/UART).
pub mod init;
#[cfg(target_arch = "riscv32")]
/// Shared LEDC (PWM) peripheral bus abstractions.
pub mod ledc_bus;
/// Guards LEDC PWM channel configuration against unsafe reconfiguration.
pub mod ledc_guard;
/// MAX31856 thermocouple driver (SPI).
pub mod max31856;
/// Sensor trait implementations and temperature conversions.
pub mod sensors;
/// Shared SPI bus abstraction for the two MAX31856 devices.
pub mod shared_spi;
#[cfg(target_arch = "riscv32")]
/// SSR heater actuator (embedded build).
pub mod ssr;
#[cfg(not(target_arch = "riscv32"))]
#[path = "ssr_stub.rs"]
/// SSR heater actuator (host stub).
pub mod ssr;
/// Pure SSR availability state machine (un-gated, host-tested).
pub mod ssr_logic;

/// Cross-task comms/output error counters.
pub mod error_counters;
/// UnsafeCell-based Sync wrapper for static driver singletons.
pub mod static_sync;
/// Status LED pattern logic (pure, host-testable).
pub mod status_led;
// Fase 2 (BUG-CATCH-PLAN.md): the mocks use `alloc::sync::Arc` +
// `critical_section::Mutex`, which require `target_has_atomic = "ptr"` —
// unavailable on riscv32imc (no atomic extension). Nothing outside the test
// surface references the mocks, so gate the module to host test builds.
#[cfg(any(test, feature = "test"))]
/// Host test mocks for thermometer, SSR, and fan.
pub mod test_mocks;
/// USB/UART reader and output task plumbing.
pub mod transport_tasks;
/// UART0 communication (reader task).
pub mod uart;
#[cfg(target_arch = "riscv32")]
#[path = "usb_cdc/mod.rs"]
/// USB CDC (native USB) communication (reader task) — embedded build.
pub mod usb_cdc;
#[cfg(not(target_arch = "riscv32"))]
#[path = "usb_cdc/mod.rs"]
/// USB CDC (native USB) communication (reader task) — host build.
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
