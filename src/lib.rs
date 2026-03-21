#![cfg_attr(not(any(test, feature = "test")), no_std)]
#![allow(clippy::type_complexity)]

extern crate alloc;

#[cfg(any(test, feature = "test"))]
extern crate std;

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

#[cfg(all(not(target_arch = "riscv32"), any(test, feature = "test")))]
mod host_time_driver;

#[cfg(not(target_arch = "riscv32"))]
pub mod common;
