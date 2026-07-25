# Informe de análisis — LibreRoaster (julio 2026) — **v2: re-auditoría tras los fixes**

**Fecha:** 2026-07-24
**Rama analizada:** `develop` @ `a290a24` (9 commits de fixes sobre el `eed3747` auditado en v1; ~1.500 líneas cambiadas en 27 archivos)
**CI:** verde en el HEAD (fmt, clippy host+riscv32, tests, regresión-host, cobertura, build embebida)
**Informe v1:** `/home/juan/report-libreroaster-julio.md` (los IDs B1–B36 se refieren a él)

**Metodología:** revisión completa del diff `eed3747..a290a24` verificando cada fix contra el hallazgo original de v1, más tres agentes de re-auditoría buscando regresiones nuevas en los subsistemas tocados (control, parser/transporte/salida, safety/logging). Cada veredicto y cada hallazgo nuevo fue re-verificado manualmente contra el código actual. Sigue sin haber toolchain de Rust en esta máquina: la evidencia de compilación es la CI de GitHub (que, importante, **sigue sin compilar la feature `regression` para el target** — ver V2-6).

---

## 1. Resumen ejecutivo

**Balance de los 36 bugs de v1: 22 arreglados correctamente, 9 arreglados parcialmente o con problemas, 5 sin arreglar.**

Lo más importante:

- **Los 5 críticos de v1 (B1–B5) están cerrados en su escenario principal.** El firmware ya debería arrancar con termopares reales (B1), el watchdog hardware ya puede resetear (B2), y los tres bugs que mataban un asado desde comandos normales (STOP/OT2/PID;T) están corregidos. La calidad de la ronda de fixes es alta: comentarios que citan el bug ID, tests nuevos multi-tick (la carencia señalada en v1), y trade-offs explicados en vez de escondidos.
- **Hallazgo nuevo grave (V2-1):** el comando `STOP` deja el tostador **bloqueado hasta apagar y encender**. `STOP` arma el latch de emergencia y la única ruta de recuperación (`RoasterCommand::StopRoast → clear_emergency_explicit`) **no tiene ningún productor en código de producción** — ningún comando del protocolo la construye. Es un bug *preexistente* (ya estaba en `eed3747`; la v1 no lo detectó), pero los fixes de esta ronda lo hacen más visible porque ahora el latch funciona de verdad en cada tick.
- **La feature `regression` sigue sin compilar** para `riscv32 + regression`, ahora por tres motivos distintos de los de v1 (V2-6). Es la segunda vez que esta feature se "arregla" sin que nada la compile — el fix estructural es el job de CI que v1 recomendó y no se añadió.
- El subsistema `#DUMP` mejoró (de ~16 filas a 64, muestreo 1 Hz) pero **sigue perdiendo la mayor parte de un asado largo** (V2-7): la cola nueva admite 64 filas de las ~225 que produce un ring lleno, drena a 1 fila/segundo, y `dump()` sigue recortando las filas más nuevas.

### Tabla de veredictos (B1–B36)

