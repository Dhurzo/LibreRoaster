//! Panic-path heater cut-off (F-H4).
//!
//! `esp-backtrace` halts in a busy loop after a panic while the LEDC keeps
//! driving the SSR at its last duty until the RTC watchdog resets the chip
//! (~2.2 s). `cut_heater_on_panic` re-muxes the SSR pin to a plain GPIO
//! output at LOW, which disconnects the LEDC signal immediately.

/// Drive the SSR pin LOW. Called from every binary's `custom_pre_backtrace`
/// hook, before the backtrace is printed.
#[cfg(target_arch = "riscv32")]
pub fn cut_heater_on_panic() {
    use esp_hal::gpio::{Level, Output, OutputConfig};
    const _: () = assert!(crate::config::constants::SSR_CONTROL_PIN == 10);
    // SAFETY: panic path — normal execution is over, nothing else will use
    // GPIO10 again; we only re-mux it as a plain output at LOW.
    let peripherals = unsafe { esp_hal::peripherals::Peripherals::steal() };
    // `Output` has no Drop impl in esp-hal 1.2: the pin stays LOW.
    let _ssr = Output::new(peripherals.GPIO10, Level::Low, OutputConfig::default());
}
