# Auditoría de Calidad de Código Rust — LibreRoaster

**Fecha:** 2026-08-11
**Alcance:** `src/` completo (94 archivos, ~24.068 líneas), `tests/`, `Cargo.toml`, configuración clippy
**Método:** Auditoría paralela multi-agente (8 dimensiones) + verificación manual de hallazgos clave + gates objetivo

---

## 1. Resumen Ejecutivo

| Dimensión | Veredicto | Nota |
|---|---|---|
| Compilación & gates | ✅ **Excelente** | `cargo fmt --check` limpio; `cargo clippy --all-targets` **0 warnings** (con `unwrap_used`/`expect_used`/`panic` como deny); build riscv32 sin warnings |
| Seguridad física (térmica/SSR/WDT) | ✅ **Fuerte** | Defensa en profundidad real: 3 capas independientes; sin hallazgos HIGH/CRITICAL |
| Manejo de errores | ✅ **Muy bueno** | `unwrap`/`expect`/`panic` fuera de producción verificados; 2 huecos en canal de salida |
| Concurrencia & async | ✅ **Sólido** | `SeqCst` conservador, sin await con lock sostenido, canales acotados |
| Protocolo/parser | ✅ **Robusto** | Sin CRITICAL/HIGH; hostilidad de entrada bien cubierta (fuzz proptest) |
| Rendimiento embebido | 🟡 **Aceptable** | Bucle no está limitado por CPU sino por sueño (210 ms serializados); 1 problema de RAM estática (66 KB) |
| Arquitectura | 🟡 **Incompleta** | "Cáscara descompuesta alrededor de un núcleo sin descomponer": `RoasterControl` sigue siendo god-module |
| Tests (631) | 🟡 **Fuerte con grietas** | Excelente profundidad en paths críticos; 1 tautología real, 1 test vacuo, cfg que excluye los tests numéricos más fuertes |
| Documentación API | 🔴 **Débil** | ~705 items `pub`, ~30% documentados; `missing_docs` fallaría en 300+ |

**Totales:** 0 CRITICAL · 10 HIGH · 26 MEDIUM · 22 LOW

**Top 3 de mayor impacto:**

1. **`tasks.rs:1140/1143` — fallos de escritura de salida silenciados** (HIGH): la única ruta de salida del protocolo descarta `Result` con `let _ =`; si Artisan se desconecta, las respuestas se pierden sin retry, log ni contador.
2. **`roaster_control.rs:86-87` — cola de dump de 66 KB de RAM estática** (HIGH): ~16% de la SRAM del ESP32-C3 para un diagnóstico raramente usado, con filas 6× sobredimensionadas.
3. **Tests: tautología en proptest del parser** (`parser.rs:712`) — el comando esperado del test se compara con `matches!(Ok(_expected_command))`, un binding fresco que **nunca se compara**: 37 comandos solo prueban "parsea", nunca "parsea a lo correcto".

---

## 2. Métricas Objetivas (verificadas por ejecución directa)

| Métrica | Valor |
|---|---|
| `cargo fmt --all -- --check` | ✅ PASS |
| `cargo clippy --locked --all-targets` | ✅ 0 warnings (denies: `unwrap_used`, `expect_used`, `panic`, `fallible_impl_from`) |
| Tests host (`--features test`) | 631 passing (claims en CONTEXT.md; ver hallazgo T-3: cfg `regression` excluido del gate) |
| `unsafe` | 10 archivos; ~29 apariciones — verificado: acceso a registros PAC (`RTC_CNTL`, `EFUSE`), `WDT` con unlock/relock correcto, `static_cell` |
| `unwrap`/`expect`/`panic` en código de producción | **0** (las 116 apariciones están en módulos `#[cfg(test)]`; única excepción: `unimplemented!()` intencional en `conversion.rs:262` gated a configuración inalcanzable) |
| `#[allow]` | 33, todos justificados (test code / casos documentados) |
| TODO/FIXME | 0 en `src/` |
| Archivos > 1.000 líneas | 5 (`roaster_control.rs` 3229, `tasks.rs` 1921, `parser.rs` 1715, `artisan.rs` 1318, `simulated.rs` 956) |
| Atomics | `SeqCst` en watchdog/container (conservador pero correcto); `Relaxed` solo en contadores/métricas (aceptable) |
| Mutex-await | Verificado: **ningún** `.lock().await` sostenido a través de un segundo await; los closures de `with_roaster_async` son síncronos |