| ID | Bug (v1) | Veredicto | Nota |
|----|----------|-----------|------|
| B1 | Init MAX31856 imposible + registro mal configurado | ✅ Correcto | CR0=0x11/CR1=0x03 conformes a datasheet; filtro 50 Hz preservado en cada disparo; espera de conversión nueva `MAX31856_CONVERSION_TIME_MS=190` |
| B2 | RWDT en modo interrupción | ✅ Correcto | `wdt_stg0=3` (ResetSystem); re-bloqueo del write-protect tras feed; comentarios corregidos (~136 kHz / ~2,2 s) |
| B3 | Fan de enfriado anulado tras STOP | ⚠️ Parcial | El latch `cooling_active` funciona; pero `PREHEAT` no lo limpia (V2-5), el borrado de `fan_profile` rompe la recarga legítima (V2-13), y la ruta de liberación por el operador documentada es inalcanzable (V2-1) |
| B4 | OT2 desactivaba el PID | ✅ Correcto | Outcome neutro en `policies.rs` y en `apply_policy_outcome`; verificado con tests |
| B5 | PID;T sustituía el PID entero | ✅ Correcto | `set_gains` in situ; extra: eliminado el pico de derivada del primer tick tras enable |
| B6 | Windup del integrador | ✅ Correcto | Integración condicional sobre el clamp propio, direccional; tests significativos |
| B7 | NaN al primer fallo de sensor | ⚠️ Parcial | El escenario principal (emergencia por un glitch) está cerrado, pero el "hold last value" **no está implementado** (V2-2) y el contador de debounce compartido entre canales lo anula con una sonda crónicamente desconectada (V2-3) |
| B8 | Parser truncaba a 128 bytes | ✅ Correcto | Buffer 256 + `CommandTooLong` explícito (nota: esa ruta es inalcanzable vía transporte; los oversize salen como `buffer_overflow`) |
| B9 | Modo °F roto en el parser | ✅ Correcto | Checks de rango eliminados de los 4 sitios; el handler valida en °C tras convertir; tests actualizados. `PROTOCOL.md` quedó desactualizado |
| B10 | Fan ~64× mal + caché de unidades mezcladas | ✅ Correcto | `max_duty()` por canal; fade almacena ticks. Trampa latente: `set_duty` (sin llamantes hoy) aún guarda % (V2-14) |
| B11 | Lector: 1 línea por chunk, CRLF | ✅ Correcto | Bucle de drenaje con progreso garantizado; sin riesgo de spin ni inanición (verificado) |
| B12 | RoR congelado todo el asado | ⚠️ Parcial | La historia ya avanza (el congelamiento está cerrado), pero la regla 2σ **por slice** sigue marcando el 70–85 % de las muestras de una rampa como outliers → el RoR converge con ~20–30 s de retardo (V2-11) |
| B13 | #DUMP perdía ~94 % | ⚠️ Parcial | Ya no se inunda el canal, pero: cola de 64 frente a ~225 filas posibles, drenaje a 1 fila/s, fila extraída perdible por `try_send`, sin `clear()` ante segundo #DUMP o nuevo asado (V2-7) |
| B14 | PREHEAT → START ignorado | ⚠️ Parcial | El handoff Preheating→Heating funciona; pero START tras `PID;SV` u `OT1` en Idle sigue tragado, con los backstops temporales desarmados (V2-4) |
| B15 | Feature regression no compilaba | ❌ No arreglado | El fix de tipos es correcto pero inalcanzable: tres roturas de compilación nuevas e independientes para riscv32+regression (V2-6); catálogo de fixtures vacío que aun así reporta éxito; CI sigue sin compilarla |
| B16 | `time_s` siempre 0 | ⚠️ Parcial | Primer asado correcto; `roast_start` nunca se resetea (el 2.º asado hereda la época del 1.º) y el flanco se dispara con cualquier comando manual pre-asado (V2-8); comentarios citan una API de `RoastLogger` que no existe |
| B17 | Ring a 10 Hz + truncado del dump | ⚠️ Parcial | Muestreo a 1 Hz correcto (ring ≈ 256 s); la otra mitad no se hizo: `dump()` sigue recortando **lo más nuevo** y `DUMP_BUFFER_SIZE=8192` sigue sin caber un ring lleno (V2-7) |
| B18 | feed_async saltaba el feed HW | ✅ Correcto | Feed incondicional primero; comentario de cadencia corregido |
| B19 | Bucles de fallo sin alimentar el RWDT | ✅ Correcto | Los tres bucles de halt alimentan el RWDT |
| B20 | RoR duplicaba el tiempo | ✅ Correcto | Solo `as_millis()/1000` |
| B21 | NaN en PID;T / PIDGAIN | ✅ Correcto | `is_finite()` en ambos caminos |
| B22 | Cross-check de calor con aliasing | ✅ Con trade-off | El falso positivo está cerrado con el gate ≥50 % duty; el precio es que el check queda **inerte por debajo del 50 %** — documentado solo en un comentario; debería estar en docs y, a futuro, sustituirse por detección sincronizada con el PWM |
| B23 | Ventana de carga 1 s vs 3 s | ⚠️ Parcial | El divisor de 3 ticks implementa correctamente la ventana de 3 s, pero la simulación con curva real (ver Adenda, V2-16b) demuestra que el par ventana/umbral sigue siendo físicamente inalcanzable: la caída máxima observable de BT en 3 s es ~9-10 °C frente a los 20 °C exigidos → `#CHARGE` sigue sin disparar nunca. Nota: `CHARGE_DETECTION_WINDOW_S` sigue decorativa (solo en comentarios) |
| B24 | "heater_cut" mentía | ✅ Correcto | `ERR OT2_CLAMPED fan={} heater_unchanged` |
| B25 | PROFILE sin conversión de unidades | ✅ Correcto | `convert_from_display` por setpoint antes de validar y almacenar |
| B26 | Drops silenciosos con canal lleno | ✅ Correcto | `ERR channel_full command_dropped` (best-effort: sale por `try_send` en un canal que puede estar también congestionado); comentario falso corregido |
| B27 | Umbral de backlog inalcanzable | ✅ Correcto | Umbral 6 sobre el canal real de 8 (nota menor: el depth no se registra en el momento exacto del drop) |
| B28 | SSR_MIN_DUTY_TICKS 2,4 ms | ✅ Correcto | 820 ticks = 10 ms (un semiciclo a 50 Hz); nota: el deadband del heater sube de 1,2 % a 5 % por el snap-to-zero |
| B29 | Ruta deprecated escribía CMODE | ✅ Correcto | 0x51 igual que la ruta async (nota cosmética: el comentario dice OCFAULT "bits 1:2"; son los bits 5:4) |
| B30 | SPI sin flush antes de CS | ❌ No arreglado | Igual que en v1 |
| B31 | TIME_FORMAT_SIZE=8 | ✅ Correcto | 16 bytes |
| B32 | Timestamp con dos `elapsed()` | ❌ No arreglado | Sigue en `artisan.rs:105-106` y `:365-366` |
| B33 | RoR 0.0 con lecturas iguales | ⚠️ Parcial | El centinela `last_bt==0.0` y el avance de historia están arreglados; el `return 0.0` duro sigue → dientes de sierra con sensores cuantizados |
| B34 | EmergencyStop sin estado Error | ⚠️ Parcial | La ruta viva (`handle_emergency_stop`) es correcta; la rama parcheada de `process_command` es código muerto (sin productor), y un `OFF` posterior con el latch armado resetea el estado a `Idle` reintroduciendo la inconsistencia (parte de V2-1) |
| B35 | Guard RoR sobre canal ET | ❌ No arreglado | `check_rate_of_rise` sigue usando la derivada del canal PV seleccionado |
| B36 | Host time driver busy-spin | ❌ No arreglado (y con riesgo nuevo) | El spin se sustituyó por **descartar el waker** para deadlines futuros (V2-9): viola el contrato del driver y convierte un test lento en un test colgado; hoy latente (CI pasa), pero roto por construcción |

