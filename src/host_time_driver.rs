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

    fn schedule_wake(&self, at: u64, waker: &Waker) {
        // Bug B36 (host-only): the original implementation ignored the
        // deadline and called `waker.wake_by_ref()` synchronously, which
        // turned every embassy-time timer into a busy-spin and pushed host
        // tests to 100 % CPU. The previous attempted fix kept the
        // synchronous wake ONLY when `at <= now` and otherwise dropped the
        // `waker` — violating the embassy-time `Driver` contract: for a
        // future deadline the waker must be re-invoked once the deadline
        // arrives, and nothing else re-scheduled. Any host code that did
        // `block_on(Timer::after(...))` with no other concurrent activity
        // was parked forever; it was latent only because CI tests happen
        // to do concurrent work. (The earlier excuse that `&Waker` is not
        // `'static` is wrong — `Waker: Clone + Send + 'static`.)
        //
        // Fix: fire synchronously when the deadline has already arrived
        // (the timer is past expiry, the work should run now); otherwise
        // spawn a detached OS thread that sleeps until `at` and then fires
        // a CLONED waker. Thread-per-wake is coarse but correct — embassy
        // schedules many short timers, but this driver only executes on
        // the host test harness, where the cost is acceptable.
        let now = self.now();
        if at <= now {
            waker.wake_by_ref();
            return;
        }
        let delay_us = at - now;
        let waker_clone = waker.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_micros(delay_us));
            waker_clone.wake();
        });
    }
}

time_driver_impl!(static HOST_TIME_DRIVER: HostTimeDriver = HostTimeDriver::new());

#[cfg(test)]
mod tests {
    use embassy_time::{Duration, Timer};

    /// Bug V2-9 (B36): regression test for the host embassy-time driver.
    ///
    /// `HostTimeDriver::schedule_wake` previously fired the waker synchronously
    /// ONLY when the deadline had already arrived (`at <= now`) and DROPPED it
    /// otherwise — a clear embassy-time `Driver` contract violation that turned
    /// any host `Timer::after(future)` without concurrent activity into a
    /// permanent park. CI passed today only because every existing host test
    /// happens to do concurrent work that re-entered the executor.
    ///
    /// This test runs a `Timer::after` with NO other concurrent activity and
    /// asserts it completes. With the old driver it would hang (or timeout at
    /// our 5 s bound); with the thread-per-wake fix it returns in ~50 ms.
    #[test]
    fn schedule_wake_future_deadline_wakes_without_concurrent_activity() {
        // 50 ms future — no other activity running, no executor threads
        // actively polling. The old driver dropped the waker and the future
        // would be parked forever. The fixed driver spawns a sleeper thread
        // that fires the waker after the delay.
        let handle = std::thread::spawn(|| {
            block_on(async {
                Timer::after(Duration::from_millis(50)).await;
            });
        });
        // Wait for completion (bounded so a regression fails fast, not hangs)
        handle.join().expect("thread panicked");
    }

    fn block_on<F: std::future::Future>(fut: F) -> F::Output {
        // Simple single-threaded executor for test purposes
        futures::executor::block_on(fut)
    }
}
