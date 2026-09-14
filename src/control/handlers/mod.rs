//! Specialized command handlers for roaster control.
//!
//! One handler per command family — temperature, safety, Artisan manual,
//! system — each self-contained and independently testable.

// Command handlers module for roaster control
//
// This module contains specialized handlers for different types of roaster commands:
// - temperature - Temperature control commands (StartRoast, StopRoast, SetTemperature)
// - safety - Safety commands (EmergencyStop, ArtisanEmergencyStop)
// - artisan - Artisan+ manual commands (SetHeaterManual, SetFanManual, UP, DOWN, SetUnits)
// - system - System commands (Reset)
//
// Each handler is self-contained and testable independently.

/// Artisan+ manual control policy handler.
pub mod artisan;
/// Emergency-stop command handler and latched flag.
pub mod safety;
/// System-level command handler (Reset).
pub mod system;
/// PID and output-manager temperature command handler.
pub mod temperature;

// Re-export common handler types and functions for backward compatibility
/// Artisan manual heater/fan policy handler.
pub use artisan::ArtisanCommandHandler;
/// Emergency-stop command handler.
pub use safety::SafetyCommandHandler;
/// System command handler.
pub use system::SystemCommandHandler;
/// Temperature command handler.
pub use temperature::TemperatureCommandHandler;