---

## 2. Hallazgos nuevos (V2)

Cada uno verificado manualmente contra el código actual (`a290a24`).

### 🔴 V2-1 — `STOP` bloquea el tostador hasta apagar y encender: la recuperación de emergencia es inalcanzable desde el host

**Archivos:** `src/input/parser.rs:163` (`"STOP"` → `ArtisanCommand::EmergencyStop`), `src/control/roaster_control.rs:178` (única invocación de `clear_emergency_explicit`, gateada a `RoasterCommand::StopRoast`), `:669-682` (whitelist con `fault_condition`), `:850-875` (`handle_emergency_stop`)
**Verificado:** grep de todos los sitios de construcción de `RoasterCommand::StopRoast` — solo match arms y tests; **ningún comando del protocolo lo produce**. Preexistente en `eed3747` (no introducido por los fixes; v1 no lo detectó).

La cadena:

1. `STOP` (comando plano, un token) parsea a `ArtisanCommand::EmergencyStop` → `handle_emergency_stop` arma el latch (`activate_emergency` + `fault_condition = true`). El propio comentario dice: *"Recovery is reserved for the explicit `RoasterCommand::StopRoast` path"*.
2. Con `fault_condition` activo, la whitelist solo permite `READ`, `STATUS`, `Stop` (OFF) y `EmergencyStop`. Todo lo demás → `ERR fault_condition_active`.
3. `RoasterCommand::StopRoast` — la única llave — no se construye en ninguna ruta de producción.
4. Bonus de inconsistencia: el `Stop` (OFF) permitido llama a `stop_streaming`, que pone `state = Idle` **con el latch aún armado** — la telemetría dice Idle mientras el fan está clavado al 100 % y todos los comandos se rechazan (reintroduce la incoherencia de B34 por otra puerta).

**Escenario:** el operador pulsa STOP en su cliente (o cualquier script envía `STOP`) → calentador a 0, fan al 100 %, y a partir de ahí **ningún comando puede rearmar ni recuperar**: hay que cortar la alimentación. En un dispositivo cuya filosofía es "Artisan es la UI", esto es un brick operativo por comando normal.

**Fix propuesto:**

```rust
// roaster_control.rs — dar al host una ruta de recuperación explícita.
// Opción mínima: OFF (ArtisanCommand::Stop) con el latch armado y en estado
// Error actúa como stop-and-recover:
ArtisanCommand::Stop => {
    if self.status.fault_condition && self.safety.is_emergency_active() {
        self.clear_emergency_explicit();   // única ruta sancionada de des-latcheo
    }
    self.handle_stop()
}
```
y en `stop_streaming`, no sobrescribir el estado mientras el latch siga armado:
```rust
if !self.safety.is_emergency_active() {
    self.state = RoasterState::Idle;
    self.status.state = self.state;
}
```
(Alternativa más conservadora: añadir un comando `RESET`/`CLEAR` al protocolo y documentarlo en PROTOCOL.md. Lo importante es que exista *alguna* ruta alcanzable.)

---

### 🟠 V2-2 — B7 residual: el "hold last value" no existe — el PID come basura cruda durante el debounce

**Archivo:** `src/control/controllers/sensor.rs:101-102` vs `:117-135`
**Verificado:** lectura directa — las líneas 101-102 se ejecutan incondicionalmente *antes* del gate de fallo:

```rust
status.bean_temp = bean_temp + BT_THERMOCOUPLE_OFFSET;   // ← siempre, también con fallo
status.env_temp = env_temp + ET_THERMOCOUPLE_OFFSET;
...
if bean_fault.has_fault() && self.consecutive_fault_count >= SENSOR_FAULT_DEBOUNCE {
    status.bean_temp = f32::NAN;
    // else: hold the last valid value already in status.bean_temp.  ← FALSO
}
```

El comentario del "else" es falso: en ese punto `status.bean_temp` ya contiene la lectura **de esta muestra con fallo** (con un TC abierto, típicamente 0 o basura del registro), y los canales con fallo además se saltan la validación de rango y el guard de sobre-temperatura (por diseño). Durante hasta 4 muestras (~0,5-0,8 s) el PID, el RoR y los guards operan sobre ese valor.

