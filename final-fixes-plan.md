# Plan de corrección — LibreRoaster v2 audit (sin romper funcionalidad, sin introducir bugs)

**Estrategia**: 7 fases, en orden de prioridad del informe. Cada fase = commit atómico + verificación completa (fmt → clippy → test host → check embebido base → check embebido+regression cuando aplique). Detengo la ejecución entre fases para re-verificar. No toco comportamiento sancionado por tests existentes salvo los que el propio informe identifica como «enshrining broken semantics».

**NO PUSH este archivo nunca** — es trabajo local.

---

## FASE 1 — V2-1 + V2-16a (STOP→brick + RoR guard en preheat vacío) 🔴

**Por qué juntas**: V2-16a amplifica el coste de V2-1 (un falso disparo de emergencia en preheat → brick). Ambas tocan el flujo de emergencia y el latch.

**Cambios**:

1. `roaster_control.rs` `process_artisan_command` rama `ArtisanCommand::Stop` (linea 697): si `self.status.fault_condition && self.safety.is_emergency_active()` → llamar `clear_emergency_explicit()` antes de `handle_stop()`. Da al host una ruta de recuperación alcanzable vía `OFF`. (Accessors confirmados: `safety()` + `is_emergency_active()`.)

2. `roaster_control.rs` `stop_streaming` (linea 312-313): No sobrescribir `state`/`status.state` si `self.safety.is_emergency_active()`. Elimina el «Idle pero latch armado» de B34.

3. `roaster_control.rs` `update_control` line 516 — gatear `check_rate_of_rise` a estados con grano: `if matches!(self.state, Heating | Stable) { check_rate_of_rise(...) }`. Cierra V2-16a sin subir el umbral (mantengo 0.5 °C/s — el informe lo deja como opción «y/o»; el gate por estado es más conservador y reversible).

**Tests a actualizar/añadir**:
- Nuevo: `stop_latch_then_off_recovers` — STOP arma latch, OFF lo limpia, READ vuelve a responder, heater rearable.
- Nuevo: `stop_streaming_does_not_clear_state_while_latched` — state sigue Error tras OFF con latch.
- Nuevo: `ror_guard_skipped_in_preheat_empty_drum` (inyectar derivada >0.5 en state=Preheating, assert no emergency).
- Verificar que tests existentes de `handle_emergency_stop` siguen pasando (no tocan su cuerpo).

**Comprobaciones tras fase**: fmt, clippy host, test host, `cargo check --target riscv32imc --features embedded`.

---

## FASE 2 — V2-2 + V2-3 (hold last value real + debounce por canal) 🟠

**Cambios en `control/controllers/sensor.rs`**:

1. Estructura `SensorController` (linea 21-29): reemplazar `consecutive_fault_count: u8` por **dos**: `consecutive_bean_faults: u8`, `consecutive_env_faults: u8`.

2. `apply_fault_debounce`: recibe `(bean_fault, env_fault)` (cambiar firma) e incrementa/resetea **cada contador por separado**. Latch de `status.fault_condition` cuando cualquiera ≥ DEBOUNCE.
   - Actualizar llamantes: `read_sensors` (linea 46-48) y `roaster_control::update_temperatures_with_fault` (linea 150-152) — pasar `bean_fault.has_fault()` y `env_fault.has_fault()` por separado.

3. `update_temperatures` (linea 101-102): **NO escribir** `status.bean_temp`/`status.env_temp` incondicionalmente. Reescribir como:
   ```rust
   if !bean_fault.has_fault() {
       status.bean_temp = bean_temp + BT_THERMOCOUPLE_OFFSET;
   } else if self.consecutive_bean_faults >= SENSOR_FAULT_DEBOUNCE {
       status.bean_temp = f32::NAN;
   } // else: conserva último valor válido (real)
   ```
   Mismo patrón para `env_temp`. La validación de rango (90-99) y el overtemp (106-115) siguen solo para no-fault. `last_temp_read` se actualiza siempre (hora de lectura válida aunque sea dato retenido).

**Tests**:
- Modificar `update_temperatures_faulted_bt_skips_overtemp`: inyectar un value distinto de basura (ej 999.0) y fijar previa 150.0 → tras faults pre-debounce, status.bean_temp == 150.0 (NO 999.0, NO NaN). Esto corrige la semántica enshrined.
- Modificar `update_temperatures_faulted_et_is_nan` análogamente.
- Añadir: `single_chronic_env_fault_does_not_poison_bean` — ET crónicamente faulted durante 10 ticks, BT un solo glitch → BT no cae en NaN en ese tick (V2-3 escenario).
- Tests de debounce existentes (`fault_debounce_*`) — weren't usando la firma antigua directamente? Revisar; ajustar a nueva firma si compilan.

