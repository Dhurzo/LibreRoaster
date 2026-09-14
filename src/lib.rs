//! `libreroaster` — `no_std` library crate for the LibreRoaster ESP32-C3 coffee-roaster
//! controller.
//!
//! Exposes the firmware modules (`application`, `control`, `hardware`, `input`, `output`,
//! `safety`, `config`, `error`, `logging`, `memory`), and on host targets a `common`
//! helper module plus the `host_time_driver` embassy-time backend used by the host test
//! suite. The embedded entry point lives in `src/main.rs`.

#![cfg_attr(not(any(test, feature = "test")), no_std)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]
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
