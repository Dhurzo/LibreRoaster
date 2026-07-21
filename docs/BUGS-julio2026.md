# LibreRoaster — Bug Report

**Fecha:** 18 julio 2026
**Alcance:** Código explorado en la sesión de revisión: `src/output/artisan.rs`, `src/input/parser.rs`, `src/control/roaster_control.rs`, `src/control/handlers/artisan.rs`, `src/control/pid.rs`, `src/control/controllers/{dispatch,safety,sensor,actuator}.rs`, `src/config/constants.rs`, `src/control/abstractions.rs`, `src/application/tasks.rs`, `src/hardware/transport_tasks.rs`, `src/hardware/uart/tasks.rs`, `src/hardware/usb_cdc/tasks.rs`, `src/safety/watchdog.rs`, `src/safety/regression.rs`, `src/input/multiplexer.rs`.
**No explorado:** `src/hardware/ssr.rs`, `src/hardware/fan.rs`, `src/hardware/ledc_guard.rs`, `src/hardware/max31856.rs`, `src/hardware/sensors/conversion.rs`, `src/control/traits.rs`, `src/control/policies.rs`, `src/application/app_builder.rs`, `src/application/service_container.rs`, `src/logging/traceability.rs`, `src/logging/roast_logger.rs`, `src/memory/*`.

---

## Estado de Corrección (actualizado 21 julio 2026)

Los **12 bugs** listados a continuación fueron verificados reales contra el código y **corregidos**. Resumen de fixes:

| # | Archivo(s) | Fix |
|---|---|---|
| #1 | `transport_tasks.rs` | `handle_parsed_command` ahora emite `ERR channel_full command_dropped` (multiplexer-aware) cuando `try_send` falla, en vez de solo `debug!()`. |
| #2 | `transport_tasks.rs`, `parser.rs` | `push_to_event_queue` ahora hace `queue.clear()` (flush del comando parcial) en overflow en vez de `pop_front()` de un solo byte. Añadido `ParseError::BufferOverflow` y `ERR buffer_overflow` cuando el overflow se detecta al recibir un terminador. |
| #3 | `safety/regression.rs` | `run_once` captura `shutdown_failed`; si `emergency_shutdown` falla, emite `SAFETY OT-REGRESSION-ABORTED shutdown_failed`, limpia `overtemp_regression_active`, y retorna temprano sin replayear fixtures. |
| #4 | `safety/regression.rs` | Ambos `keep_feeding_watchdog` cambiados de 500ms a 400ms para dejar margen contra `WATCHDOG_TIMEOUT_MS=500`. |
| #5 | `control/pid.rs` | Añadido flag `last_error_initialized`; en el primer tick post-`enable()` el derivative es 0 (no `(error - 0) / dt`), eliminando el spike de ~85% output. |
| #6 | `output/artisan.rs` | `MutableArtisanFormatter` usa flag `is_initialised` en vez de comparar `last_bt == 0.0`. BT legítimo de 0°C ya no resetea ROR. |
| #7 | `output/artisan.rs` | En `current_bt == last_bt`, ahora se llama `update_bt_history_with_timestamp` antes de retornar 0.0, para que la base de tiempo avance. |
| #8 | `control/handlers/artisan.rs` | `IncreaseHeater`/`DecreaseHeater` en `evaluate()` usan `self.manual_heater` como baseline en vez de `status.ssr_output`. |
| #9 | `control/handlers/artisan.rs` | Eliminada por completo la impl `RoasterCommandHandler for ArtisanCommandHandler` (dead code: nunca se invocaba desde `dispatch.rs:37-45`). |
| #10 | `control/handlers/artisan.rs` | Resuelto junto con #9 al eliminar `handle_command` duplicado de `evaluate`. |
| #11 | `application/tasks.rs` | Log cambiado de "Service container error in control loop" a "Sensor read error in control loop" (el error viene de `roaster_async_sensor_read`, no del ServiceContainer). |
| #12 | `CONTEXT.md`, `README.md` | Actualizado de 7 a 5 tasks; removidos USB queue processor y UART queue processor; añadida nota "F5.3 refactor note". Complementa el fix parcial que solo había tocado `ARCHITECTURE.md`. |

**Validación:** `cargo fmt --check` OK · `cargo clippy --all-targets --features test` OK (solo warning pre-existente `tick_at` dead_code en `tests/critical_path_tests.rs:47`) · `cargo test --features test` → **579/579 tests pasan** · `cargo build --release --target riscv32imc-unknown-none-elf --features embedded` limpio sin warnings · `cargo clippy --release --target riscv32imc --features embedded` limpio.

