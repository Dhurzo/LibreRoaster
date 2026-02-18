#![cfg_attr(not(any(test, feature = "test")), no_std)]

extern crate alloc;

#[cfg(any(test, feature = "test"))]
extern crate std;

// Custom embassy-time driver only for riscv32 embedded tests
// When using --features std, embassy-time provides its own driver via std feature
#[cfg(all(any(test, feature = "test"), target_arch = "riscv32"))]
#[no_mangle]
fn _embassy_time_now() -> u64 {
    0
}

pub mod application;
pub mod config;
pub mod control;
pub mod error;
pub mod hardware;
pub mod input;
pub mod logging;
pub mod output;
