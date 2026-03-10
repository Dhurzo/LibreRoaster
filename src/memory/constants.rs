//! Constantes de memoria para LibreRoaster
//!
//! Este módulo define los tamaños estándar para buffers heapless,
//! garantizando consistencia y predecibilidad en el uso de memoria.

/// Tamaño máximo para mensajes de error en hot paths
///
/// Usado para errores que pueden ocurrir durante operación normal
/// en paths críticos donde no se permite heap allocation.
pub const ERROR_MSG_MAX_LEN: usize = 128;

/// Tamaño máximo para comandos Artisan en hot paths
///
/// Usado para formateo de comandos y respuestas Artisan
/// en el path de comunicación en tiempo real.
pub const ARTISAN_CMD_MAX_LEN: usize = 64;

/// Tamaño para buffers de reporte de temperatura
///
/// Usado para formateo de datos de temperatura que se envían
/// a Artisan o otros sistemas de monitoreo.
pub const REPORT_BUFFER_SIZE: usize = 32;

/// Tamaño para historial de temperatura BT (Bean Temperature)
///
/// Usado para tracking de temperatura en el módulo Artisan
/// para cálculos de derivadas y tendencias.
pub const BT_HISTORY_SIZE: usize = 5;

/// Tamaño máximo para nombres de etapa o estados
///
/// Usado para identificar estados actuales de la tostadora
/// en reportes y logs.
pub const STAGE_NAME_MAX_LEN: usize = 16;

/// Tamaño máximo para mensajes de estado del sistema
///
/// Usado para reportes periódicos del estado del sistema
/// que no son errores críticos.
pub const STATUS_MSG_MAX_LEN: usize = 64;

/// Tamaño para buffers de comandos UART/USB
///
/// Usado para procesamiento de comandos recibidos vía
/// comunicación serial o USB.
pub const COMMAND_BUFFER_SIZE: usize = 256;

/// Tamaño para buffers de respuesta UART/USB
///
/// Usado para construir respuestas a comandos
/// sin allocation dinámica.
pub const RESPONSE_BUFFER_SIZE: usize = 512;

/// Tamaño máximo para mensajes de política de control
///
/// Usado en módulos de políticas donde los mensajes
/// pueden generarse durante inicialización.
pub const POLICY_MSG_MAX_LEN: usize = 96;

/// Tamaño para buffers de parseo de comandos
///
/// Usado durante el parseo de comandos Artisan
/// para mantener tokens y parámetros.
pub const PARSE_TOKENS_MAX: usize = 8;

/// Tamaño máximo para valores de parámetros
///
/// Usado para almacenamiento temporal de parámetros
/// durante el parseo y procesamiento de comandos.
pub const PARAM_VALUE_MAX_LEN: usize = 32;

/// Tamaño para buffers de instrumentación
///
/// Usado para recolectar métricas y datos de
/// instrumentación del sistema.
pub const INSTRUMENT_BUFFER_SIZE: usize = 128;

/// Tamaño máximo para nombres de perfiles de tostado
///
/// Usado durante inicialización para almacenar
/// nombres de perfiles de configuración.
pub const PROFILE_NAME_MAX_LEN: usize = 32;

/// Capacidad máxima para cola de eventos de seguridad
///
/// Usado para manejar eventos de seguridad sin
/// allocations en tiempo real.
pub const SAFETY_EVENT_QUEUE_SIZE: usize = 16;

/// Tamaño para buffers de logging en tiempo real
///
/// Usado para formateo de mensajes de log que
/// pueden ocurrir en cualquier momento.
pub const LOG_MSG_MAX_LEN: usize = 96;

/// Tamaño máximo para mensajes de diagnóstico
///
/// Usado para reportes de diagnóstico del sistema
/// que pueden incluir información detallada.
pub const DIAGNOSTIC_MSG_MAX_LEN: usize = 256;

/// Tamaño para buffers de calibración
///
/// Usado durante operaciones de calibración de
/// sensores y sistemas de control.
pub const CALIBRATION_BUFFER_SIZE: usize = 64;

/// Tamaño para formateo de tiempo
///
/// Usado para formateo de timestamps en segundos y milisegundos
/// para protocolos como Artisan.
pub const TIME_FORMAT_SIZE: usize = 8;

/// Tamaño para mensajes de error en módulos de seguridad
///
/// Usado para mensajes de error críticos que pueden
/// ocurrir durante operaciones de seguridad.
pub const SAFETY_ERROR_MSG_MAX_LEN: usize = 128;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_sanity() {
        // Verificar que las constantes sean razonables
        assert!(ERROR_MSG_MAX_LEN > 0);
        assert!(ERROR_MSG_MAX_LEN <= 1024); // No demasiado grande

        assert!(ARTISAN_CMD_MAX_LEN > 0);
        assert!(ARTISAN_CMD_MAX_LEN <= 256);

        assert!(REPORT_BUFFER_SIZE > 0);
        assert!(REPORT_BUFFER_SIZE <= 128);

        assert!(BT_HISTORY_SIZE > 0);
        assert!(BT_HISTORY_SIZE <= 32); // Historial razonable

        assert!(COMMAND_BUFFER_SIZE >= RESPONSE_BUFFER_SIZE / 2);
        assert!(RESPONSE_BUFFER_SIZE <= 1024); // Buffer de respuesta manejable

        // Verificar que los tamaños sean potencias de 2 o múltiplos comúnmente usados
        assert!(ERROR_MSG_MAX_LEN % 8 == 0 || ERROR_MSG_MAX_LEN == 128);
        assert!(ARTISAN_CMD_MAX_LEN % 8 == 0 || ARTISAN_CMD_MAX_LEN == 64);
    }
}
