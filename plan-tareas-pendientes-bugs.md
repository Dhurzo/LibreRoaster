# Tareas Pendientes y Bugs - LibreRoaster

**Fecha:** 2026-07-17
**Versión:** Basado en análisis de documentación vs código

---

## Resumen de Inconsistencias Detectadas y Corregidas

### 1. ✅ CORREGIDO: Conteo de Tasks (Embassy)

**Problema:** La documentación de ARCHITECTURE.md mencionaba 7 tareas Embassy, pero tras el refactor F5.3 solo hay 5 tareas activas.

**Tareas documentadas (antes):**
- USB reader task
- UART reader task
- USB queue processor task
- UART queue processor task
- control loop task
- dual output task
- regression task

**Tareas reales (código):**
- `usb_reader_task` (lee bytes y parsea comandos)
- `uart_reader_task` (lee bytes y parsea comandos)
- `uart_writer_task` (escribe a UART)
- `control_loop_task` (control principal)
- `regression_task` (regresión sobre-temperatura)

**Fix:** Actualizado ARCHITECTURE.md para reflejar que las queue processor tasks fueron fusionadas en las reader tasks en F5.3.

---

### 2. ✅ CORREGIDO: Campos STATUS (19 vs 20)

**Problema:** ARTISAN_CONNECTION.md decía que STATUS devuelve 19 campos, pero el código devuelve 20.

**Fix:** Corregido a 20 campos.

---

### 3. ✅ CORREGIDO: Rango de temperatura válido

**Problema:** `is_valid_target_temp()` aceptaba 0-300°C, pero el parser y la documentación requieren 50-300°C.

**Fix:** Corregido `is_valid_target_temp()` en `src/config/constants.rs` para requerir 50°C mínimo.

---

### 4. ❌ PENDIENTE: CONTROL_BUG_AUDIT.md no existe

**Problema:** HARDWARE.md referenciaba `BUGS.md` / `CONTROL_BUG_AUDIT.md` que no existe.

**Fix:** Removida la referencia a BUGS.md de HARDWARE.md.

**Nota:** Si en el futuro se necesita un documento de bugs, debería crearse con un proceso de auditoría formal.

---

## Tasks Pendientes de Implementación

### Alta Prioridad

1. **[ ] Verificar que el refactor de tareas no rompió la funcionalidad**
   - Necesario: prueba HIL en hardware real
   - Validar que comandos USB y UART funcionan correctamente

2. **[ ] Crear CONTROL_BUG_AUDIT.md si se requiere documentación de bugs**
   - Definir formato y proceso de auditoría
   - Asignar propietario del documento

### Prioridad Media

3. **[ ] Revisar si hay más inconsistencias documentación-código**
   - Campos en STATUS/READ response
   - Timings documentados vs implementados

4. **[ ] Actualizar TESTING.md si el conteo de tareas cambió**
   - Verificar que las filas de "Task-level instrumentation" siguen siendo válidas

---

## Métricas de Calidad

- **Tests passing (host):** 244/244 (según última ejecución conocida)
- **Embedded build:** Warning-free (según última ejecución conocida)
- **Consistencia docs/código:** 4/5 issues resueltos

---

## Notas

- El usuario pidió no hacer commits de momento hasta tener el plan guardado
- Este documento es el plan de trabajo
- Después de revisar, hacer commit y push