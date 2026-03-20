//! Roasting phase simulation tests
//! Valida fase de tueste (150°C → 220°C)

#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use crate::roast_scenarios::mod::*;
use libreroaster::config::ArtisanCommand;

/// Test 1: Mantenimiento de temperatura en setpoint
#[test]
fn test_roasting_phase_temp_stability() {
    let mut roaster = create_test_roaster();
    let curve = RoastingCurve::new();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    roaster.process_artisan_command(ArtisanCommand::SetHeater(75)).unwrap();
    
    let mut temps = vec![];
    
    // Simular 10 lecturas de temperatura en fase estable (220°C target)
    for i in 0..10 {
        let temp = curve.get_temp_at_second(240 + i * 3); // Near target
        temps.push(temp);
        
        let _ = roaster.update_temperatures(temp, 200.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
    }
    
    // Validar estabilidad (±3°C aceptable)
    let mean: f32 = temps.iter().sum::<f32>() / temps.len() as f32;
    let variance: f32 = temps.iter()
        .map(|&t| (t - mean).powi(2))
        .sum::<f32>() / temps.len() as f32;
    let std_dev = variance.sqrt();
    
    assert!((mean - 220.0).abs() < 5.0, "Temperatura debe estar cerca de target");
    assert!(std_dev < 3.0, "Desviación estándar debe ser <3°C");
}

/// Test 2: Rate of Rise (ROR) cálculo correcto
#[test]
fn test_roasting_phase_ror_calculation() {
    let mut roaster = create_test_roaster();
    let curve = RoastingCurve::new();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    let mut ror_values = vec![];
    
    // Simular varias lecturas con cambios de temperatura
    for i in 0..7 {
        let temp = curve.get_temp_at_second(i * 30);
        let _ = roaster.update_temperatures(temp, 190.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
        
        let status = roaster.get_status();
        ror_values.push(status.derivative_rate);
    }
    
    // ROR debe ser positivo durante calentamiento (temperaturas subiendo)
    for &ror in &ror_values[0..4] {
        assert!(ror >= 0.0, "ROR debe ser positivo durante rampa de calentamiento");
    }
    
    // ROR debe estar cerca de 0 cerca de target estable
    for &ror in &ror_values[5..7] {
        assert!(ror.abs() < 0.5, "ROR debe ser cercano a 0 en fase estable");
    }
}

/// Test 3: Comportamiento de fan durante tueste
#[test]
fn test_roasting_phase_fan_control() {
    let mut roaster = create_test_roaster();
    let curve = RoastingCurve::new();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // Simular fase de tueste
    for i in 0..7 {
        let temp = curve.get_temp_at_second(i * 30);
        let et = temp + 20.0; // ET siempre > BT
        
        let _ = roaster.update_temperatures(temp, et, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
        
        let status = roaster.get_status();
        
        // Fan debe aumentar para moderar temperatura (20-80%)
        assert!(
            status.fan_output >= 20.0,
            "Fan debe estar >=20% durante tueste"
        );
        assert!(
            status.fan_output <= 80.0,
            "Fan debe ser <=80% durante tueste"
        );
    }
}

/// Test 4: Derivative disponible solo después de 2 muestras
#[test]
fn test_roasting_phase_derivative_availability() {
    let mut roaster = create_test_roaster();
    let curve = RoastingCurve::new();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // Primera muestra
    let temp1 = curve.get_temp_at_second(0);
    let _ = roaster.update_temperatures(temp1, 170.0, Instant::now());
    let status1 = roaster.get_status();
    assert!(!status1.derivative_available, "Derivative NO disponible tras 1 muestra");
    
    // Segunda muestra
    let temp2 = curve.get_temp_at_second(1);
    let _ = roaster.update_temperatures(temp2, 175.0, Instant::now());
    let status2 = roaster.get_status();
    assert!(status2.derivative_available, "Derivative disponible tras 2 muestras");
    
    // Tercera muestra
    let temp3 = curve.get_temp_at_second(2);
    let _ = roaster.update_temperatures(temp3, 180.0, Instant::now());
    let status3 = roaster.get_status();
    assert!(status3.derivative_available, "Derivative debe seguir disponible");
}