**Test actualizado:** `tests/critical_path_tests.rs::up_command_increments_heater` bendaba el Bug #8 (asumía baseline = `ssr_output`); reescrito para testear `manual_heater` como baseline.

---

## 🔴 Críticos — Seguridad

### Bug #1 — Silent Command Drop on Channel Full ✅ CORREGIDO
**Archivo:** `src/hardware/transport_tasks.rs:210-212`
**También en:** `src/hardware/uart/tasks.rs:128-130`, `src/hardware/usb_cdc/tasks.rs:103-105`

**Descripción:** Cuando el canal del artisan está lleno (`try_send` falla), el comando se descarta silenciosamente con solo un `debug!()`. No se envía ninguna respuesta `ERR` al host. Artisan puede seguir enviando comandos que se pierden sin notificación.

**Código afectado:**
```rust
// transport_tasks.rs:205-212
match artisan_channel.try_send(traced) {
    Ok(()) => { /* ... */ }
    Err(_) => {
        debug!("{} artisan channel full, command dropped", config.name);
        // ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
        // Sin respuesta ERR al host
    }
}
```

**Impacto:** Comandos de control perdidos (START, STOP, OT1, etc.) sin que Artisan lo sepa. Puede dejar el roaster en estado inesperado.
**Recomendación:** Enviar `ERR channel_full` al host cuando `try_send` falla, o implementar backpressure.

---

### Bug #2 — Silent Byte Drop on Event Queue Overflow ✅ CORREGIDO
**Archivo:** `src/hardware/transport_tasks.rs:132-134`

**Descripción:** Cuando la cola de eventos (256 bytes) se llena, el byte más viejo se descarta con `pop_front()` para hacer espacio. Si ese byte descartado era parte de un comando parcial en curso (ej. "SETTAR" sin el "GET"), el comando restante se corrompe.

**Código afectado:**
```rust
// transport_tasks.rs:131-136
for &byte in data {
    if queue.len() >= EVENT_QUEUE_SIZE {
        let _ = queue.pop_front(); // ← byte más viejo destruido
    }
    let _ = queue.push_back(byte);
}
```

**Impacto:** Corrupción silenciosa de comandos. Puede causar comandos inesperados o errores de parseo.
**Recomendación:** Descartar el comando incompleto entero (flush) en vez de un byte, o rechazar el comando entrante con `ERR buffer_overflow`.

---

### Bug #3 — `emergency_shutdown` Failure Ignored in Regression ✅ CORREGIDO
**Archivo:** `src/safety/regression.rs:65`

**Descripción:** En `run_once()`, el resultado de `emergency_shutdown` se ignora (solo `warn!`). Si el apagado de emergencia falla, la regresión continúa con heater y fan al 100%.

**Código afectado:**
```rust
// regression.rs:65-67
if let Err(err) = roaster.emergency_shutdown("Over-temp regression") {
    warn!("Regression shutdown failed: {:?}", err);
    // ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    // El error se ignora; la regresión continúa
}
```

**Impacto:** Durante regression test, si el shutdown falla, el heater queda al 100% sin supervisión.
**Recomendación:** Si `emergency_shutdown` falla, el regression test debe abortar inmediatamente.

---

### Bug #4 — Watchdog Feed Duration Equals Timeout Threshold ✅ CORREGIDO
**Archivo:** `src/safety/regression.rs:72`

**Descripción:** `keep_feeding_watchdog(Duration::from_millis(500))` con `WATCHDOG_TIMEOUT_MS = 500`. La duración del feeding exactamente igual al timeout no deja margen para jitter del scheduler.

**Código afectado:**
```rust
// regression.rs:72
self.keep_feeding_watchdog(Duration::from_millis(500)).await;

// watchdog.rs:40
const WATCHDOG_TIMEOUT_MS: u64 = 500; // 500ms
```

`keep_feeding_watchdog` alimenta cada 100ms (`WATCHDOG_FEED_INTERVAL_MS`), pero la duración total de 500ms junto con cualquier jitter del Timer puede causar que un feed llegue exactamente al límite del timeout.

**Impacto:** Durante regression, el watchdog podría expirar causando reset del sistema.
**Recomendación:** Usar una duración menor (ej. 400ms) o reducir el interval de feed a 80ms.

---

## 🟠 Lógicos / Correctitud

### Bug #5 — PID Derivative Kick on First Tick After Enable ✅ CORREGIDO
**Archivo:** `src/control/pid.rs:178`

**Descripción:** En el primer tick después de `enable()`, `last_error` está en 0.0, produciendo un spike derivativo masivo.

