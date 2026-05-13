//! # LibreRoaster Memory Strategy
//!
//! This document defines the unified memory management strategy for LibreRoaster,
//! focused on RAM predictability and real-time performance.
//!
//! ## Design Philosophy
//!
//! LibreRoaster uses a **dual memory strategy**:
//! - **HOT PATH**: Exclusively heapless/stack for real-time critical operations
//! - **INITIALIZATION**: Heap allowed only during initialization and setup
//!
//! This duality ensures that critical operations have deterministic execution times
//! while allowing flexibility where it does not impact real-time performance.
//!
//! ## Module Classification
//!
//! ### HOT PATH Modules (heapless only)
//!
//! These modules operate on real-time critical paths and **MUST NOT** perform
//! any dynamic allocation during execution.
//!
//! ```text
//! // ✅ Allowed in HOT PATH
//! heapless::Vec<u8, 64>
//! heapless::String<32>
//! stack arrays: [u8; 32]
//! primitives: f32, u32, bool
//!
//! // ❌ Prohibido en HOT PATH
//! alloc::vec::Vec
//! alloc::string::String
//! alloc::boxed::Box
//! any dynamic allocation
//! ```
//!
//! **Identified HOT PATH modules:**
//! - `hardware/max31856/` - Temperature reading via SPI
//! - `control/pid/` - PID control calculations
//! - `hardware/ssr/` - SSR PWM control
//! - `hardware/ledc_*` - PWM signal generation
//! - `output/artisan/` - Output formatting for Artisan
//!
//! **HOT PATH characteristics:**
//! - Deterministic execution times (±5%)
//! - Zero dynamic allocations during normal operation
//! - Predictable memory usage
//! - No heap allocator dependency
//!
//! ### INITIALIZATION Modules (heap allowed)
//!
//! These modules operate during system initialization or on non-critical paths
//! where allocations do not impact real-time performance.
//!
//! ```text
//! // ✅ Allowed in INITIALIZATION
//! alloc::boxed::Box<dyn Trait>  // for dynamic dispatch
//! alloc::string::String         // for configuration messages
//! alloc::vec::Vec               // for initial construction
//!
//! // ⚠️ Use with documented justification
//! any allocation that could affect real time
//! ```
//!
//! **Identified INITIALIZATION modules:**
//! - `application/app_builder/` - Application construction
//! - `control/policies/` - Control policies (non-critical evaluation)
//! - `application/tasks/` - Async task creation
//! - `hardware/uart/` - UART driver initialization
//!
//! **INITIALIZATION rules:**
//! - All allocations must occur before the main loop
//! - Document any allocation that could impact critical paths
//! - Prefer pre-allocation over dynamic allocation
//!
//! ### MIXED Modules (document carefully)
//!
//! These modules can have both hot path and initialization operations,
//! requiring careful design and explicit documentation.
//!
//! **Identified MIXED modules:**
//! - `error/app_error.rs` - Error handling (can occur at any time)
//! - `input/parser.rs` - Command parsing (init + runtime)
//! - `application/stage_instrumentation.rs` - Instrumentation (periodic reporting)
//!
//! **MIXED rules:**
//! - Clearly separate hot path vs initialization operations
//! - Document each function with its memory category
//! - Use heapless for operations that may occur at runtime
//!
//! ## Memory Constants
//!
//! To ensure consistency in heapless buffer sizes,
//! standard constants are defined in `memory::constants`.
//!
//! ```rust
//! /// Maximum size for error messages in hot paths
//! pub const ERROR_MSG_MAX_LEN: usize = 128;
//!
//! /// Maximum size for Artisan commands
//! pub const ARTISAN_CMD_MAX_LEN: usize = 64;
//!
//! // Size for temperature report buffers
//! // pub const REPORT_BUFFER_SIZE: usize = 32;
//! ```
//!
//! ## Recommended Patterns
//!
//! ### For HOT PATH Operations
//!
//! ```rust
//! use heapless::{String, Vec};
//!
//! pub fn read_temperature(&mut self) -> Result<f32, TemperatureError> {
//!     // ✅ Use fixed-size buffers
//!     let mut buffer: [u8; 3] = [0; 3];
//!     let mut error_msg: String<32> = String::new();
//!
//!     // ✅ Operations without allocations
//!     self.spi_read(&mut buffer)?;
//!     let temp = self.convert_temperature(buffer, &mut error_msg)?;
//!
//!     Ok(temp)
//! }
//! ```
//!
//! ### For INITIALIZATION Operations
//!
//! ```rust
//! use alloc::{boxed::Box, string::String};
//!
//! pub fn build_system() -> Result<System, BuildError> {
//!     // ✅ Heap allocations allowed during initialization
//!     let heater: Box<dyn Heater> = Box::new(SSRHeater::new()?);
//!     let config: String = load_configuration()?;
//!
//!     Ok(System { heater, config })
//! }
//! ```
//!
//! ### For MIXED Operations
//!
//! ```rust
//! use heapless::String;
//!
//! pub enum AppError {
//!     /// Error in hot path - use heapless
//!     Temperature {
//!         message: String<128>,  // ✅ heapless for runtime errors
//!         source: TemperatureError,
//!     },
//!     /// Error in initialization - may use alloc
//!     Configuration {
//!         message: alloc::string::String,  // ⚠️ documented: only occurs in init
//!         source: ConfigError,
//!     },
//! }
//! ```
//!
//! ## Verification and Testing
//!
//! ### Memory Tests
//!
//! ```rust
//! #[test]
//! fn test_hot_path_no_allocations() {
//!     // This test must be able to run without any heap allocation
//!     let system = create_test_system();
//!
//!     // Simulate normal operation without allocations
//!     for _ in 0..1000 {
//!         let temp = system.read_temperature().unwrap();
//!         system.update_pid(temp);
//!         system.set_heater_duty(50.0);
//!     }
//! }
//! ```
//!
//! ### Static Linting
//!
//! The project includes custom clippy rules to detect:
//! - Use of `alloc::` in HOT PATH modules
//! - Inconsistent heapless capacities
//! - Missing memory strategy documentation
//!
//! ## Success Metrics
//!
//! 1. **Zero heap allocations in hot paths** during normal operation
//! 2. **Deterministic execution time** (variation ≤ 5%)
//! 3. **Predictable RAM usage** (variation ≤ 10%)
//! 4. **Complete documentation** of all modules according to their category
//! 5. **Tests that verify** memory guarantees
//!
//! ## Maintenance
//!
//! - Every new module must be classified as HOT PATH, INITIALIZATION or MIXED
//! - Changes in HOT PATH modules must be verified to ensure zero allocations
//! - Memory constants must be used consistently
//! - Documentation must be kept up to date with strategy changes

pub use crate::memory::constants;
