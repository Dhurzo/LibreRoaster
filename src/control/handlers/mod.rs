// Command handlers module for roaster control
//
// This module contains specialized handlers for different types of roaster commands:
// - temperature - Temperature control commands (StartRoast, StopRoast, SetTemperature)
// - safety - Safety commands (EmergencyStop, ArtisanEmergencyStop)
// - artisan - Artisan+ manual commands (SetHeaterManual, SetFanManual, UP, DOWN, SetUnits)
// - system - System commands (Reset)
//
// Each handler is self-contained and testable independently.

pub mod artisan;
pub mod safety;
pub mod system;
pub mod temperature;

// Re-export common handler types and functions for backward compatibility
pub use artisan::ArtisanCommandHandler;
pub use safety::SafetyCommandHandler;
pub use system::SystemCommandHandler;
pub use temperature::TemperatureCommandHandler;