**Comprobaciones**: fmt, clippy, test host, check embebido.

---

## FASE 3 — V2-4 + V2-16c (START tragado + backstops por heater energizado) 🟠

**Cambios en `roaster_control.rs`**:

1. `handle_start_roast` (linea 744): gate por estado, no por streaming:
   ```rust
   if matches!(self.state, RoasterState::Heating | RoasterState::Stable) {
       info!("START ignored - roast already active");
       ... // rama ignored actual
   } else { ... } // handoff completo para Idle-con-PID, Idle-manual, Preheating
   ```

2. `update_control` lineas 403-406 y 421-424: cambiar el gate `Preheating|Heating|Stable` por «heater energizado o asado activo» — combinar condición física + estado para no perder protección en Preheating. Patrón:
   ```rust
   let heater_energized = self.status.ssr_output > 0.0
       || matches!(self.state, Preheating|Heating|Stable);
   if heater_energized { /* comms-idle check */ }
   ```
   Mismo para max-roast-time. Esto cubre el modo Artisan-manual puro (V2-16c) **sin** desarmar la protección en Preheating (caso que ya funciona).

**Tests**:
- Añadir: `start_after_pid_sv_in_idle_starts_roast` — PID;SV en Idle, START → state=Heating, profile_start_time fijado, logger activo.
- Añadir: `start_after_ot1_in_idle_starts_roast` — OT1 en Idle, START → handoff completo.
- Añadir: `comms_idle_protects_manual_mode_heater_on` — Idle + heater al 80% (vía OT1) + sin comandos > 15s → emergency.
- Añadir: `max_roast_time_protects_manual_mode_heater_on` — análogo.
- Verificar que `start_after_preheat_handoff` y tests de comms-idle en Preheating siguen pasando.

**Comprobaciones**: fmt, clippy, test host, check embebido.

---

## FASE 4 — V2-6 (regression feature compilea + CI) 🟡

**Orden crítico** (descubierto en baseline): el error actual es `void`/std (V2-6.3) que MASKS los demás.

1. `Cargo.toml` linea 34: `regression = []` (sin `embedded-hal-mock`). Deja `embedded-hal-mock = { ... optional = true }` en deps (linea 60) — ya no se activa en el build embebido. (Opcionalmente añadir feature host-only `regression-mock = ["embedded-hal-mock"]` si se quiere para tests host; no es necesario hoy.)

2. Confirmar que `regression` feature se construye solo en riscv32 — unificar: que `regression` **imply** `simulated-sensors` para que `SensorConversionHub::new_uninit()` tome la rama sim (no panic):
   ```toml
   regression = ["simulated-sensors"]
   ```
   (Esto activa la rama `simulated-sensors` de `new_uninit` que llama a `new_simulated(default_curve)` — segura.)

3. `safety/regression.rs` `run_once` closure (linea 84-100): closure devuelve `bool` plano, no `Ok::<bool,_>`:
   ```rust
   let shutdown_failed = ServiceContainer::with_roaster_async(|roaster| {
       ...
       shutdown_result.is_err()   // bool, no Ok()
   }).await;
   let shutdown_failed = shutdown_failed.unwrap_or(true);
   ```
   `with_roaster_async` ya envuelve en `Result<bool, ContainerError>` → `.unwrap_or(true)` ahora opera sobre bool correcto.

4. `safety/regression.rs` `replay_fixture` (linea 145): reemplazar `SensorConversionHub::new()` (E0061) por `SensorConversionHub::from_fixture(fixture.reading)` (ya existe, gated `regression`, usa `new_uninit()` que ahora toma rama sim). Ajustar manejo de error.

5. `safety/regression.rs` tras el loop de fixtures (linea 123-129): si `canonical_fixtures().is_empty()`, emitir `SAFETY OT-REGRESSION-EMPTY no_fixtures` en lugar de `SAFETY OT-REGRESSION`. No mentir éxito.

6. `.github/workflows/ci.yml`: añadir job/step `cargo check --target riscv32imc-unknown-none-elf --features embedded,regression`. (Tercera vez que se rompe sin detección — este job es la palanca.)

**Tests/verificación**: compilar `--features embedded,regression` localmente (la verificación confianza). No hay tests unitarios nuevos (catálogo empty), pero confirmar que el stub non-regression sigue usando su rama y que el host build sin regression sigue verde.

**Comprobaciones**: fmt, clippy host, test host, `cargo check --target riscv32imc --features embedded`, `cargo check --target riscv32imc --features embedded,regression` (debe pasar).