**Escenario:** termopar BT abierto leyendo raw 0 → el PID ve BT = 0 °C → error ≈ target → calentador al máximo durante el debounce; a la 5.ª muestra llega el NaN y la emergencia salta igualmente — un fallo persistente recibe primero ~0,5 s de potencia máxima. El test nuevo (`update_temperatures_faulted_bt_skips_overtemp`) **fija la semántica rota**: comprueba `!is_nan()` pero el valor "retenido" es la lectura con fallo, no la última válida.

**Fix:**

```rust
// No escribir status.*_temp cuando el canal está en fallo y aún no se ha
// confirmado el debounce — retención real del último valor válido:
if !bean_fault.has_fault() {
    status.bean_temp = bean_temp + BT_THERMOCOUPLE_OFFSET;
} else if self.consecutive_fault_count >= SENSOR_FAULT_DEBOUNCE {
    status.bean_temp = f32::NAN;
} // else: status.bean_temp conserva el último valor válido de verdad
```
(idéntico para `env_temp`; actualizar los tests para inyectar un valor basura distinto y comprobar que se retiene el anterior).

---

### 🟠 V2-3 — B7 residual: el contador de debounce es compartido entre canales — una sonda ET desconectada lo anula

**Archivo:** `src/control/controllers/sensor.rs` (un solo `consecutive_fault_count`), alimentado en `roaster_control.rs` con `bean_fault || env_fault`
**Verificado:** en el diff — `apply_fault_debounce(has_fault = bean || env)` incrementa un contador único, pero el envenenamiento con NaN se decide por canal contra ese contador compartido.

**Escenario:** configuración de una sola sonda (ET desconectada — caso que el propio código contempla): el contador está permanentemente ≥ umbral por el fallo crónico de ET. El **primer** glitch transitorio de BT cumple `bean_fault && count >= 5` → NaN inmediato → emergencia en el mismo tick. Exactamente el escenario que B7 debía eliminar.

**Fix:** un contador por canal:

```rust
consecutive_bean_faults: u8,
consecutive_env_faults: u8,
// gate por canal:
if bean_fault.has_fault() && self.consecutive_bean_faults >= SENSOR_FAULT_DEBOUNCE { ... }
```

---

### 🟠 V2-4 — B14 residual: START sigue tragándose después de `PID;SV` u `OT1` — sin backstops temporales

**Archivo:** `src/control/roaster_control.rs:744` (`if self.is_streaming() && self.state != RoasterState::Preheating`), `controllers/dispatch.rs:65-83, 109-115`
**Verificado:** `PID;SV` → `enable_pid_control` → PID + salida continua activados con `state = Idle`; `OT1` activa `artisan_control` en Idle. En ambos casos `is_streaming()` es true y el START posterior cae en la rama "ignored": `profile_start_time` sin fijar, estado en `Idle` → `MAX_ROAST_TIME_SECS` y el timeout de comunicaciones **inactivos** (ambos exigen Preheating/Heating/Stable), detección de carga inerte y sin log de backup para ese asado.

**Escenario:** flujo habitual de Artisan — fijar el setpoint primero (`PID;SV;215`) y después pulsar START: el tostador calienta indefinidamente sin supervisión de tiempo máximo ni de comunicaciones.

**Fix:** gatear por estado, no por streaming — la misma lógica que el handoff de PREHEAT:

```rust
if matches!(self.state, RoasterState::Heating | RoasterState::Stable) {
    info!("Artisan+ START ignored - roast already active");
    ...
} else {
    // Idle-con-PID, Idle-manual y Preheating hacen todos el handoff completo
    ...
}
```

---

### 🟡 V2-5 — B3: `PREHEAT` no limpia el latch de enfriado — precalentar contra el fan al 100 %

**Archivo:** `src/control/roaster_control.rs` — escrituras de `cooling_active` solo en líneas 340 (STOP, set), 369 (recovery explícito — inalcanzable, ver V2-1), 453 (BT < 60 °C) y 757 (START)
**Verificado:** grep — `handle_preheat` no toca el latch.

**Escenario:** lotes consecutivos — `OFF` con BT ≈ 205 °C (latch armado) → `PREHEAT;180` inmediato: el PID calienta contra un ventilador forzado al 100 % en cada tick, y como el heater mantiene BT > 60 °C, el latch no puede autoliberarse durante todo el precalentado. Solo el START lo suelta.

**Fix:** `self.cooling_active = false;` en `handle_preheat` (misma justificación documentada que en START: re-energizado deliberado).

---

### 🟡 V2-6 — B15: la feature `regression` sigue sin compilar (tres causas nuevas) y su ejecución reporta éxito sin probar nada

**Archivos:** `src/safety/regression.rs:145`, `:92-100`; `Cargo.toml:34`; `.github/workflows/ci.yml` (sin cambios)
**Verificado:** los tres puntos, por lectura de firmas y cfgs (sin toolchain local; confirmables con `cargo check --target riscv32imc-unknown-none-elf --features embedded,regression`):

