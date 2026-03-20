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
        waker.wake_by_ref();
    }
}

time_driver_impl!(static HOST_TIME_DRIVER: HostTimeDriver = HostTimeDriver::new());