---

## 3. Fortalezas (verificado como correcto)

Estas áreas se auditaron y **se confirmaron bien implementadas**:

- **Cutoff térmico** (`sensor.rs:216-223`): comparación `>=` correcta contra `OVERTEMP_THRESHOLD: f32 = 260.0`, con guard `!bean_fault.has_fault()` para ignorar sensores con fallo (no se dispara con datos corruptos), y latch por `sensor.rs:17` (no hay auto-rearme).
- **Interlock fan-floor** (`roaster_control.rs:365, 944`): `FAN_MIN_SAFETY_PCT = 20.0` forzado cuando `ssr_output > 0.0`; bug S6 del 2026-08-05 documentado y corregido.
- **Stale-temperature guard** (`roaster_control.rs:835-852`): el PID se mantiene (hold) con datos viejos en vez de integrar contra ellos; guard de no-lectura-antes-de-PID.
- **Watchdog** (`watchdog.rs`): margen 3× (feed ~330 ms vs timeout soft 1.000 ms) y 6,6× (RWDT HW 2.206 ms, const-asserted en `constants.rs:181-184`); feed fallido → escalado a emergencia tras 2 consecutivos; saturación en `saturating_sub`; sentinel `NEVER_FED`.
- **Parser**: sin panic/UB ante entrada hostil (proptest fuzz `parse_never_panics`, `hostile_bytes_never_panic_and_never_unclamp`); overflow numérico → `Err` tipado; NaN/Inf rechazados con `is_finite()` en todos los flotantes; arity exacta; `update_control` sin formatos de string en estado estable.
- **Conversión de temperatura**: cast lossy siempre pre-guardados con `clamp`/`min` (`fan.rs:53-58`, `ssr.rs:422-435`, `parser.rs:523`); denominadores variables todos con guard de división por cero.
- **Cultura de "bug-anchored comments"**: cada fix de bug lleva comentario `// Bug XX (fecha)` con la explicación — excepcional para mantener seguridad ante refactors.
- **Test doubles honestos** (`test_mocks.rs`): actualizan estado solo en éxito, fallan de forma ordenada, exponen `Arc<CsMutex>` para re-armado en mitad de test.
- **Config clippy ambiciosa**: denies de `unwrap`/`expect`/`panic` realmente efectivos (verificado: 0 en producción).

---

## 4. Hallazgos por Severidad

### 4.1 CRITICAL
Ninguno.

### 4.2 HIGH (10)

