//! Minimal ESP32 Blinky - Single HAL dependency
//! Uses only esp-idf-hal with minimal working configuration

use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::gpio::PinDriver;
use esp_idf_hal::peripherals::Peripherals;

#[no_std]
fn main() -> ! {
    // Link patches needed for ESP32
    unsafe {
        esp_idf_sys::link_patches();
    }
    
    // Initialize peripherals
    let peripherals = Peripherals::take().unwrap_or_else(|_| panic!("Failed to get peripherals"));
    
    // Configure GPIO2 as output (built-in LED on NodeMCU)
    let mut led = PinDriver::output(peripherals.pins.gpio2).unwrap_or_else(|_| panic!("Failed to configure LED"));
    
    // Simple blink loop
    loop {
        led.set_low().unwrap_or_else(|_| panic!("LED set low failed"));
        FreeRtos::delay_ms(1000);
        led.set_high().unwrap_or_else(|_| panic!("LED set high failed"));
        FreeRtos::delay_ms(1000);
    }
}