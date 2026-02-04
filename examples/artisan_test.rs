//! Example demonstrating Artisan+ protocol integration
//!
//! This example demonstrates the complete Artisan+ protocol flow:
//! 1. SystemStatus → ArtisanFormatter → Artisan CSV output
//! 2. READ response formatting (ET,BT,Power,Fan)
//! 3. MutableArtisanFormatter for ROR calculation
//!
//! # API Usage
//!
//! ```rust
//! use libreroaster::config::{RoasterState, SystemStatus, SsrHardwareStatus};
//! use libreroaster::output::artisan::ArtisanFormatter;
//! use libreroaster::output::OutputFormatter;
//!
//! // Create a status representing roaster state
//! let status = SystemStatus {
//!     state: RoasterState::Stable,
//!     bean_temp: 150.5,
//!     env_temp: 120.3,
//!     ssr_output: 75.0,
//!     ..Default::default()
//! };
//!
//! // Format as Artisan CSV
//! let formatter = ArtisanFormatter::new();
//! let csv_output = formatter.format(&status).unwrap();
//! // Output: "120.3,150.5,0.00,75.0" (time,ET,BT,ROR,Gas)
//!
//! // READ response format (ET,BT,Power,Fan)
//! let read_response = ArtisanFormatter::format_read_response(&status, 25.0);
//! // Output: "120.3,150.5,75.0,25.0"
//! ```

#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::string::ToString;
use esp_println::println;
use libreroaster::config::{RoasterState, SsrHardwareStatus, SystemStatus};
use libreroaster::output::artisan::{ArtisanFormatter, MutableArtisanFormatter};
use libreroaster::output::OutputFormatter;

/// Main entry point for Artisan+ protocol demonstration
fn main() {
    println!("=== Artisan+ Protocol Integration Demo ===");
    println!();

    // Demonstrate ArtisanFormatter CSV output
    demonstrate_artisan_formatter();

    // Demonstrate READ response formatting
    demonstrate_read_response();

    // Demonstrate ROR calculation with MutableArtisanFormatter
    demonstrate_ror_calculation();

    println!();
    println!("✅ Artisan+ integration demo completed successfully!");
    println!();
    println!("Protocol Format Reference:");
    println!("- Artisan CSV: time,ET,BT,ROR,Gas");
    println!("- READ response: ET,BT,Power,Fan");
    println!("- OT1 x: Set heater power (0-100%)");
    println!("- IO3 x: Set fan speed (0-100%)");
}

/// Demonstrates standard Artisan CSV formatting
fn demonstrate_artisan_formatter() {
    println!("1. ArtisanFormatter CSV Output:");

    // Create test status
    let status = SystemStatus {
        state: RoasterState::Stable,
        bean_temp: 150.5,
        env_temp: 120.3,
        target_temp: 200.0,
        ssr_output: 75.0, // SSR as Gas control
        fan_output: 50.0,
        pid_enabled: true,
        artisan_control: false,
        fault_condition: false,
        ssr_hardware_status: SsrHardwareStatus::Available,
    };

    // Format as Artisan CSV (implements OutputFormatter trait)
    let formatter = ArtisanFormatter::new();

    match formatter.format(&status) {
        Ok(csv) => {
            println!("   Output: {}", csv);
            println!("   Format: time,ET,BT,ROR,Gas");
        }
        Err(e) => {
            println!("   Error: {:?}", e);
        }
    }
}

/// Demonstrates READ response formatting
fn demonstrate_read_response() {
    println!();
    println!("2. READ Response Formatting:");

    let status = SystemStatus {
        state: RoasterState::Stable,
        bean_temp: 150.5,
        env_temp: 120.3,
        ssr_output: 75.0,
        fan_output: 25.0, // Fan speed for READ response
        ..Default::default()
    };

    // READ response: ET,BT,Power,Fan
    let response = ArtisanFormatter::format_read_response(&status, 25.0);
    println!("   Response: {}", response);
    println!("   Format: ET,BT,Power,Fan");
}

/// Demonstrates ROR (Rate of Rise) calculation
fn demonstrate_ror_calculation() {
    println!();
    println!("3. MutableArtisanFormatter ROR Calculation:");

    let mut formatter = MutableArtisanFormatter::new();

    // First reading - ROR should be 0.0 (no history)
    let status1 = SystemStatus {
        bean_temp: 100.0,
        env_temp: 120.0,
        ssr_output: 50.0,
        ..Default::default()
    };

    match formatter.format(&status1) {
        Ok(output) => {
            println!("   Reading 1 (BT=100.0): {}", output);
        }
        Err(e) => {
            println!("   Error: {:?}", e);
        }
    }

    // Second reading - ROR shows delta
    let status2 = SystemStatus {
        bean_temp: 102.0,
        env_temp: 121.0,
        ssr_output: 55.0,
        ..Default::default()
    };

    match formatter.format(&status2) {
        Ok(output) => {
            println!("   Reading 2 (BT=102.0): {}", output);
            println!("   ROR: ~2.0°C/s (102 - 100)");
        }
        Err(e) => {
            println!("   Error: {:?}", e);
        }
    }

    // Third reading - more history
    let status3 = SystemStatus {
        bean_temp: 106.0,
        env_temp: 122.0,
        ssr_output: 60.0,
        ..Default::default()
    };

    match formatter.format(&status3) {
        Ok(output) => {
            println!("   Reading 3 (BT=106.0): {}", output);
        }
        Err(e) => {
            println!("   Error: {:?}", e);
        }
    }
}
