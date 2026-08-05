# LibreRoaster — Casos Borde Detectados (Auditoría E2E)

**Fecha:** 2026-08-04
**Método:** Auditoría E2E de verificación estricta (3 perspectivas: flujo de datos, cumplimiento de contratos, casos borde/fallos).
**Resultado global:** ✅ Sin bloqueadores de ejecución. 646/646 tests host pasan, clippy estricto limpio, build embedded `riscv32imc-unknown-none-elf` OK.
**Estado:** Los hallazgos marcados como (RESUELTO) están cerrados en el working tree; los marcados como (ABIERTO) son mejoras defensivas pendientes.

---

## 1. Resumen ejecutivo

El sistema funciona de punta a punta en un escenario real (Artisan + USB CDC / UART, control manual/PID/perfiles, telemetría, seguridad multicapa). Esta lista documenta los casos borde analizados, verificados o detectados durante la auditoría, con evidencia archivo:línea y referencia cruzada a la documentación. Ninguno rompe la ejecución E2E; los hallazgos abiertos son riesgos defensivos menores con mitigación ya presente en el diseño.

---

## 2. Casos borde VERIFICADOS (funcionan como se espera)

| # | Caso borde | Resultado | Evidencia |
|---|---|---|---|
| EC-01 | Comando parcial con queue llena (overflow) | ✅ Se limpia la cola y se emite `ERR buffer_overflow` en el siguiente terminador; no corrompe comandos | `src/hardware/transport_tasks.rs:132-152` (Bug #2) |
| EC-02 | Terminadores CRLF (LF suelto tras CR) | ✅ El bucle `while has_terminator` consume la LF sin desperdiciar turnos; 1 comando por llegada, no 1 por 2 | `src/hardware/transport_tasks.rs:354-382` (Bug B11) |
| EC-03 | Comando > 256 bytes (PROFILE largo) | ✅ `CommandTooLong` explícito, no truncación silenciosa (antes String<128> cortaba perfiles ~170 B) | `src/input/parser.rs:79-98` (Bug B8) |
| EC-04 | NaN/Inf en entradas del operador | ✅ Rechazado en parser para OT2 (M6), PIDGAIN/PID;T/PID;LIMIT (B21, PROTO-1), SV/SETTARGET/PREHEAT/PROFILE (B9); NaN en PV → emergency | `src/input/parser.rs:250-252, 364-369, 416-418, 470-472`; `src/control/roaster_control.rs:666-669` |
| EC-05 | `PID;CHAN;3|4` | ✅ Rechazado con `ERR out_of_range` (solo 1=ET, 2=BT; antes se ejecutaba como BT silenciosamente) | `src/input/parser.rs:372-389` (Bug P12) |
| EC-06 | Handshake Artisan (UNITS/FILT) | ✅ `#OK` con prefijo `#` (requisito del driver ArduinoTC4); antes `OK` rompía la inicialización | `src/control/roaster_control.rs:1345-1378`; `src/output/artisan.rs:312-316` (Bug P-TC4, RESUELTO) |
| EC-07 | Delimitadores `OT1,75` / `IO3=50` / `OT1;75` | ✅ Los 4 delimitadores (espacio, `;`, `,`, `=`) aceptados solo para actuadores; no corrompe FILT/PROFILE/PID,ON | `src/input/parser.rs:154-179` (Bug P-TC4, RESUELTO) |
| EC-08 | OT2 fuera de rango (ej. 150, -5, 50.5) | ✅ Redondeo ±0.5, clamp 0-100, `ERR OT2_CLAMPED fan=<n> heater_unchanged`; heater/PID intactos | `src/input/parser.rs:460-485` (Bug L10); `src/control/roaster_control.rs:1193-1217` |
| EC-09 | STOP con heater encendido | ✅ Latch armado (estado Error), heater 0, fan 100% persistente (cooldown latch); solo READ/STATUS/STOP/START/PREHEAT aceptados durante fault | `src/control/roaster_control.rs:1226-1259, 991-1006` |
| EC-10 | STOP → recuperación | ✅ `PID;OFF` y `START`/`PREHEAT` desarman el latch (antes solo un camino sin productor de protocolo, brick permanente) | `src/control/roaster_control.rs:1035-1038, 1112-1114, 1499-1501` (P3/V2-1) |
| EC-11 | START duplicado durante roast activo | ✅ Ignorado; `profile_start_time` original intacto (no reinicia el reloj del roast) | `src/control/roaster_control.rs:1089-1094` |
| EC-12 | PREHEAT → START (handoff B14) | ✅ Transición limpia; `profile_start_time` fijado; backstops temporales activos | `src/control/roaster_control.rs:1096-1170` |
| EC-13 | START desde Idle con PID ya habilitado (PID;SV) | ✅ Toma el handoff completo (el gate por `is_streaming()` previo tragaba START) | `src/control/roaster_control.rs:1089-1090` (V2-4) |
| EC-14 | Desconexión de Artisan con heater encendido | ✅ `COMMS_IDLE_TIMEOUT_MS` (15 s) con gate físico `heater_energized || roast_active` → emergency (antes solo por estado de roast; manual mode desprotegido) | `src/control/roaster_control.rs:498-522` (V2-16c) |
| EC-15 | Sesión manual sin START (solo OT1) | ✅ Mismo presupuesto temporal: `heat_session_start` (M3) + MAX_ROAST_TIME 1800 s + comms-idle | `src/control/roaster_control.rs:530-582` |
| EC-16 | `OT1 0` momentáneo entre comandos | ✅ El presupuesto de MAX_ROAST_TIME no se reinicia: debounce de 60 s de heater-off | `src/control/roaster_control.rs:535-550` (R7) |
| EC-17 | Preheating largo (> 30 min, tambor grande) | ✅ Excluido de MAX_ROAST_TIME (P6); comms-idle sigue cubriendo | `src/control/roaster_control.rs:567-569` |
| EC-18 | RoR alto durante PREHEAT (tambor vacío) | ✅ Guard desarmado en Preheating (V2-16a); solo armado en Heating/Stable o Idle+PID+heater (P4) | `src/control/roaster_control.rs:699-737` |
| EC-19 | `PID;CHAN;1` (ET como PV) | ✅ RoR guard BT-only (M4/P1); transiciones Heating↔Stable por PV activo (L11/F); sin flip-flop | `src/control/roaster_control.rs:699-737, 1682-1719` |
| EC-20 | Sonda pegada (cortocircuito TC, lee ~0 °C válido sin fault bit) | ✅ Detector probe-stuck: ≥50% heater + BT plano 120 s fuera de ±5 °C del target → emergency; desarmado cerca del target (regulación legítima) | `src/control/roaster_control.rs:904-939` (P5) |
| EC-21 | BT = 0.0 en cooldown (status default / POR MAX31856) | ✅ No libera el latch de cooldown; exige BT real > 0 y < 60 °C (R8) | `src/control/roaster_control.rs:590-604` |
| EC-22 | Fan nunca configurada (sin OT2/FANPROFILE) con heater on | ✅ FAN_MIN_SAFETY_PCT = 20% aplicado (antes heater hasta 100% con 0 airflow) | `src/control/roaster_control.rs:853-876` (Bug A) |
| EC-23 | Fallo de write de heater/fan en control loop | ✅ Escalado a `emergency_shutdown` completo (antes warn-only con SSR en estado desconocido) | `src/control/roaster_control.rs:812-827, 885-888` (Bug B) |
| EC-24 | `ssr_cycle_busy` en OT1 (guard 100 ms) | ✅ Hardware-first: el estado software se commitea solo tras write aceptado; el siguiente tick reintenta | `src/control/roaster_control.rs:282-312` (M10/Bug C) |
| EC-25 | Sensor nunca leído + START (PID contra PV=0) | ✅ Tratado como stale → PID mantiene el último output aplicado (antes rampa a 100% sin feedback) | `src/control/roaster_control.rs:762-774` (NEW-5/R2) |
| EC-26 | Comando en canal inactivo (UART mientras USB activo) | ✅ `ERR command_ignored_inactive_channel` explícito (antes descarte silencioso, STOP perdido) | `src/hardware/transport_tasks.rs:272-277` (Bug D) |
| EC-27 | Error de lectura en transporte inactivo | ✅ No cuenta hacia el umbral de emergency (solo canal ACTIVO; antes un UART muerto abortaba un roast USB) | `src/hardware/transport_tasks.rs:162-170, 459-479` (P7) |
| EC-28 | Garbage en boot window (canal None) | ✅ Los errores de parseo NO activan el multiplexor desde None (antes UART podía secuestrar el canal) | `src/hardware/transport_tasks.rs:283-337` (P8) |
| EC-29 | Host USB desaparece (Artisan muerto, cable fuera) | ✅ USB write con timeout 50+20 ms (A2); línea descartada, siguiente tick lleva dato fresco; UART sigue operativo | `src/hardware/usb_cdc/driver.rs:59-90` |
| EC-30 | READ respuesta duplicada | ✅ Solo `drain_commands` emite; `handle_read_status` solo valida el formato (histórico Bug #3) | `src/application/tasks.rs:324-339`; `src/control/roaster_control.rs:1292-1313` |
| EC-31 | Segundo `#DUMP` mid-drain | ✅ `dump_pending.clear()` al inicio; no se empalman dumps | `src/control/roaster_control.rs:1468-1493` (V2-7) |
| EC-32 | Canal de salida lleno durante #DUMP | ✅ Re-push al frente del deque; ninguna fila se pierde (FIFO preservado) | `src/application/tasks.rs:849-864` (V2-7); `src/control/roaster_control.rs:1454-1466` |
| EC-33 | Ráfaga de inicio de Artisan (10-15 líneas) | ✅ Canal 16 (antes 8): ráfaga residente dentro de una ventana de tick; rate-limit 8/tick con bypass de emergencia | `src/application/service_container.rs:41-49` (E1); `src/application/tasks.rs:247-266` |

---

## 3. Hallazgos ABIERTOS (mejoras defensivas, no bloqueantes)

### EC-A1 — Diff Bug P-TC4 sin commitear ⚠️ (bloqueador de proceso)
- **Archivo**: working tree, rama `develop` (último commit 475581b)
- **Archivos afectados**: `docs/ARTISAN_CONNECTION.md`, `docs/PROTOCOL.md`, `src/control/roaster_control.rs`, `src/input/parser.rs`, `src/output/artisan.rs`
- **Causa**: la corrección del handshake (`#OK` para UNITS/FILT, delimitadores `,`/`=`/`DCFAN`) no está commiteada. Un build desde checkout limpio (CI, otro PC) fallaría el handshake de Artisan (`Arduino could not set temperature unit`) y READ nunca funcionaría.
- **Solución**: commitear los 5 archivos.

### EC-A2 — UART TX sin timeout (asimetría con USB)
- **Evidencia**: `src/hardware/uart/driver.rs:49-58` (`write_bytes` sin `with_timeout`) vs `src/hardware/usb_cdc/driver.rs:70-89` (timeout 50+20 ms, Bug A2).
- **Riesgo**: bajo en la práctica — el FIFO TX UART se vacía por hardware a 115200 baud sin depender del host. Un fallo del driver que cuelgue `flush()` congelaría `dual_output_task` y con él todo el output.
- **Solución sugerida**: `with_timeout(50 ms)` simétrico en `UartTxDriver::write_bytes`.

### EC-A3 — Drops silenciosos del canal de salida bajo carga
- **Evidencia**: `src/application/service_container.rs:48-49` (canal 16 slots); todos los productores usan `try_send` (`src/application/tasks.rs:316, 333, 263`; `src/control/roaster_control.rs:1616-1627`); drenaje 4 msgs/5 ms (`tasks.rs:1090-1121`, Bug E2).
- **Riesgo**: con `#DUMP` en curso (256 filas, ~20 s) y un host que no lee, líneas SAFETY/ERR posteriores pueden descartarse. Margen amplio en operación normal (productores ≈ 1-4 líneas/tick de ~310 ms vs capacidad de drenaje ≈ 800 líneas/s).
- **Solución sugerida**: slot de prioridad para SAFETY/ERR o descarte por antigüedad.

### EC-A4 — Discrepancia doc-código: cadencia de telemetría
- **Evidencia**: `docs/PROTOCOL.md:157-160` dice "once per control tick"; la implementación emite con gate wall-clock de 1 Hz (`src/config/constants.rs:143` `DEFAULT_OUTPUT_INTERVAL_MS=1000`; `src/application/tasks.rs:768-774`, Bug M1).
- **Impacto**: ninguna (el código es el correcto; ARCHITECTURE.md §9 ya dice 1000 ms). Actualizar PROTOCOL.md.

### EC-A5 — Latencia de respuesta READ (nota de rendimiento)
- **Evidencia**: `src/application/tasks.rs:233-360` (drain una vez por tick); tick real ≈ 310-330 ms (`src/config/constants.rs:233` `CONTROL_LOOP_TICK_MS`).
- **Impacto**: la respuesta a READ puede tardar hasta ~310 ms. Dentro de los timeouts de Artisan, pero es el punto más débil del E2E con configuraciones de timeout agresivas.

### EC-A6 — Spam de `ERR rate_limited` en ráfaga extrema
- **Evidencia**: `src/application/tasks.rs:254-266` — los comandos que exceden el límite se consumen del canal con `continue`, emitiendo un ERR por comando sobrante (hasta 8).
- **Impacto**: ruido en el wire, no pérdida de funcionalidad. El bypass de emergencia (STOP) es correcto.

---

## 4. Casos borde documentados en la repo que se verificaron en código

| Ref doc | Hallazgo | Verificación |
|---|---|---|
| PROTOCOL.md §11 | `UNITS` no persiste entre power cycles | ✅ `TemperatureSettings` es RAM-only (`src/config/constants.rs:655-706`) |
| PROTOCOL.md §11 | `FILT` coacciona malformados a 0 | ✅ `src/input/parser.rs:118-132` (riesgo documentado, aceptado) |
| PROTOCOL.md §6 | `OFF` suelto no se parsea (solo `PID;OFF`) | ✅ `parse_pid_subcommand` (`src/input/parser.rs:311-423`); `OFF` suelto → `unknown_command` |
| PROTOCOL.md §10 | `handler_failed <token>:<source>` con tokens válidos | ✅ `src/application/tasks.rs:1057-1073` |
| ARCHITECTURE.md §9 | Cadencia real ≈ 310-330 ms; watchdog SW 1000 ms | ✅ `src/config/constants.rs:224-233`; feed 1/tick ≈ 3 feeds/ventana |

---

## 5. Metodología y ejecución de la auditoría

1. **Flujo de datos**: trazado completo entrada → parseo → multiplexor → canal → handlers → formateo → transporte de salida, con atención a estados `Option`/`None`, tipos y asunciones implícitas.
2. **Contratos**: cada comando/parámetro/respuesta contrastado contra `docs/PROTOCOL.md` (última actualización 2026-08-04).
3. **Casos borde**: simulación de condiciones de fallo, concurrencia (mutex async, `ssr_cycle_busy`), latencia (tick 310 ms), excepciones (NaN/Inf) y estados intermedios (latch, cooldown).
4. **Quality gates ejecutados**:
   - `cargo fmt --all -- --check` → OK
   - `cargo clippy --locked --all-targets -- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic` → OK
   - `cargo test --target x86_64-unknown-linux-gnu --features test --lib --tests --no-fail-fast` → **646 passed, 0 failed**
   - `cargo build --release --target riscv32imc-unknown-none-elf --features embedded` → OK
