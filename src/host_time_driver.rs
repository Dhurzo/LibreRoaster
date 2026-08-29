//! Host (non-embedded) `embassy-time` driver used by the host test suite.
//!
//! Provides `embassy_time_driver::Driver` via a single worker thread that sleeps until the
//! earliest scheduled deadline and fires all due wakers, so host `Timer` futures advance
//! without spawning one OS thread per wake. Compiled only when `target_arch` is not
//! `riscv32`.

#![cfg(not(target_arch = "riscv32"))]

use core::task::Waker;
use embassy_time_driver::{time_driver_impl, Driver};
use std::sync::{Condvar, Mutex, Once, OnceLock};
use std::time::Instant;

const TICKS_PER_SEC: u64 = 1_000_000;
const NANOS_PER_SEC: u128 = 1_000_000_000;

struct PendingWake {
    at: u64,
    waker: Waker,
}

struct HostTimeDriver {
    start: OnceLock<Instant>,
    /// Deadlines with future timestamps, guarded by the mutex.
    pending: Mutex<Vec<PendingWake>>,
    /// Notified whenever a new deadline is scheduled (possibly earlier than
    /// the current wait), so the single worker re-sleeps to the earliest one.
    changed: Condvar,
}

impl HostTimeDriver {
    /// Create an empty driver with no scheduled wakes yet.
    const fn new() -> Self {
        Self {
            start: OnceLock::new(),
            pending: Mutex::new(Vec::new()),
            changed: Condvar::new(),
        }
    }

    /// Return the instant this driver first became active (monotonic baseline).
    fn baseline(&self) -> Instant {
        *self.start.get_or_init(Instant::now)
    }
}

/// Bug M5 (2026-08-10): the previous driver spawned one detached OS thread
/// PER `schedule_wake` with a future deadline. `embassy_time::Timer::poll`
/// calls `schedule_wake` on EVERY poll of a pending timer, so any
/// `select(Timer, channel)` with sustained traffic re-scheduled the pending
/// timer on every wake — one new thread each time — until RLIMIT_NPROC was
/// exhausted and the whole test suite died. The fix mirrors embassy-time's
/// own `driver_std.rs`: a SINGLE worker thread sleeps until the earliest
/// deadline and fires all due wakers; `schedule_wake` only enqueues
/// (deduping by `waker.will_wake` so repeated polls of the same timer do not
/// grow the queue) and notifies the worker.
fn worker_loop() {
    loop {
        // Collect all wakers whose deadline has arrived, out of the lock.
        let (due, next_deadline) = {
            let mut pending = HOST_TIME_DRIVER
                .pending
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            let now = HOST_TIME_DRIVER.now();
            let mut due: Vec<Waker> = Vec::new();
            let mut i = 0;
            while i < pending.len() {
                if pending[i].at <= now {
                    due.push(pending.swap_remove(i).waker);
                } else {
                    i += 1;
                }
            }
            let next = pending.iter().map(|p| p.at).min();
            (due, next)
        };

        if !due.is_empty() {
            for waker in due {
                waker.wake();
            }
            continue;
        }

        // Sleep until the earliest deadline — computed UNDER the lock so a
        // `schedule_wake` racing between the computation and the wait cannot
        // be lost (it would need the lock, which we only release inside
        // `wait_timeout`, by which point the waiter is registered).
        let pending = HOST_TIME_DRIVER
            .pending
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let wait = match next_deadline.or_else(|| pending.iter().map(|p| p.at).min()) {
            Some(next) => {
                let now = HOST_TIME_DRIVER.now();
                if next <= now {
                    continue;
                }
                std::time::Duration::from_micros(next - now)
            }
            None => std::time::Duration::from_secs(3600),
        };
        let _ = HOST_TIME_DRIVER
            .changed
            .wait_timeout(pending, wait)
            // Poison-tolerant: a panicked test thread must not kill the
            // driver; the mutex state is still usable.
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
}

/// Lazily spawn the single worker thread that services pending wake deadlines.
fn spawn_worker() {
    static WORKER: Once = Once::new();
    WORKER.call_once(|| {
        let res = std::thread::Builder::new()
            .name("embassy-host-time".to_owned())
            .spawn(worker_loop);
        if res.is_err() {
            // The host time driver is the heartbeat of the test harness —
            // without it every future-deadline timer parks forever.
            eprintln!("fatal: failed to spawn host time driver thread");
            std::process::abort();
        }
    });
}

impl Driver for HostTimeDriver {
    /// Current time in embassy ticks since the driver baseline.
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
        // enqueue on the shared worker thread (bug M5).
        let now = self.now();
        if at <= now {
            waker.wake_by_ref();
            return;
        }
        spawn_worker();
        let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
        // Dedupe: repeated polls of the same timer re-schedule the same
        // waker — keep ONE entry with the earliest deadline so the queue
        // cannot grow with the poll count.
        let mut replaced = false;
        for entry in pending.iter_mut() {
            if entry.waker.will_wake(waker) {
                if at < entry.at {
                    entry.at = at;
                }
                replaced = true;
                break;
            }
        }
        if !replaced {
            pending.push(PendingWake {
                at,
                waker: waker.clone(),
            });
        }
        self.changed.notify_one();
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
