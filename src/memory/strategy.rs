//! # Estrategia de Memoria LibreRoaster
//!
//! Este documento define la estrategia unificada de gestión de memoria para LibreRoaster,
//! enfocada en predecibilidad de RAM y rendimiento en tiempo real.
//!
//! ## Filosofía de Diseño
//!
//! LibreRoaster utiliza una **estrategia dual de memoria**:
//! - **HOT PATH**: Exclusivamente heapless/stack para operaciones críticas en tiempo real
//! - **INITIALIZATION**: Heap permitido solo durante inicialización y configuración
//!
//! Esta dualidad garantiza que las operaciones críticas tengan tiempos determinísticos
//! mientras permite flexibilidad donde no impacta el rendimiento en tiempo real.
//!
//! ## Clasificación de Módulos
//!
//! ### HOT PATH Modules (heapless únicamente)
//!
//! Estos módulos operan en paths críticos de tiempo real y **NO** deben realizar
//! ninguna allocation dinámica durante su ejecución.
//!
//! ```rust
//! // ✅ Permitido en HOT PATH
//! heapless::Vec<u8, 64>
//! heapless::String<32>
//! stack arrays: [u8; 32]
//! primitives: f32, u32, bool
//!
//! // ❌ Prohibido en HOT PATH
//! alloc::vec::Vec
//! alloc::string::String
//! alloc::boxed::Box
//! cualquier allocation dinámica
//! ```
//!
//! **Módulos HOT PATH identificados:**
//! - `hardware/max31856/` - Lectura de temperatura vía SPI
//! - `control/pid/` - Cálculos de control PID
//! - `hardware/ssr/` - Control PWM de SSR
//! - `hardware/ledc_*` - Generación de señales PWM
//! - `output/artisan/` - Formateo de salida para Artisan
//!
//! **Características de HOT PATH:**
//! - Tiempos de ejecución determinísticos (±5%)
//! - Cero allocations dinámicas durante operación normal
//! - Uso de memoria predecible
//! - Sin dependencia del heap allocator
//!
//! ### INITIALIZATION Modules (heap permitido)
//!
//! Estos módulos operan durante la inicialización del sistema o en paths no críticos
//! donde las allocations no impactan el rendimiento en tiempo real.
//!
//! ```rust
//! // ✅ Permitido en INITIALIZATION
//! alloc::boxed::Box<dyn Trait>  // para dynamic dispatch
//! alloc::string::String         // para mensajes de configuración
//! alloc::vec::Vec               // para construcción inicial
//!
//! // ⚠️ Usar con justificación documentada
//! cualquier allocation que pueda afectar tiempo real
//! ```
//!
//! **Módulos INITIALIZATION identificados:**
//! - `application/app_builder/` - Construcción de la aplicación
//! - `control/policies/` - Políticas de control (evaluación no crítica)
//! - `application/tasks/` - Creación de tareas asíncronas
//! - `hardware/uart/` - Inicialización de drivers UART
//!
//! **Reglas para INITIALIZATION:**
//! - Todas las allocations deben ocurrir antes del loop principal
//! - Documentar cualquier allocation que pueda impactar paths críticos
//! - Preferir pre-allocation sobre allocation dinámica
//!
//! ### MIXED Modules (documentar cuidadosamente)
//!
//! Estos módulos pueden tener operaciones tanto en hot paths como en initialization,
//! requiriendo un diseño cuidadoso y documentación explícita.
//!
//! **Módulos MIXED identificados:**
//! - `error/app_error.rs` - Manejo de errores (puede ocurrir en cualquier momento)
//! - `input/parser.rs` - Parseo de comandos (init + runtime)
//! - `application/stage_instrumentation.rs` - Instrumentación (reporting periódico)
//!
//! **Reglas para MIXED:**
//! - Separar claramente operations de hot path vs initialization
//! - Documentar cada función con su categoría de memoria
//! - Usar heapless para operaciones que puedan ocurrir en runtime
//!
//! ## Constantes de Memoria
//!
//! Para garantizar consistencia en los tamaños de los buffers heapless, se definen
//! constantes estándar en `memory::constants`.
//!
//! ```rust
//! /// Tamaño máximo para mensajes de error en hot paths
//! pub const ERROR_MSG_MAX_LEN: usize = 128;
//!
//! /// Tamaño máximo para comandos Artisan
//! pub const ARTISAN_CMD_MAX_LEN: usize = 64;
//!
// Tamaño para buffers de reporte de temperatura
// pub const REPORT_BUFFER_SIZE: usize = 32;
// ```
//
// ## Patrones Recomendados
//
// ### Para HOT PATH Operations
//
// ```rust
// use heapless::{String, Vec};
//
// pub fn read_temperature(&mut self) -> Result<f32, TemperatureError> {
//     // ✅ Usar buffers con tamaño fijo
//     let mut buffer: [u8; 3] = [0; 3];
//     let mut error_msg: String<32> = String::new();
//
//     // ✅ Operaciones sin allocations
//     self.spi_read(&mut buffer)?;
//     let temp = self.convert_temperature(buffer, &mut error_msg)?;
//
//     Ok(temp)
// }
// ```
//
// ### Para INITIALIZATION Operations
//
// ```rust
// use alloc::{boxed::Box, string::String};
//
// pub fn build_system() -> Result<System, BuildError> {
//     // ✅ Heap allocations permitidas durante inicialización
//     let heater: Box<dyn Heater> = Box::new(SSRHeater::new()?);
//     let config: String = load_configuration()?;
//
//     Ok(System { heater, config })
// }
// ```
//
// ### Para MIXED Operations
//
// ```rust
// use heapless::String;
//
// pub enum AppError {
//     /// Error en hot path - usa heapless
//     Temperature {
//         message: String<128>,  // ✅ heapless para errores en runtime
//         source: TemperatureError,
//     },
//     /// Error en initialization - puede usar alloc
//     Configuration {
//         message: alloc::string::String,  // ⚠️ documentado: solo ocurre en init
//         source: ConfigError,
//     },
// }
// ```
//
// ## Verificación y Testing
//
// ### Tests de Memoria
//
// ```rust
// #[test]
// fn test_hot_path_no_allocations() {
//     // Este test debe poder ejecutarse sin ningún heap allocation
//     let system = create_test_system();
//
//     // Simular operación normal sin allocations
//     for _ in 0..1000 {
//         let temp = system.read_temperature().unwrap();
//         system.update_pid(temp);
//         system.set_heater_duty(50.0);
//     }
// }
// ```
//
// ### Linting Estático
//
// El proyecto incluye reglas de clippy personalizadas para detectar:
// - Uso de `alloc::` en módulos HOT PATH
// - Capacidades heapless inconsistentes
// - Falta de documentación de estrategia de memoria
//
// ## Métricas de Éxito
//
// 1. **Cero heap allocations en hot paths** durante operación normal
// 2. **Tiempo de ejecución determinista** (variación ≤ 5%)
// 3. **Uso de RAM predecible** (variación ≤ 10%)
// 4. **Documentación completa** de todos los módulos según su categoría
// 5. **Tests que verifiquen** garantías de memoria
//
// ## Mantenimiento
//
// - Todo nuevo módulo debe clasificarse como HOT PATH, INITIALIZATION o MIXED
// - Los cambios en módulos HOT PATH deben verificarse para asegurar cero allocations
// - Las constantes de memoria deben usarse consistentemente
// - La documentación debe mantenerse actualizada con los cambios en la estrategia

pub use crate::memory::constants;
