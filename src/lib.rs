#![cfg_attr(not(any(test, feature = "test")), no_std)]

extern crate alloc;

#[cfg(any(test, feature = "test"))]
extern crate std;

// Provide a stubbed `_embassy_time_now` for riscv so the embedded driver links cleanly.
#[cfg(target_arch = "riscv32")]
#[no_mangle]
pub extern "C" fn _embassy_time_now() -> u64 {
    0
}

#[cfg(target_arch = "riscv32")]
#[no_mangle]
pub extern "C" fn _embassy_time_schedule_wake(_at: u64, _waker: &core::task::Waker) {}

#[cfg(not(target_arch = "riscv32"))]
#[no_mangle]
pub extern "C" fn _embassy_time_now() -> u64 {
    0
}

#[cfg(not(target_arch = "riscv32"))]
#[no_mangle]
pub extern "C" fn _embassy_time_schedule_wake(_at: u64, waker: &core::task::Waker) {
    waker.wake_by_ref();
}

pub mod application;
pub mod config;
pub mod control;
pub mod error;
pub mod hardware;
pub mod input;
pub mod logging;
pub mod memory;
pub mod output;
pub mod safety;

#[cfg(not(target_arch = "riscv32"))]
pub mod common;
