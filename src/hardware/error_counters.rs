use core::cell::Cell;

// On riscv32imc (ESP32-C3), AtomicU32::fetch_add is not available.
// We use critical_section + Cell for safe cross-task counter updates.

#[derive(Clone, Copy)]
struct CommsErrorCounters {
    uart: u32,
    usb: u32,
}

static COUNTERS: critical_section::Mutex<Cell<CommsErrorCounters>> =
    critical_section::Mutex::new(Cell::new(CommsErrorCounters { uart: 0, usb: 0 }));

/// Increment the UART read error counter. Call on each read failure.
pub fn increment_uart_error_count() {
    critical_section::with(|cs| {
        let counters = COUNTERS.borrow(cs).get();
        COUNTERS.borrow(cs).set(CommsErrorCounters {
            uart: counters.uart.saturating_add(1),
            usb: counters.usb,
        });
    });
}

/// Reset the UART error counter. Call on successful read.
pub fn reset_uart_error_count() {
    critical_section::with(|cs| {
        let counters = COUNTERS.borrow(cs).get();
        COUNTERS.borrow(cs).set(CommsErrorCounters {
            uart: 0,
            usb: counters.usb,
        });
    });
}

/// Increment the USB CDC read error counter. Call on each read failure.
pub fn increment_usb_error_count() {
    critical_section::with(|cs| {
        let counters = COUNTERS.borrow(cs).get();
        COUNTERS.borrow(cs).set(CommsErrorCounters {
            uart: counters.uart,
            usb: counters.usb.saturating_add(1),
        });
    });
}

/// Reset the USB error counter. Call on successful read.
pub fn reset_usb_error_count() {
    critical_section::with(|cs| {
        let counters = COUNTERS.borrow(cs).get();
        COUNTERS.borrow(cs).set(CommsErrorCounters {
            uart: counters.uart,
            usb: 0,
        });
    });
}

/// Threshold: if either comms channel accumulates this many (net) read
/// errors without an intervening successful read, the control loop
/// triggers emergency shutdown.
pub const MAX_COMMS_READ_ERRORS: u32 = 10;

/// Check whether either comms channel has exceeded the error threshold.
pub fn any_comms_error_threshold_exceeded() -> bool {
    critical_section::with(|cs| {
        let counters = COUNTERS.borrow(cs).get();
        counters.uart >= MAX_COMMS_READ_ERRORS || counters.usb >= MAX_COMMS_READ_ERRORS
    })
}