**Cálculo:**
- `error = target - current_temp` (ej. 200 - 30 = 170)
- `last_error = 0.0` (inicializado en `enable()`)
- `dt = 0.1s` (ciclo de 100ms)
- `derivative = (error - 0) / 0.1 = 170 / 0.1 = 1700 °C/s`
- `kd = 0.05` → `kd * derivative = 0.05 * 1700 = 85%` de output solo del término derivativo

**Código afectado:**
```rust
// pid.rs:177-183
let derivative = if dt > 0.0 {
    let derivative = (error - self.last_error) / dt;
    //                                ^^^^^^^^^^^^ 0.0 en primer tick
    self.derivative_rate = derivative;
    derivative
} else {
    self.derivative_rate
};
```

**Impacto:** Overshoot inicial de heater en el primer tick de PID. No es crítico para seguridad (el integrator lo corrige después) pero causa comportamiento transitorio undesired.
**Recomendación:** En `enable()`, inicializar `last_error = error` en vez de 0.0, o usar `derivative_on_measurement` (medir derivative sobre la temperatura en vez del error).

---

### Bug #6 — ROR State False Reset When BT Reads 0°C ✅ CORREGIDO
**Archivo:** `src/output/artisan.rs:388`

**Descripción:** `if self.last_bt == 0.0` trata cualquier lectura de BT = 0.0°C como "primera muestra", reseteando el estado ROR. Si un sensor reporta legitimately 0°C (ambiente frío, fallback del MAX31856), el ROR se corrompe.

**Código afectado:**
```rust
// artisan.rs:387-397
fn calculate_ror(&mut self, current_bt: f32, now: Instant) -> f32 {
    if self.last_bt == 0.0 {
        self.last_bt = current_bt;
        Self::update_bt_history_with_timestamp(...);
        return 0.0;
    }
```

**Impacto:** ROR incorrecto durante condiciones de frío. No afecta seguridad (la temperatura 0°C es validada y trigger emergency si es spurious).
**Recomendación:** Usar un flag `is_initialized: bool` en vez de comparar `last_bt == 0.0`.

---

### Bug #7 — Stale ROR on Equal BT Samples ✅ CORREGIDO
**Archivo:** `src/output/artisan.rs:399-401`

**Descripción:** Cuando `current_bt == last_bt`, se retorna `0.0` sin llamar `update_bt_history_with_timestamp`. El historial pierde un punto y el ROR en siguientes ticks se calcula con datos rancios.

**Código afectado:**
```rust
// artisan.rs:399-402
if current_bt == self.last_bt {
    self.last_bt = current_bt;
    return 0.0; // ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ sin update_bt_history
}
```

**Impacto:** ROR se vuelve estático aunque las temperaturas previas indicaran una tendencia. Pérdida de granularidad.
**Recomendación:** Llamar `update_bt_history_with_timestamp` incluso cuando `current_bt == last_bt`.

---

### Bug #8 — UP/DOWN Heater Delta Uses Wrong Baseline ✅ CORREGIDO
**Archivo:** `src/control/handlers/artisan.rs:147-148, 160-161, 248-249, 260-261`

**Descripción:** `IncreaseHeater`/`DecreaseHeater` usan `status.ssr_output` como baseline para calcular el nuevo valor. Si PID o safety cambió `ssr_output` sin actualizar `manual_heater`, los deltas de UP/DOWN se calculan desde el valor incorrecto.

**Código afectado:**
```rust
// handlers/artisan.rs:143-154 (handle_command)
RoasterCommand::IncreaseHeater => {
    status.artisan_control = true;
    status.pid_enabled = false;
    let current = status.ssr_output;  // ← baseline = ssr_output
    let new_value = Self::apply_heater_delta(current, 1);
    status.ssr_output = new_value;
    self.manual_heater = new_value;
}

// handlers/artisan.rs:247-257 (evaluate) — mismo problema
RoasterCommand::IncreaseHeater => {
    let current = status.ssr_output;  // ← baseline = ssr_output
```

**Escenario:** Usuario setea heater 50% via OT1 (`manual_heater=50`, `ssr_output=50`). PID later computes `ssr_output=75` (para mantener temperatura). Usuario presiona UP. El delta se calcula desde 75, no desde 50. El heater salta a 80% (75+5) en vez de 55% (50+5).

**Impacto:** Comportamiento inesperado para el operador al usar UP/DOWN con PID activo.
**Recomendación:** Usar `self.manual_heater` como baseline, o sincronizar `manual_heater` con `ssr_output` cuando se establece manual mode.

---

## 🟡 Código Muerto / Duplicación

### Bug #9 — Dead `RoasterCommandHandler::handle_command` for ArtisanCommandHandler ✅ CORREGIDO
**Archivo:** `src/control/handlers/artisan.rs:83-200`

**Descripción:** `ArtisanCommandHandler` implementa `RoasterCommandHandler::handle_command()` pero **nunca se invoca**. En `dispatch.rs:37-38`:

