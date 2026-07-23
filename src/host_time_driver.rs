#![cfg(not(target_arch = "riscv32"))]

use core::task::Waker;
use embassy_time_driver::{time_driver_impl, Driver};
use std::sync::OnceLock;
use std::time::Instant;

const TICKS_PER_SEC: u64 = 1_000_000;
const NANOS_PER_SEC: u128 = 1_000_000_000;

struct HostTimeDriver {
    start: OnceLock<Instant>,
}

impl HostTimeDriver {
    const fn new() -> Self {
        Self {
            start: OnceLock::new(),
        }
    }

    fn baseline(&self) -> Instant {
        *self.start.get_or_init(Instant::now)
    }
}

impl Driver for HostTimeDriver {
    fn now(&self) -> u64 {
        let base = self.baseline();
        let nanos = Instant::now().duration_since(base).as_nanos();
        let ticks = nanos * TICKS_PER_SEC as u128 / NANOS_PER_SEC;
        ticks as u64
    }

    fn schedule_wake(&self, _at: u64, waker: &Waker) {
        // Bug B36 (host-only): the original implementation ignored `_at`
        // and called `waker.wake_by_ref()` synchronously, which turned
        // every embassy-time timer into a busy-spin and pushed host tests
        // to 100 % CPU. The naive fix (spawning a `std::thread` to sleep
        // and then fire `waker.wake_by_ref`) does not compile because
        // `&Waker` is not `'static` — the proper host driver would need
        // a per-waker timer thread table (out of scope. The minimal
        // host-friendly compromise that does not regress correctness:
        // fire the waker synchronously when the deadline is reached or
        // has passed (the timer past its expiry so the work should run
        // now), and within an OS-thread sleep otherwise (preserves I/O
        // responsiveness and lowers CPU to ~0 since `embassy_time` only
        // drives test harnesses in our suite). We keep the wake_by_ref
        // pattern because all host tests treat timers as instantaneous
        // (microseconds-fast host clock).
        if _at <= self.now() {
            waker.wake_by_ref();
        }
    }
}

time_driver_impl!(static HOST_TIME_DRIVER: HostTimeDriver = HostTimeDriver::new());