---

## FASE 5 — V2-7 + V2-8 (#DUMP completo: cola, drenaje, truncado, época) 🟡

**Un PR — feature de alto valor de usuario.**

**V2-8 primero** (time base, prereq para que #DUMP tenga sentido):

1. `roast_logger.rs` `RoastLogger` struct (linea 68-71): añadir `start: Option<Instant>` (cambiar const fn `new_empty`). 
2. `start_roast(&mut self, now: Instant)`: `self.start = Some(now); self.active = true; self.buffer.clear();` (ya no descarta `_now`).
3. `log_sample(&mut self, data, now: Instant)`: cambiar firma — compute `elapsed = start.map(|s| now.duration_since(s).as_secs() as u32).unwrap_or(data.elapsed_secs /* fallback */)`; usa ese elapsed, no el del caller. (O mantener `LogSampleData.elapsed_secs` como fallback si start es None — robustez.)
   - Actualizar llamante `tasks.rs:778` para pasar `tick_start`.
4. `tasks.rs`: eliminar `TickState.roast_start` (linea 112) y `mark_continuous_started` (142-146) — la época vive en el logger ahora. Borrar el comentario fantasma API (104-111). Eliminar la llamada `mark_continuous_started` (linea 749).
   - `roaster_control.rs::handle_start_roast` ya llama `roast_logger::start_roast(Instant::now())` (linea 786) — ese es el evento START real. ✅ Correcto.
5. `stop_roast()` del logger: nada que resetear respecto a start (start se re-escribe en próximo start_roast).

**V2-7 (cola, drenaje, truncado)**:

6. `roaster_control.rs` struct (linea 58): `dump_pending: heapless::Deque<heapless::String<256>, { LOG_CAPACITY + 1 }>` — dimensionar al ring. Importar `LOG_CAPACITY` (hacerlo `pub` en `roast_logger.rs` linea 9). Tamaño 257 cubre un asado completo.
7. `roaster_control.rs::handle_dump_log` (linea 1039): empezar con `self.dump_pending.clear();`.
8. `roaster_control.rs::handle_start_roast` (linea 735): `self.dump_pending.clear();` al inicio del handoff (no mezclar dumps previos con asado nuevo).
9. Añadir método `pub fn push_dump_row_front(&mut self, row: heapless::String<256>)` (re-push en fallo de send).
10. `tasks.rs::emit_telemetry_stage` lineas 805-822: **mover el drain FUERA del `should_emit` gate** (drenar cada 100ms, no cada 1s) y re-push en fallo:
    ```rust
    // fuera del gate should_emit, cada tick (100ms), hasta N rows:
    let max_rows_per_tick = 4;
    for _ in 0..max_rows_per_tick {
        let row_opt = ServiceContainer::with_roaster_async(|r| r.take_dump_row()).await;
        if let Ok(Some(row)) = row_opt {
            if output_channel.try_send(row.clone()).is_err() {
                let _ = ServiceContainer::with_roaster_async(|r| r.push_dump_row_front(row)).await;
                break;
            }
        } else { break; }
    }
    ```
    Hasta 4 rows/tick × 10 ticks/s = 40 rows/s → un asado de 4 min (240 rows) se drena en ~6s, mezcladísimamente con telemetría pero sin pérdida.
11. `roast_logger.rs::dump()` (linea 129-143): mantener orden oldest→newest pero seleccionar **newest→oldest qué cabe** (preservar el final del asado, lo más valioso). Implementación: iterar `back.iter().rev()` primero (newest), push al principio de un Vec temporal, luego front, luego emitir en orden cronológico. Alternativa más simple: aumentar `DUMP_BUFFER_SIZE` a `LOG_CAPACITY * (SAMPLE_CAPACITY + 1)` ≈ 256*129 ≈ 33KB — costoso para RAM. Prefiero la selección newest-first (memoria constante). **Decisión**: selección newest-first con buffer de índices (no duplicar strings).

**Tests**:
- Logger: `second_roast_resets_epoch` — start_roast(t0), log, stop, start_roast(t1) → dump time_s empieza en 0.
- Logger: `log_sample_uses_internal_epoch` — llamar log_sample con `data.elapsed_secs=999` pero `start=Some(t)`, `now=t+5s` → row con `5`.
- Logger: `dump_preserves_tail_of_long_roast` — 300 samples, dump → últimos 256 (no los primeros).
- Roaster: `handle_dump_log_clears_previous_dump` — dos #DUMP consecutivos no mezclan.
- Roaster: `start_clears_dump_pending`.
- Tasks (si factible block_on): `dump_drains_outside_should_emit`. Si difícil, testear `push_dump_row_front` round-trip a nivel roaster.

**Comprobaciones**: fmt, clippy, test host, check embebido.

---

## FASE 6 — V2-5 + V2-13 + V2-11 + V2-12 (control/telemetría menor-prioridad) 🟡

1. **V2-5** `roaster_control.rs::handle_preheat` (linea 1094): `self.cooling_active = false;` antes de fijar preheat_target (lote consecutivo sin fan 100%).
2. **V2-13** `roaster_control.rs::stop_streaming` (linea 333): quitar `self.fan_profile = None;` (dejar `profile_start_time = None`).
3. **V2-12** `artisan.rs::calculate_ror` (linea 393): al inicio `if !current_bt.is_finite() { return self.last_filtered_ror; }`.
4. **V2-11** `artisan.rs` lineas 439-442: combinar front+back en una ventana antes del test outlier (como ya hace RoR calc):
   ```rust
   let mut window: heapless::Vec<f32, BT_HISTORY_SIZE> = heapless::Vec::new();
   let (front, back) = self.bt_history.as_slices();
   let _ = window.extend_from_slice(front);
   let _ = window.extend_from_slice(back);
   let is_outlier = ArtisanFormatter::is_temperature_outlier(current_bt, &window);
   ```

**Tests**:
- `preheat_drops_cooling_latch`: STOP (cooling armado) → PREHEAT → cooling_active==false.
- `off_start_preserves_fan_profile`: FANPROFILE, OFF, START → fan_profile intacto.
- `calculate_ror_nan_bt_does_not_poison`: inyectar NaN → ror = last_filtered_ror (no NaN), siguiente finito restaura.
- `outlier_test_uses_combined_window`: rampa lineal, confirmar <30% marcados (vs 70-85% previo).

**Comprobaciones**: fmt, clippy, test host, check embebido.

---

## FASE 7 — V2-9, V2-10, V2-14, V2-15 menores 🟢

1. **V2-9** `host_time_driver.rs::schedule_wake` (literal del informe): thread-per-wake correcto para infraestructura de test.
2. **V2-10** `transport_tasks.rs:328-347` None arm: consumir overflow flag y emitir error.
3. **V2-14** `ledc_bus.rs::set_duty`: convertir %{0..=100} → ticks del canal (misma fórmula del fade) antes de cachear; o eliminar método (preferible eliminar — sin callers de producción, trampa latente).
4. **V2-15**:
   - `transport_tasks.rs:240-243`: mover `record_queue_depth` fuera de `if sent`.
   - `constants.rs`: derivar `CHARGE_SAMPLE_TICK_DIV` de `CHARGE_DETECTION_WINDOW_S` (const eval) en lugar de decorativa.
   - Corregir comentarios falsos (RoastLogger API en tasks.rs — hecho en F5; OCFAULT bits 5:4; «drains one row per tick» → ya drenamos por tick en F5).
   - `PROTOCOL.md`: actualizar rango 50–300 y wire error code post-B9.
   - `formatters/ror.rs`: eliminar (RoR muerto divergente) — verificar sin callers con grep primero.
   - Reset `dump_pending`/`charge_history_tick_div` en stop (cosmético).

**Tests**: para V2-9 (driver host), test que espera un `Timer::after(menos del baseline)` sin actividad concurrente → despierta (no cuelga). Para V2-10, test: cola con solo `\r\n` + overflow.triggered + comando válido siguiente → el válido se procesa (no se descarta).

**Comprobaciones**: fmt, clippy, test host, check embebido + regression.

---

## NO incluimos en este plan (fuera de scope / bajo impacto)
- **V2-16b** (#CHARGE umbral): la simulación del propio informe ya baja el impacto (CHARGE se marca manual en Artisan). Dejar — oportunista si alguien toca el detector.
- **B31** (skipped in v1, no se menciona roto ahora), **B33/B35** (cosméticos post-V2-11/V2-16d, el informe rebaja urgencia — Artisan calcula su propio RoR).

---

## Reglas de seguridad durante la ejecución
- **Commit atómico por fase**; no mezclar concerns.
- **No uso `unwrap`/`expect`/`panic`** (clippy deny).
- Tras **cada** edit de un archivo: `cargo fmt`, `cargo clippy --all-targets`, `cargo test --lib --tests` (host), `cargo check --target riscv32imc --features embedded`. Para F4 añadir `--features embedded,regression`.
- Si un test existente (no enshrining-broken) rompe: **revertir** y re-analizar antes de forzar.
- Detengo y reporto entre fases.

---

*Última actualización: 2026-07-24. Documento de trabajo local — no commitear/pushear.*