| # | Archivo | Hallazgo | Fix |
|---|---|---|---|
| H-1 | `tasks.rs:1140-1143` | Fallos de escritura USB/UART descartados con `let _ =`; mensaje ya desencolado se pierde para siempre. Las lecturas tienen contadores de error con escalado a emergencia; las escrituras, nada | Contador saturante por canal + `log::warn!`; exponer en STATUS |
| H-2 | `roaster_control.rs:22-88` | God-module de 3.229 líneas: `update_control` (:542-1051, ~500 líneas) centraliza timeout, comms-idle, charge detection, RoR gating, stale-hold, PID throttle, fan selector, probe-stuck — el cerebro nunca se descompuso | Extraer `OrchestrationPolicy`/`RoastStateMachine`; target <1.500 líneas |
| H-3 | `roaster_control.rs:145` | Encapsulación rota: `status_mut()` público + 8 accessors de controladores + campos `pub` del contenedor; 10 call-sites mutan estado de seguridad desde la capa de tasks (`tasks.rs:572-593` escribe watchdog y llama `emergency_shutdown` directamente) | Métodos tipados (`record_watchdog_failure`), `status_mut()` privado, accessors privatizados |
| H-4 | `tasks.rs:233` | `drain_commands` (orquestador) incrusta formateo de cable TC4 (STATUS/READ CSV), strings de error `ERR rate_limited…`, y reimplementa `send_handler_error` que `ArtisanFormatter::format_err` ya provee | Mover emisión de respuestas al flujo `process_artisan_command`; `drain_commands` solo drena→despacha→loguea |
| H-5 | `roaster_control.rs:86-87` | `dump_pending: Deque<String<256>, 257>` ≈ **66 KB estáticos** (16% SRAM) para un dump que duplica el ring buffer (33 KB); filas reales de 33-40 B en slots de 264 B | Dump por streaming: `Option<String<8192>>` + cursor; o filas `String<96>` × 230 ≈ 22 KB |
| H-6 | `conversion.rs:396-399` + `tasks.rs:986,1078` | La espera de conversión MAX31856 de 210 ms es sueño puro serializado: nada se solapa (drenaje, control, WDT, telemetría corren después); tick = 210+100+overhead ≈ 330 ms | (a) Disparar conversión al inicio del tick y esperar solo el resto; (b) modo continuo CMODE=1 con validación en banco; tick → ~100-150 ms |
| H-7 | `tests` — `parser.rs:712` | **Tautología**: `assert!(matches!(result, Ok(_expected_command)))` — `_expected_command` es binding fresco sin usar; 37 comandos solo prueban "parsea", nunca el payload | `assert_eq!(result, Ok(expected))` |
| H-8 | `critical_path_tests.rs:549-568` | Test vacuo: `if status.ssr_hardware_status != Available { assert_eq!(ssr_output, 0.0) }` sin `else`; el stub default es `Available` → el assert **nunca se ejecuta** | Fijar `MockSsr.set_status(Error)` y assert incondicional (como T6 en `safety_injection_midroast_tests.rs:303`) |
| H-9 | `sensor_conversion.rs:9` | Los tests numéricos más fuertes (matemática de 2-complemento 19-bit) corren solo con `--features regression`, **excluidos del gate estándar** `--features test`; el "631 tests pass" los sobrevende | Cambiar cfg a `feature = "test"` o añadir `regression` al gate canónico |
| H-10 | `max31856.rs`, `shared_spi.rs`, `ledc_bus.rs`, `ssr.rs` | Rutas solo-hardware sin cobertura host: decode de registros SPI/fault, mapping de duty → cero tests en host; `ssr_stub.rs` (el doble de producción en host) tiene 0 tests | Extraer lógica pura (decode, duty-math) a funciones host-testeadas; tests unitarios de `percentage_to_ledc_duty` en x86_64 |

### 4.3 MEDIUM (26, resumen por área)

**Protocolo/parser:**
- M-P1 `parser.rs:11-29,575` — FIFO de perfiles (4 slots) se llena **antes** del gate del multiplexer y del enqueue; un PROFILE descartado o de transporte inactivo instala perfil stale en la siguiente sesión (único estado compartido sin gate)
- M-P2 `multiplexer.rs:79-88` — Hijack de arranque: el primer comando sintácticamente válido de cualquier cable reclama la sesión (ruido UART `READ\r` gana a USB antes de que Artisan conecte)
- M-P3 `transport_tasks.rs:270-275` — Comandos válidos en transporte inactivo spamean `ERR command_ignored_inactive_channel` al host activo (canal de 16, drenado 4/5 ms)
- M-P4 `usb_cdc/tasks.rs:44-59`, `uart/tasks.rs:156-178` — Helpers legacy divergentes: USB descarta todo salvo el primer comando del buffer; ambos reactivan canal en parse-error (regresión P8 ya corregida en la ruta producción)
- M-P5 `main.rs:190` — Fallo de init USB CDC ignorado y logueado como "USB CDC initialized"

**Arquitectura:**
- M-A1 `tasks.rs` — Módulo de 1.921 líneas mezcla orquestación, glue I/O, escalado de seguridad, telemetría e instrumentación (funciones bien, módulo mal)
- M-A2 `service_container.rs:149` — DI real en construcción, cosmético en runtime: singleton global + campos pub + canales como statics; tests serializados con `TEST_LOCK` global
- M-A3 `controllers/sensor.rs:22` — Frontera control→hardware parcial: actuadores van por traits, sensores por tipo concreto (`SensorConversionHub`, `SensorFault` en API pública)
- M-A4 `dispatch.rs:27` — Tres interfaces de handler no uniformes (`RoasterCommandHandler`, `ManualCommandPolicy`, `SafetyPolicy`) con routing duplicado en `RoasterControl::process_command`
- M-A5 `abstractions.rs:140-154` — `OutputController` es stub no-op (su `process_status` solo devuelve `Ok(())`); `OutputFormatter` (traits.rs:13) tiene **cero implementores**; `CsvFormatter`/`TimeFormatter` muertos (solo tests/reexports)
- M-A6 API pública ~705 items, ~30% documentados; re-exports wildcard en 4 mod.rs; `missing_docs` fallaría en 300+
- M-A7 `constants.rs:197` — Variantes muertas: `RoasterState::Cooling`, `::Fault`, `::EmergencyStop` (0 referencias); 3 variantes "failure-ish" superpuestas
- M-A8 `handlers/artisan.rs:164` — Asimetría de commit: `SetFanManual` aplica `apply_to_status` en el handler, `SetHeaterManual` no (viola el contrato propio "single writer"); fan muta status dos veces

