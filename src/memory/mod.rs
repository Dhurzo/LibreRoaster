//! Memory module: heapless buffer-size constants (`constants`) and the dual
//! hot-path/initialization allocation strategy (`strategy`).

pub mod constants;
pub mod strategy;

pub use constants::*;
