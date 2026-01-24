// Test simple para verificar compatibilidad Artisan
use crate::config::RoasterState;
use crate::output::artisan::{ArtisanFormatter, SystemStatus};

fn main() {
    let mut formatter = ArtisanFormatter::new();

    // Crear un estado de ejemplo como lo enviaría el tuesteador
    let status = SystemStatus {
        state: RoasterState::Roasting,
        bean_temp: 150.5,
        env_temp: 120.3,
        target_temp: 200.0,
        ssr_output: 75.0,
        fan_output: 50.0,
        pid_enabled: true,
        artisan_control: false,
        fault_condition: false,
    };

    // Formatear para Artisan
    match formatter.format(&status, 25.0) {
        Ok(line) => {
            println!("✅ Artisan Output: {}", line);
            println!("Formato esperado: BT,TEMP_C,TEMP_F,TIME,OT1,OT2,OT3,IO3,SSID");

            // Verificar que contiene los campos esperados
            assert!(line.contains("BT")); // Beacon
            assert!(line.contains("150.5")); // Bean temperature
            assert!(line.contains("120.3")); // Environment temperature
            assert!(line.contains("75.0")); // SSR output

            println!("✅ Compatibilidad Artisan verificada exitosamente!");
        }
        Err(e) => {
            println!("❌ Error: {:?}", e);
        }
    }
}
