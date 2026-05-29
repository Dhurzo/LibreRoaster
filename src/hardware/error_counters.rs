use core::cell::UnsafeCell;

// On riscv32imc (ESP32-C3), AtomicU32::fetch_add is not available.
// We use critical_section + UnsafeCell for safe cross-task counter updates.

struct CommsErrorCounters {
    uart: u32,
    usb: u32,
}

static COUNTERS: critical_section::Mutex<UnsafeCell<CommsErrorCounters>> =
    critical_section::Mutex::new(UnsafeCell::new(CommsErrorCounters { uart: 0, usb: 0 }));

/// Increment the UART read error counter. Call on each read failure.
pub fn increment_uart_error_count() {
    critical_section::with(|cs| {
        let cell = COUNTERS.borrow(cs).get();
        // SAFETY: critical section provides mutual exclusion; no other task
        // can access COUNTERS concurrently.
        unsafe {
            (*cell).uart = (*cell).uart.saturating_add(1);
        }
    });
}

/// Reset the UART error counter. Call on successful read.
pub fn reset_uart_error_count() {
    critical_section::with(|cs| {
        let cell = COUNTERS.borrow(cs).get();
        unsafe {
            (*cell).uart = 0;
        }
    });
}

/// Increment the USB CDC read error counter. Call on each read failure.
pub fn increment_usb_error_count() {
    critical_section::with(|cs| {
        let cell = COUNTERS.borrow(cs).get();
        unsafe {
            (*cell).usb = (*cell).usb.saturating_add(1);
        }
    });
}

/// Reset the USB error counter. Call on successful read.
pub fn reset_usb_error_count() {
    critical_section::with(|cs| {
        let cell = COUNTERS.borrow(cs).get();
        unsafe {
            (*cell).usb = 0;
        }
    });
}

/// Threshold: if either comms channel accumulates this many (net) read
/// errors without an intervening successful read, the control loop
/// triggers emergency shutdown.
pub const MAX_COMMS_READ_ERRORS: u32 = 10;

/// Check whether either comms channel has exceeded the error threshold.
pub fn any_comms_error_threshold_exceeded() -> bool {
    critical_section::with(|cs| {
        let cell = COUNTERS.borrow(cs).get();
        // SAFETY: critical section guarantees exclusive access.
        unsafe { (*cell).uart >= MAX_COMMS_READ_ERRORS || (*cell).usb >= MAX_COMMS_READ_ERRORS }
    })
}
