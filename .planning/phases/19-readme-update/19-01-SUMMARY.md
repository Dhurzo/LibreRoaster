---
files_modified:
  - README.md
---

## Resumen

Actualizado README.md con información precisa de LibreRoaster v2.0.

## Cambios Realizados

### Secciones Nuevas
- **Title + Description**: LibreRoaster v2.0 - Artisan-Compatible Firmware
- **Core Value**: Artisan serial connection statement
- **Supported Commands**: Tabla con READ, OT1, IO3, UP, DOWN, START, STOP
- **Quick Start**: 5 pasos (deps, build, flash, connect)
- **Hardware Requirements**: Tabla de componentes
- **Pinout**: GPIO assignments table
- **Artisan Connection**: USB CDC + UART0 dual-channel
- **Protocol**: READ response format con ejemplo
- **Status**: v2.0 complete

### Secciones Eliminadas
- PID control (no implementado)
- Roast profiles (no implementado)
- WiFi/Web UI (no implementado)
- Philosophy redundante
- Safety Warning extenso (simplificado)

### Secciones Mantenidas
- Project Structure
- Development (build/flash commands)
- Debugging
- License

## Verificación

- Proyecto compila: ✅
- Sin referencias a PID/profiles/WiFi: ✅
- Comandos Artisan documentados: ✅
- Quick Start presente: ✅
- Hardware Requirements presente: ✅
- Pinout incluido: ✅