```rust
let mut handlers: [&mut dyn RoasterCommandHandler; 2] =
    [&mut self.temp_handler, &mut self.system_handler];
//              ^^^^^^^^^^^^^^^^^  ^^^^^^^^^^^^^^^^^
//              artisan_handler NO está aquí
```

El path real para comandos artisan es `evaluate_manual_policy()` → `artisan_handler.evaluate()`.

**Impacto:** El trait impl de `handle_command` es dead code. Si alguien añade un call site que use el trait, divergirá del comportamiento de `evaluate`.
**Recomendación:** Eliminar `handle_command` de `ArtisanCommandHandler` o moverlo a un lugar donde sí se use.

---

### Bug #10 — Duplicated Logic in ArtisanCommandHandler ✅ CORREGIDO
**Archivo:** `src/control/handlers/artisan.rs:103-187` y `213-294`

**Descripción:** `handle_command` y `evaluate` implementan lógica idéntica para `SetHeaterManual`, `SetFanManual`, `IncreaseHeater`, `DecreaseHeater`, `SetUnits`. Ambas funciones hacen lo mismo.

**Impacto:** Duplicación de código. Si se corrige un bug en una función pero no en la otra, se introduce divergencia.
**Recomendación:** Eliminar `handle_command` (ver Bug #9) o refactorizar para compartir la lógica.

---

## 🟢 Cosméticos / Doc Drift

### Bug #11 — Misleading Log Message ✅ CORREGIDO
**Archivo:** `src/application/tasks.rs:901-903`

**Descripción:** El log dice "Service container error in control loop" pero la variable es `sensor_err`, que viene de `read_sensors()`, no de un service container error.

```rust
// tasks.rs:901-903
if let Some(e) = tick_state.sensor_err.take() {
    info!("Service container error in control loop: {:?}", e);
    //     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    //     sensor_err = error de lectura de sensor, no de service container
}
```

**Impacto:** Confusión durante debugging. No funcional.
**Recomendación:** Cambiar a `info!("Sensor read error in control loop: {:?}", e)`.

---

### Bug #12 — CONTEXT.md Claims 7 Tasks (Actual: 5) ✅ CORREGIDO
**Archivo:** `docs/CONTEXT.md` (descripción de tareas), `README.md`
**Ref:** `src/application/app_builder.rs:start_tasks()`

**Descripción:** CONTEXT.md dice que hay 7 tareas incluyendo "USB queue processor" y "UART queue processor". El código spawnea 5 tareas:

1. `uart_reader_task`
2. `usb_reader_task`
3. `control_loop_task`
4. `dual_output_task`
5. `regression_task`

Las tareas "queue processor" fueron eliminadas en F5.3 (el comentario en `transport_tasks.rs:9-12` lo confirma: "F5.3: Command path simplified — intermediate command_queue and run_queue_processor_task have been removed").

**Impacto:** Documentación stale. ARCHITECTURE.md §4 es correcto.
**Recomendación:** Actualizar CONTEXT.md para reflejar 5 tareas y la simplificación F5.3.

---

## Bugs Descartados (No-bugs)

Los siguientesitems fueroninvestigadosperose descartaron como bugs:

- **Fan profile end-of-life**: `FanProfile::target_at()` retorna `Some(last)` después del último setpoint. El `unwrap_or(20.0)` en `roaster_control.rs:501` es inalcanzable porque `target_at` solo retorna `None` cuando `setpoints.is_empty()`, lo cual no puede ocurrir si el perfil fue cargado exitosamente.
- **STATUS duplicate response**: Confirmado que NO hay bug. `handle_status_report` NO emite; solo `drain_commands` en `tasks.rs` emite después de `Ok(())`. Modelo de single-emission correcto.
- **PID `bound_to_actuator` redundant clamp**: El segundo `clamp` en `bound_to_actuator` es intencional (comentado) y no es bug.
- **Watchdog `as_millis()` wrap**: u64 wrap en ~584 años, no es problema práctico.

---

## Gaps de Testing Identificados

1. **watchdog.rs**: No hay test que verifique que `feed_async` retorna `Err` cuando pasan >500ms entre feeds.
2. **transport_tasks.rs**: No hay test de overflow del event queue ni de full artisan channel.
3. **regression.rs**: `emit_status_line` hace `warn!` en mismatch pero no falla el test (feature-gated `regression`, aceptable como diagnóstico).

---

*Generado: 2026-07-18 — Sesión de revisión de código*
*Corregido: 2026-07-21 — Los 12 bugs marcados ✅ CORREGIDO han sido fixados y verificados (579/579 tests pasan, embedded build limpio).*