**Tests:**
- M-T1 `critical_path_tests.rs:127,172,430,1010` — Asserts OR (`is_emergency_active() || fault_condition`): cualquier trip enmascara al backstop específico bajo test
- M-T2 `critical_path_tests.rs:508-542` — Falta el negativo justo-debajo: `OVERTEMP_THRESHOLD - 1.0` debe NO disparar
- M-T3 `watchdog.rs:107,396` — `bean_temp` del feed es `_` (sin uso) pero el test se llama `feed_accepts_varying_temperatures` — promesa de gating por temperatura que no existe
- M-T4 `roast_resilience_tests.rs:92,99,12,77` — Tests con nombres que sobrevenden: solo parsean strings, nunca transicionan estado
- M-T5 `critical_path_tests.rs:54,76` + `roaster_control.rs:2081` — Tests de reset que nunca establecen la precondición (nunca ponen `charge_detected=true`); `accessor_methods_return_references` con 0 asserts
- M-T6 `control_loop_integration.rs:268-304` — Snapshot de status antes del tick final: heater y fan assertados en instantes distintos
- M-T7 `parser.rs:671-714` — Filas `STAT`, `OT2 150`, minúsculas, `PID;ON/OFF`... sin test semántico dedicado (consecuencia directa de H-7)

**Rendimiento:**
- M-R1 `traceability.rs:174-193` — TRACE formatea `String<256>` + `write!` (soft-float para f32) **y descarta** el resultado en release (`let _ = event`) cada tick
- M-R2 `tasks.rs:868-883` — Drain de #DUMP: hasta 5 adquisiciones del mutex async por tick, al menos 1 cada tick aunque esté idle, + `row.clone()` de 264 B
- M-R3 `tasks.rs:404-414` — `debug!` con `.await`s en sus argumentos: 2 lock acquisitions extra por tick en builds instrumentation (distorsiona el timing que mide)
- M-R4 `tasks.rs:1163-1169` — `append_crlf` aloca `Vec<u8, 1024>` por mensaje para payload de ≤258 B (hasta 4 KB churn de stack por tick)
- M-R5 `constants.rs:775-822` — `SystemStatus` (~120-220 B, `Copy`) copiado 3-4×/tick; el split `CoreRoastStatus`/`InstrumentationSnapshot` está anotado como TODO en el propio struct
- M-R6 Salida: contador de drops del canal de salida inexistente (`try_send` descartado en ~20 call-sites); los `ERR` que notifican drops pueden caerse ellos mismos bajo saturación

**Varios:**
- M-X1 `roaster_control.rs` — SILENT: `get_status()` devuelve struct de 45 campos por valor (58 call-sites) mientras `status_mut()` devuelve `&mut`; shim deprecated `with_roaster` sin llamadores
- M-X2 `uart/tasks.rs:62,71` — `ERR rate_limited` es código muerto inalcanzable: `MAX_COMMANDS_PER_TICK == ARTISAN_CMD_CHANNEL_SIZE == 16`, el canal no puede llenarse a 17

### 4.4 LOW (22, selección)