1. `SensorConversionHub::new()` sin argumentos **solo existe** bajo `#[cfg(not(target_arch = "riscv32"))]` (`conversion.rs:237-238`); en riscv32 sin simulated-sensors `new` exige 2 sensores (`:208-209`). El módulo está gateado a riscv32 → E0061 garantizado. Existe `new_uninit()` privado exactamente para esto, sin usar.
2. Error de tipos anidados: el closure devuelve `Ok::<bool, ContainerError>(…)` dentro de `with_roaster_async` (que ya envuelve en `Result<R, ContainerError>`), así que el resultado es `Result<Result<bool,_>,_>` y `.unwrap_or(true)` con un `bool` es E0308 (`service_container.rs:197-205`).
3. `Cargo.toml`: `regression = ["embedded-hal-mock"]` arrastra un crate solo-std a la build no_std — el propio comentario del fix lo declara "incompatible with the embedded build" y la feature lo sigue activando.

Además: `canonical_fixtures()` devuelve `&[]`, de modo que incluso compilando, la "regresión" reproduce **cero fixtures** y aun así emite la línea `SAFETY OT-REGRESSION` de éxito — el dispositivo anuncia una regresión aprobada que no probó nada.

**Fix:** (a) `Self::new_uninit()` o inyectar el hub; (b) closure devolviendo `bool` plano: `Ok::<bool,_>` → `shutdown_result.is_err()` y fuera `.map(...).unwrap_or(true)`; (c) `regression = []` y mover `embedded-hal-mock` a un feature solo-host; (d) **añadir a CI** `cargo check --target riscv32imc-unknown-none-elf --features embedded,regression` — es la tercera vez que esta feature se rompe sin que nada lo detecte; (e) no emitir la línea de éxito con catálogo vacío (emitir `SAFETY OT-REGRESSION-EMPTY no_fixtures`).

---

### 🟡 V2-7 — B13/B17 residual: `#DUMP` sigue perdiendo la mayor parte de un asado largo

**Archivos:** `src/control/roaster_control.rs:58` (`Deque<String<256>, 64>`), `:1039-1072` (`break` al llenar, sin `clear()`); `src/application/tasks.rs:805-822` (drenaje dentro del gate 1 Hz, `try_send` tras extraer); `src/logging/roast_logger.rs` (sin cambios: `LOG_CAPACITY=256`, `DUMP_BUFFER_SIZE=8192`, truncado oldest-first)
**Verificado:** aritmética — a 1 Hz el ring lleno produce ~200-225 filas (~36 B/fila dentro de los 8 KiB); la cola admite 64 → un asado de más de ~1 min sigue perdiendo ~70-90 % del log por `#DUMP`. El comentario justificativo compara *bytes* de la cola contra *bytes* del payload cuando la restricción que muerde es el *número de slots*. Encima: el drenaje va a 1 fila/segundo (64 filas ≈ 1 minuto de goteo entremezclado con la telemetría en vivo), la fila ya extraída de la cola se pierde si el `try_send` falla, un segundo `#DUMP` a mitad de drenaje mezcla dos dumps parciales, y `dump()` sigue descartando las filas **más nuevas** al truncar (la mitad de B17 que no se hizo).

**Fix conjunto:**

```rust
// 1. Cola dimensionada al ring (o cursor sobre el ring, sin copia):
dump_pending: heapless::Deque<heapless::String<256>, { LOG_CAPACITY + 1 }>,

// 2. handle_dump_log: empezar limpio
self.dump_pending.clear();

// 3. Drenaje: N filas por tick de 100 ms, fuera del gate should_emit,
//    devolviendo la fila a la cola si el canal está lleno:
while let Some(row) = roaster.take_dump_row() {
    if output_channel.try_send(row.clone()).is_err() {
        roaster.push_dump_row_front(row);   // no perder la fila extraída
        break;
    }
}

// 4. roast_logger::dump(): seleccionar de nuevo→viejo qué filas caben y
//    emitirlas en orden cronológico (conservar el final del asado), o
//    DUMP_BUFFER_SIZE >= LOG_CAPACITY * (SAMPLE_CAPACITY + 1).

// 5. handle_start_roast: self.dump_pending.clear();
```

---

### 🟡 V2-8 — B16 residual: la base de tiempo del log no se resetea entre asados y se dispara con comandos manuales

**Archivo:** `src/application/tasks.rs:142-146` (`mark_continuous_started` solo escribe si `None`; nada lo devuelve a `None` — verificado por grep)
**Problemas:** (a) el 2.º asado del mismo encendido hereda la época del 1.º: su `#DUMP` empieza en `time_s` ≈ el uptime acumulado; (b) el flanco que captura la época es el de *salida continua*, que también activan `OT1`/`OT2` manuales — mover el slider del fan 3 min antes del START desplaza la base de tiempo 3 min; (c) los comentarios describen una API de `RoastLogger` (`start_offset`, `elapsed_secs_at`) que **no existe** (`start_roast(&mut self, _now: Instant)` sigue descartando el instante).

