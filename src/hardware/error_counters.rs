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

/// Generic increment for transport-specific error counter by name.
/// Used by generic transport tasks.
pub fn increment_error_count(name: &str) {
    match name {
        "UART" => increment_uart_error_count(),
        "USB" => increment_usb_error_count(),
        _ => {}
    }
}

/// Generic reset for transport-specific error counter by name.
/// Used by generic transport tasks.
pub fn reset_error_count(name: &str) {
    match name {
        "UART" => reset_uart_error_count(),
        "USB" => reset_usb_error_count(),
        _ => {}
    }
}

// ── Output-direction counters ──────────────────────────────────────────────
// Audit H-1 (2026-08-11): reads had first-class error accounting while the
// OUTPUT direction was invisible: `let _ = try_send(...)` swallowed
// channel-full drops and `dual_output_tick` swallowed USB/UART write
// failures. These saturating counters close that gap. They are intentionally
// NOT part of the TC4 STATUS wire line (20 fields, byte-exact, pinned by
// golden tests) — they are exposed via the getters below and the telemetry
// trail (debug!/warn! in tasks.rs).

#[derive(Clone, Copy)]
struct OutputCounters {
    channel_drops: u32,
    usb_write_failures: u32,
    uart_write_failures: u32,
}

static OUTPUT_COUNTERS: critical_section::Mutex<Cell<OutputCounters>> =
    critical_section::Mutex::new(Cell::new(OutputCounters {
        channel_drops: 0,
        usb_write_failures: 0,
        uart_write_failures: 0,
    }));

#[inline]
fn bump_output_counter(mutate: impl FnOnce(&mut OutputCounters)) {
    critical_section::with(|cs| {
        let mut counters = OUTPUT_COUNTERS.borrow(cs).get();
        mutate(&mut counters);
        OUTPUT_COUNTERS.borrow(cs).set(counters);
    });
}

/// Account a message dropped because the output channel was full.
pub fn increment_output_drop_count() {
    bump_output_counter(|c| c.channel_drops = c.channel_drops.saturating_add(1));
}

/// Account a failed USB CDC write in `dual_output_tick`.
pub fn increment_usb_write_failure() {
    bump_output_counter(|c| c.usb_write_failures = c.usb_write_failures.saturating_add(1));
}

/// Account a failed UART write in `dual_output_tick`.
pub fn increment_uart_write_failure() {
    bump_output_counter(|c| c.uart_write_failures = c.uart_write_failures.saturating_add(1));
}

/// Total messages dropped because the output channel was full.
pub fn output_drop_count() -> u32 {
    critical_section::with(|cs| OUTPUT_COUNTERS.borrow(cs).get().channel_drops)
}

/// Failed USB CDC writes (host unplugged / Artisan killed).
pub fn usb_write_failure_count() -> u32 {
    critical_section::with(|cs| OUTPUT_COUNTERS.borrow(cs).get().usb_write_failures)
}

/// Failed UART writes.
pub fn uart_write_failure_count() -> u32 {
    critical_section::with(|cs| OUTPUT_COUNTERS.borrow(cs).get().uart_write_failures)
}

/// Total failed output writes across both transports.
pub fn output_write_failure_count() -> u32 {
    critical_section::with(|cs| {
        let c = OUTPUT_COUNTERS.borrow(cs).get();
        c.usb_write_failures.saturating_add(c.uart_write_failures)
    })
}

/// Reset the output counters (test teardown / session start).
pub fn reset_output_counters() {
    critical_section::with(|cs| {
        OUTPUT_COUNTERS.borrow(cs).set(OutputCounters {
            channel_drops: 0,
            usb_write_failures: 0,
            uart_write_failures: 0,
        });
    });
}

/// Best-effort send to the output channel with drop accounting.
///
/// Generic over the channel type so `hardware` never depends on
/// `application`'s `OutputChannel` alias. Returns `true` when the message
/// was accepted, `false` (and increments the drop counter) when full.
pub fn try_send_output<M, const N: usize>(
    channel: &embassy_sync::channel::Channel<
        embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
        M,
        N,
    >,
    msg: M,
) -> bool {
    match channel.try_send(msg) {
        Ok(()) => true,
        Err(_returned) => {
            increment_output_drop_count();
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embassy_sync::channel::Channel;
    use heapless::String;

    // Single combined test: the output counters live in one static, so
    // parallel tests would race each other's resets.
    #[test]
    fn output_counters_count_drops_and_saturate() {
        reset_output_counters();
        static CH: Channel<
            embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
            String<8>,
            2,
        > = Channel::new();

        // Channel full → try_send_output must fail AND be counted.
        assert!(try_send_output(&CH, String::<8>::try_from("a").unwrap()));
        assert!(try_send_output(&CH, String::<8>::try_from("b").unwrap()));
        assert_eq!(output_drop_count(), 0);
        assert!(!try_send_output(&CH, String::<8>::try_from("c").unwrap()));
        assert!(!try_send_output(&CH, String::<8>::try_from("d").unwrap()));
        assert_eq!(output_drop_count(), 2);

        // Drain one slot; the next send succeeds again and the counter stops.
        let _ = CH.try_receive();
        assert!(try_send_output(&CH, String::<8>::try_from("e").unwrap()));
        assert_eq!(output_drop_count(), 2);

        // Write-failure counters.
        increment_uart_write_failure();
        increment_uart_write_failure();
        increment_usb_write_failure();
        assert_eq!(uart_write_failure_count(), 2);
        assert_eq!(usb_write_failure_count(), 1);
        assert_eq!(output_write_failure_count(), 3);

        // Saturation: park the counter one below the ceiling, then add — a
        // wrap (overflow-checks / debug) would panic or go negative.
        bump_output_counter(|c| c.channel_drops = u32::MAX - 1);
        increment_output_drop_count();
        increment_output_drop_count();
        assert_eq!(output_drop_count(), u32::MAX);
    }
}
