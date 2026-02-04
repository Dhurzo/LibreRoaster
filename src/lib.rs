#![cfg(not(test))]
#![no_std]

extern crate alloc;

#[cfg(test)]
extern crate std;

pub mod application;
pub mod config;
pub mod control;
pub mod error;
pub mod hardware;
pub mod input;
pub mod output;