**Fix (el que los comentarios ya describen):** mover la época dentro del logger y borrar `TickState.roast_start`:

```rust
// roast_logger.rs
pub fn start_roast(&mut self, now: Instant) {
    self.active = true;
    self.buffer.clear();
    self.start = Some(now);
}
pub fn log_sample(&mut self, data: LogSampleData, now: Instant) {
    let elapsed = self.start.map(|s| now.duration_since(s).as_secs() as u32).unwrap_or(0);
    ...
}
```
y llamar a `start_roast` desde `handle_start_roast` (el evento START real, no el flanco de telemetría).

---

### 🟡 V2-9 — B36: el driver de tiempo host ahora descarta wakes futuros (cuelgue en vez de spin)

**Archivo:** `src/host_time_driver.rs:51-53`
**Verificado:** `if _at <= self.now() { waker.wake_by_ref(); }` — **sin rama else**. Para un deadline futuro el waker se descarta y nada vuelve a invocar `schedule_wake`: cualquier código host que haga `block_on(Timer::after(..))` sin otra actividad concurrente queda aparcado para siempre. El comentario describe un "OS-thread sleep otherwise" que no está escrito, y su excusa ("`&Waker` no es `'static`") es incorrecta: `Waker` es `Clone + Send + 'static`. Hoy es latente (la CI pasa — ningún test actual espera un timer futuro sin actividad concurrente), pero el driver viola el contrato de embassy-time por construcción.

**Fix:**

```rust
fn schedule_wake(&self, at: u64, waker: &Waker) {
    let now = self.now();
    if at <= now {
        waker.wake_by_ref();
        return;
    }
    let w = waker.clone();
    let delay_us = at - now;
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_micros(delay_us));
        w.wake();
    });
}
```
(un hilo por wake es tosco pero correcto para infraestructura de test; la alternativa elegante es un hilo-timer único con un min-heap).

---

### 🟡 V2-10 — Transporte: el flag de overflow sobrevive a la extracción de un terminador suelto y descarta el siguiente comando válido

**Archivo:** `src/hardware/transport_tasks.rs:328-347`
**Verificado:** el check `overflow.triggered` está *después* del `let-else`; si tras el flush del buffer solo quedan terminadores (`[CR, LF]`), las extracciones devuelven `None` → `continue` sin consumir el flag → el flag se aplica al **siguiente comando válido** (p. ej. el próximo `READ`), que se descarta con `ERR buffer_overflow` atribuido al comando equivocado.

**Fix:**

```rust
let Some(command_data) = extract_line_from_event_queue(event_queue) else {
    if overflow.triggered {
        overflow.triggered = false;
        send_parse_error(ParseError::BufferOverflow, channel, config).await;
        return;
    }
    continue;
};
```

---

### 🟡 V2-11 — B12 residual: la regla 2σ por-slice sigue suprimiendo el 70-85 % de las muestras de una rampa — RoR con 20-30 s de retardo

**Archivo:** `src/output/artisan.rs:439-454` + `is_temperature_outlier`
**Verificado:** el test de outlier se aplica a cada slice del ring **por separado** con la media/σ de ese slice. Cuando la deque de 10 da la vuelta, el slice frontal contiene solo las muestras más viejas: en una rampa lineal la muestra actual se desvía hasta ~9d de ese slice mientras 2σ de un fragmento de 3 elementos es ~1,63d → outlier garantizado. Simulación (rampa 0,5 °C/s a 1 Hz): 81/119 muestras marcadas sin ruido, 98/119 con ruido de 0,2 °C. Como la historia sí avanza (eso está arreglado), el efecto ya no es congelación sino que el IIR solo se actualiza en ~2 de cada 10 muestras → el RoR converge con ~20-30 s de retardo, relevante para detectar crash/flick.

**Fix:** aplicar el test sobre la ventana combinada (copiando ambos slices a un array, como ya hace el cálculo del RoR unas líneas más abajo), idealmente des-tendenciado:

```rust
let mut window: heapless::Vec<f32, BT_HISTORY_SIZE> = heapless::Vec::new();
let (front, back) = self.bt_history.as_slices();
let _ = window.extend_from_slice(front);
let _ = window.extend_from_slice(back);
let is_outlier = ArtisanFormatter::is_temperature_outlier(current_bt, &window);
```

---

### 🟢 V2-12 a V2-15 — Menores