- L-1 `constants.rs:238` — Dependencia invertida: config importa de application (`MAX_COMMANDS_PER_TICK = ARTISAN_CMD_CHANNEL_SIZE`)
- L-2 `temperature.rs:52` — Timestamps truncados `as_millis() as u32`: wrap a 49,7 días (inofensivo para roasts, documentar o u64)
- L-3 `fan.rs:56` — `libm::floorf` soft-float por escritura de fan en hot path; inconsistente con SSR que usa cast `as u32`
- L-4 `parser.rs:130-144` — `FILT` coerciona silencio a 0 y acepta cualquier u8 (rompe la convención "rechazar con ruido" de todo el parser)
- L-5 `parser.rs:254-271` vs `367-400` — `PIDGAIN` y `PID;T` rechazan negativos en capas distintas con tokens ERR distintos
- L-6 `watchdog.rs:263` — `is_alive()` trata `NEVER_FED` como vivo (sentinel)
- L-7 `roaster_control.rs:3083-3123` — `FlakyFan` duplica el seam de fallo de `MockFan` (2 dobles para el mismo concepto)
- L-8 `constants.rs:96` — Margen de 15 ms no ganado en la espera de 210 ms (datasheet: 195 ms peor caso)
- L-9 `fan.rs` / `ssr.rs` — 4 transacciones SPI en sección crítica por tick con delay fijo de 160 ciclos (correcto; recuperable ~64 ciclos)
- L-10 `logging/` — `warn!`/`error!` bloquean en el mismo cable físico del protocolo (UART es Printf); añadir warning en hot paths con disciplina de una-vez
- L-11 Nombres: módulo `artisan` duplicado (handlers vs output); `abstractions.rs` mezcla error + trait + stub; 2 IIR derivadas duplicadas (`sensor.rs:230-268` vs `325-341`)
- L-12 Constantes dispersas en 4 módulos con duplicados cercanos (`REPORT_BUFFER_SIZE` 64 vs `RESPONSE_BUFFER_SIZE` 512 vs `TRACE_EVENT_MAX_LEN`)

---

## 5. Concurrencia y Async — Veredicto Verificado

Autopsia manual directa (grep + lectura):

- **Sin deadlock por lock-await**: `with_roaster_async` (service_container.rs:211) toma `.lock().await` pero el closure es **síncrono** — no hay await mientras se sostiene el mutex. Los mutex de UART/USB (`driver.rs:137,153,187,200`) son broadcasts cortos de driver.
- **Ordenamiento de memoria**: `SeqCst` en watchdog y lock-depth (conservador, correcto); `Relaxed` en `ledc_guard`/`queue_metrics`/`traceability` — todos contadores/métricas donde está bien.
- **Feed del watchdog**: desde el control loop una vez por tick (330 ms) contra timeout soft de 1.000 ms y HW de 2.206 ms; los stalls del loop quedan expuestos al HW WDT (no hay feed desde otra tarea que pudiera enmascarar un hang).
- **Prioridad de seguridad**: `EmergencyStop` y `ArtisanEmergencyStop` viven en `handlers/safety.rs:84-92` con latch (`source: Some("emergency_shutdown")`) y bypass del rate-limit del canal de comandos.
- **Send/Sync**: `unsafe` verificado — acceso a registros PAC con unlock/relock correcto del WDT; `static_cell` para init; sin `static mut` mutable.

**Conclusión: diseño de concurrencia sólido.** Notas menores: drops de canal silenciosos (M-R6), `is_alive()` sentinel (L-6), TEST_LOCK global serializando tests (M-A2).

## 6. Seguridad Física — Veredicto Verificado

| Capa | Estado |
|---|---|
| Cutoff térmico 260 °C (`sensor.rs:216`) | ✅ Correcto: `>=`, guard de fault por canal, latch |
| Stale-temperature hold (`roaster_control.rs:835-852`) | ✅ PID mantiene, no integra con datos viejos |
| Interlock fan-floor 20% (`roaster_control.rs:365,944`) | ✅ Aplicado en policy y en emergencia |
| Fan-fail = unsafe (`roaster_control.rs:1365`) | ✅ Emergencia si el fan no llega a 100% tras retries |
| SSR: OFF en errores, duty clamp 14-bit | ✅ Pre-guardado |
| RWDT HW: 2.206 ms, unlock/relock correcto | ✅ Margen 6,6× |
| Comandos con límites | ✅ Setpoints clampados; NaN/Inf rechazados en parser |
| Startup: heater OFF por defecto | ✅ (inicialización sin estado activo) |

**No se encontró ningún camino que permita a un comando erróneo/maligno fijar el heater al 100% sin pasar por límites.**

---

## 7. Plan de Remediación Priorizado