- **V2-12** — `calculate_ror` no protege contra BT no finito: un solo NaN entra en la ventana y `last_filtered_ror = α·NaN + … = NaN` **para siempre** (el IIR nunca se recupera). Añadir `if !current_bt.is_finite() { return self.last_filtered_ror; }` al inicio. (`artisan.rs:393`)
- **V2-13** — `stop_streaming` borra `fan_profile` innecesariamente (el latch de cooling ya tiene precedencia y borrar `profile_start_time` ya desactiva la interpolación): tras `OFF`+`START`, el perfil de temperatura se conserva pero el de ventilador desaparece en silencio — asimetría que obliga a reenviar `FANPROFILE`. Quitar `self.fan_profile = None;`. (`roaster_control.rs:331-334`)
- **V2-14** — `LedcChannelHandle::set_duty` (sin llamantes de producción hoy) sigue guardando **porcentaje** en la caché de ticks — trampa latente que reintroduciría B10 con el primer llamante futuro. Convertir con la misma fórmula del fade o eliminar el método. (`ledc_bus.rs:121-137`)
- **V2-15** — Varios: la profundidad de cola no se registra en el momento del drop (`transport_tasks.rs:240-243` — mover `record_queue_depth` fuera del `if sent`); `CHARGE_DETECTION_WINDOW_S` sigue sin usarse en código (cambiarla no hace nada — derivar `CHARGE_SAMPLE_TICK_DIV` de ella); comentarios que describen código inexistente (la API de RoastLogger en tasks.rs, el hilo del host driver, "drains one row per tick" cuando es por segundo, OCFAULT "bits 1:2"); `PROTOCOL.md` desactualizado tras B9 (rango 50-300 y código de error en el cable cambiado); `formatters/ror.rs` es una segunda implementación de RoR divergente y muerta (pre-B12/B20) — candidata a borrado; `dump_pending`/`charge_history_tick_div` sin reset en stop (cosmético).

---

## 3. Evaluación actualizada

### Hardware real

Los dos bloqueadores deterministas de v1 están cerrados: **el firmware ya debería arrancar con termopares reales** (B1: registros conformes al datasheet, espera de conversión con margen) **y el watchdog hardware ya protege de verdad** (B2+B18+B19 forman ahora una cadena coherente: reset real, feed incondicional del lazo vivo, halts de diagnóstico alimentados). La telemetría del ventilador es correcta (B10). Probabilidad de una primera sesión real funcional: **alta** — con dos avisos operativos: no pulsar `STOP` esperando poder continuar (V2-1: requiere ciclo de alimentación) y saber que el detector "heater ON sin calor" está inerte por debajo del 50 % de duty (trade-off de B22). La validación HIL con resultados archivados sigue pendiente y sigue siendo el paso de mayor valor.

### Calidad

La ronda de fixes es seria y se nota la mejora de proceso: commits organizados por bloques citando IDs, tests multi-tick nuevos (exactamente la carencia que v1 señaló, p. ej. los tests de anti-windup de B6 y del debounce de B7), y trade-offs explicados en vez de ocultados (B22, el catálogo vacío de B15). De 36 bugs, 22 quedaron bien cerrados a la primera.

Los dos patrones de v1 persisten atenuados:

1. **Fixes a medias:** la cola de 64 del #DUMP frente a ~225 filas (misma clase de defecto que cerraba), el `roast_start` que no se resetea, el "hold last value" comentado pero no implementado. El anti-patrón común es validar el fix contra el *escenario del informe* y no contra el *invariante* ("ningún dump pierde filas", "toda época de tiempo se resetea con el asado").
2. **Comentarios que describen código inexistente:** la API fantasma de `RoastLogger`, el hilo del host driver que "no compila" (sí compila), el "one row per tick" que es por segundo. En tres de los cinco fixes problemáticos, el comentario correcto habría hecho evidente el bug.

Y una lección repetida que ya no admite excusa: **B15 se ha "arreglado" dos veces sin que nada lo compile**. El job de CI para `--features embedded,regression` cuesta 6 líneas de YAML y habría convertido V2-6 en un fallo de build inmediato.

### Prioridades recomendadas

1. **V2-1** (STOP → brick): es el único hallazgo nuevo con impacto de operación directa; el fix es pequeño y el riesgo de no hacerlo, alto.
2. **Bloque de control residual** — V2-2, V2-3, V2-4, V2-5: cuatro fixes pequeños que completan de verdad B7, B14 y B3.
3. **CI para la feature `regression`** + los 3 fixes de compilación de V2-6.
4. **Bloque #DUMP** (V2-7 + V2-8): un solo PR que cierre cola, drenaje, truncado y época de tiempo — es una feature con mucho valor de usuario que lleva dos rondas a medio cerrar.
5. Los no-arreglados de v1 (B30, B32, B33, B35) y los menores V2-9…V2-15, en cualquier orden.
6. **La sesión HIL** con los fixes aplicados y resultados archivados — sin cambios desde v1: es la palanca más grande del proyecto.

---

## Adenda (2026-07-24): simulación de una curva real de Artisan contra el firmware

Tras cerrar el informe v2 se ejecutó una simulación numérica (script: `artisan_curve_sim.py`, conservado en el scratchpad de la sesión) que replica **exactamente** los algoritmos del firmware — el filtro RoR de `MutableArtisanFormatter` (Deque ring de 10 con `as_slices`, regla 2σ por slice, IIR α=0,25), el derivador del guard de seguridad (`refresh_filtered_derivative`, EMA α=0,3, límite 0,5 °C/s, 3 ticks consecutivos), el detector de carga (muestra cada 3 ticks, Deque de 10, umbral 20 °C) y los umbrales térmicos — alimentándolos con una curva realista de tueste de tambor: carga con sonda a 178 °C, turning point 96 °C en 1:25, pico RoR 22 °C/min, first crack 196 °C a 9:00, descarga a 211 °C en 11:30, ET máximo 227 °C. Ruido y cuantización del MAX31856 (0,0078 °C/LSB) incluidos.

### Resultado global

**Un asado estándar completo funcionaría de principio a fin con datos fieles en Artisan.** BT/ET correctos toda la sesión con margen a OVERTEMP (40 °C en BT, 33 °C en ET); el guard de RoR no dispara en falso durante el asado (máx. filtrado 0,47 °C/s vs límite 0,5); la secuencia de protocolo realista (`CHAN`→`UNITS`→`FILT`→`READ` 1-3 s + `OT1`/`OT2` + `PID;SV`) pasa entera con los fixes de esta ronda. Punto clave mitigante: **Artisan calcula su ΔBT (RoR) en el cliente a partir de los valores BT que recibe** — la mala calidad del campo `ror` del firmware (V2-11) no afecta a lo que Artisan pinta y graba.

### V2-16 — Hallazgos nuevos de la simulación

**V2-16a (ALTO) — El guard de RoR puede disparar una emergencia en el precalentado con tambor vacío.** `check_rate_of_rise` corre cada tick **en todos los modos y estados** (`update_control`, `roaster_control.rs:514-520`). Sin grano, una sonda BT de baja masa sube rápido al calentar el tambor; la simulación con una sonda a 0,65 °C/s dispara `rate_of_rise_exceeded` → `emergency_shutdown` **a los 1-2 segundos** de empezar a calentar — y por V2-1, eso deja el tostador bloqueado hasta ciclo de alimentación. *Caveat: depende de la masa de la sonda; una vaina de 3-4 mm puede quedarse en 0,2-0,4 °C/s y no disparar.* El margen también es fino en asados rápidos reales: una recuperación post-TP de 28-30 °C/min queda a ~10 % del límite.
**Fix propuesto:** gatear el guard a estados con grano (`Heating`/`Stable`) o a `charge_detected`, y/o subir `MAX_BT_RATE_OF_RISE` a ~0,8 °C/s. Debe corregirse junto con V2-1.

**V2-16b (MEDIO) — El umbral de `#CHARGE` es físicamente inalcanzable incluso tras B23.** La caída máxima de BT en cualquier ventana de 3 s de la curva simulada es **9,4 °C** (la inercia de la sonda limita la caída a ~2 °C/s los primeros segundos tras la carga); el detector exige >20 °C en 3 s — más del doble. `#CHARGE` no dispara nunca con física real. El veredicto de B23 en la tabla queda corregido a Parcial: la *ventana* se arregló, la *feature* sigue muerta.
**Fix propuesto:** umbral ~8 °C con la ventana de 3 s actual, o mantener 20 °C con una ventana de 8-10 s. (Impacto de usuario bajo: en la práctica CHARGE se marca manualmente en Artisan.)

**V2-16c (MEDIO) — En modo Artisan-manual puro no hay backstops temporales.** El flujo más común con TC4 (Artisan manda `OT1`/`OT2` desde sus sliders o su propio PID, sin `START`) deja el estado en `Idle`, y tanto el timeout de comunicaciones (15 s) como el tiempo máximo de asado gatean por `Preheating|Heating|Stable` → **ninguno protege**; el roast logger tampoco arranca (sin `#DUMP` de respaldo). Si el USB se desconecta con el heater al 80 %, la única red restante es el corte OVERTEMP a 260 °C — evita el incendio pero quema el lote. Es la generalización de V2-4.
**Fix propuesto:** gatear el comms-idle y el max-roast-time por "heater energizado" (`ssr_output > 0`) en lugar de por estado de asado.

**V2-16d (dato, no bug nuevo) — Cuantificación de V2-11 con curva real:** el filtro de outliers suprime el **74 %** de las muestras de RoR (513/691). El valor emitido resulta utilizable en crucero (mediana reportada 12,8 vs real 12,3 °C/min en Maillard; 5,9 vs 5,4 en desarrollo; pico seguido con ~1 s de retardo — mejor de lo estimado en V2-11) pero es dentado, y en la caída post-carga el error es grande (MAE ~40 °C/min). El caso `equal→0.0` (B33) resultó irrelevante en curva real: 2 muestras de 691. Esto rebaja la urgencia práctica de V2-11/B33 (solo afectan a `#DUMP` y telemetría propia, no a Artisan).

### Prioridades actualizadas

V2-16a se inserta en el puesto 2 de la lista de prioridades (junto al bloque de control residual): es un falso disparo plausible en el primer uso real con `PREHEAT`, y su coste está multiplicado por V2-1. V2-16c se une al fix de V2-4 (misma raíz: gateo por estado en vez de por condición física). V2-16b es oportunista (2 líneas si se toca ese archivo).