### Fase 1 — Rápida, bajo riesgo (1-2 días)
1. **[H-7] Proptest tautológico**: `matches!` → `assert_eq!` en `parser.rs:712` — desbloquea semántica real para 37 comandos.
2. **[H-8] Test vacuo** `ssr_not_detected_forces_zero_output_in_manual_mode`: fijar estado del stub y assert incondicional.
3. **[M-T3/L-6] Watchdog**: renombrar test o implementar gating por temperatura; decidir sentinel `NEVER_FED`.
4. **[M-R1] TRACE**: gating `#[cfg]` del cuerpo de las 5 funciones de `traceability.rs` — cero riesgo, elimina ~800 B de churn/tick.
5. **[M-R4] `append_crlf`**: `Vec<u8, 300>` o doble write de drivers.
6. **[M-X2] `ERR rate_limited` muerto**: bajar `MAX_COMMANDS_PER_TICK` o eliminar la rama.
7. **[L-4/L-5] FILT + PIDGAIN**: validación de parser consistente con el resto.
8. **[M-A5] Código muerto**: eliminar `OutputController` stub, `OutputFormatter` (0 implementores), `CsvFormatter`/`TimeFormatter` muertos, shim `with_roaster`, variantes `RoasterState` muertas (M-A7).
9. **[M-T2] Negativo térmico**: test `OVERTEMP_THRESHOLD - 1.0` no dispara.

### Fase 2 — Media (3-5 días)
10. **[H-1/M-R6] Visibilidad de salida**: contadores saturantes de fallos de escritura y drops de canal, expuestos en STATUS; `warn!` rate-limited.
11. **[M-P5] Init USB**: error en boot → halt path (`run_init_or_panic`), nunca loguear "initialized" en fallo.
12. **[H-5] Dump streaming**: eliminar la deque de 66 KB (o mínimo filas `String<96>`); libera ~44-66 KB de SRAM.
13. **[M-A6] Encogimiento de API + docs**: `#![warn(missing_docs)]` + `#![warn(unreachable_pub)]`, documentar ~40 entry points, podar re-exports wildcard.
14. **[M-T1/M-T4/M-T5/M-T6] Endurecer asserts de tests**: backstops independientes, precondiciones reales, snapshot post-tick.
15. **[H-9] Gate de tests numéricos**: incorporar `regression` al comando canónico de calidad en CONTEXT.md.
16. **[M-P4] Helpers legacy USB/UART**: alinear con la ruta de producción o eliminar; re-apuntar tests integración.
17. **[M-R3] `debug!` con awaits**: snapshots de temperatura antes del macro.

### Fase 3 — Estructural (1-2 semanas, con la suite de 631 tests como red)
18. **[H-2] Extraer `OrchestrationPolicy`/`RoastStateMachine`** de `update_control` y los handlers artisan; `RoasterControl` → fachada delgada (<1.500 líneas). La suite de regresión protege la extracción — semántica byte-idéntica.
19. **[H-4/M-A4] Unificar interfaces de handler** en un solo router dentro de `CommandDispatcher`; respuesta de comandos dentro del flujo de proceso.
20. **[H-3] Encapsulación**: `status_mut()` privado, métodos tipados para watchdog/latencia/guard, campos del contenedor privatizados con accessors.
21. **[M-A3] Frontera sensor**: trait `SensorHub`/`ThermometerPair` en `control/traits.rs`, hardware errors mapeados detrás del trait.
22. **[M-P1/M-P2/M-P3] Multiplexer**: gate de FIFO de perfiles, USB activo en boot o período de gracia, notificación de canal inactivo rate-limited.
23. **[H-6] Tick**: disparo temprano de conversión + solapamiento con el timer (o modo continuo con validación en banco) — el único cambio que hace real el "100 ms".
24. **[M-A1/M-R5] Split `SystemStatus`** (anotado ya en `constants.rs:772-774`) + re-homing de tasks.rs.

---

## 8. Veredicto Final

**LibreRoaster es código embebido de calidad superior a la media del sector**: gates impecables (fmt + clippy 0 warnings con denies estrictos), cero `unwrap` en producción, cero TODO/FIXME, protección en profundidad real en las capas de seguridad, parser endurecido contra entrada hostil con fuzz, cultura de comentarios de bug extraordinaria, y una suite de 631 tests que ha demostrado atrapar bugs reales.

Las deudas son de **estructura, no de comportamiento**: un god-module central que concentra toda la política de control, una API pública 3× más grande de lo necesario con 30% documentada, dos huecos de observabilidad en el canal de salida, 66 KB de RAM estática para un dump, una espera serializada de 210 ms que define el tick real (~330 ms), y algunos tests que sobrevenden lo que verifican. Ninguno de los hallazgos puede causar sobre-temperatura, heater descontrolado o pérdida del hardware — las capas de emergencia y watchdog son el backstop final y están verificadas.