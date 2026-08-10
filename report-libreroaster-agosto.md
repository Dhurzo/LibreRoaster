# LibreRoaster — Análisis completo (v2, re-auditoría desde cero)

**Fecha:** 2026-08-10 · **Rama:** `develop` @ `d562ef2` · **Tamaño:** ~23.850 líneas en `src/`, ~9.100 en `tests/`
**CI:** 🟢 verde · **Tests host:** 674 pasando, 0 fallos (ejecutados localmente) · **Build ESP32-C3:** compila · **Clippy:** limpio (6 avisos triviales)

**Método:** auditoría desde cero — 4 agentes revisaron todo `src/` **a ciegas** (sin acceso a informes previos, a `docs/SAFETY_BUGS.md` ni a `BUG-CATCH-PLAN.md`, para no heredar sesgos), y después verifiqué cada hallazgo relevante personalmente contra el código, contra las fuentes reales de las dependencias en el registry local (`esp-hal 1.0.0`, `embassy-time 0.5.1`, PAC `esp32c3 0.31.0`, `heapless`) y, donde era posible, **ejecutando tests de reproducción**. Toolchain de Rust disponible: los tests, la build embebida y 34 combinaciones de features se compilaron de verdad, no se asumieron.

**Balance: 1 crítico · 7 altos · 13 medios · ~18 bajos.**

---

## 1. ¿Funcionaría en hardware real?

**Hoy, no: un único bug impide calentar.** Todo lo demás del cuadro es favorable, y por primera vez los cimientos están sólidos.

**El bloqueador (C1).** La verificación de escritura del duty del SSR lee el registro **`DUTY_R`** del LEDC (el duty que el hardware está aplicando *en el cable*) inmediatamente después de escribir el nuevo valor. En el ESP32-C3 un cambio de duty vía `DUTY` + `DUTY_START` + `PARA_UP` no toma efecto hasta el siguiente periodo de PWM — y el SSR corre a 5 Hz, o sea **200 ms de latencia**. La lectura ocurre microsegundos después de la escritura, así que devuelve el duty anterior. Con una tolerancia de 128 ticks sobre 16383 (0,78 %), **cualquier** rampa de calentador por encima de ~0,8 % falla la verificación, reintenta, vuelve a fallar y devuelve `Err` → el lazo de control escala a `emergency_shutdown("Heater control failure")`. El tostador no puede calentar. El mismo defecto rompe la ruta inversa: `force_heater_off` escribe 0 correctamente pero lee el duty alto anterior, "falla" sus 3 reintentos y marca `ssr_hardware_status = Error` sobre una escritura que sí funcionó.

Corroboración interna fuerte: **los propios ejemplos HIL del proyecto leen `duty()`** (el registro de configuración), no `duty_r()` — `examples/hil_ssr.rs:29` y `examples/hil_fan.rs:40`. `DUTY_R` se introdujo para un caso legítimo distinto (decidir el reinicio del fade del ventilador) y se reutilizó para verificar escrituras; son dos usos que necesitan registros distintos.

Los 674 tests de host no lo detectan porque los mocks implementan `read_duty_ticks` devolviendo el valor comandado — no hay LEDC real.

*Honestidad sobre la certeza:* la **ruta de código es segura** (verificada por mí de extremo a extremo: `read_duty_ticks` → `DUTY_R` → `monitor_ledc_after_set` → `Err` → `emergency_shutdown`). La **latencia del hardware** viene de la documentación (TRM y contrato LEDC de ESP-IDF: el duty se aplica en el siguiente ciclo) y no de una medición en silicio. Es exactamente el tipo de afirmación que la sesión HIL debe cerrar — y una ilustración perfecta de por qué ese banco no es opcional.

**Lo que sí está bien** (verificado por mí o por los agentes contra datasheet/HAL): registros del MAX31856 correctos (CR0=0x11 con FILT50 y OCFAULT, CR1=0x03 tipo K, one-shot 0x51, mapa de fallos conforme); extensión de signo de 19 bits correcta incluso en el extremo −2048 °C; **el contrato SPI se respeta** (`bus.flush()` antes de subir CS — el bloqueador de la ronda anterior está cerrado); matemática del timeout RWDT correcta, con el shift de efuse compensado igual que hace esp-hal y el ancho de pulso de reset programado; pinout coherente; frecuencias PWM apropiadas; el retardo tCS no se optimiza (verificado compilando a ensamblador).

**Veredicto:** con C1 arreglado (~6 líneas), la probabilidad de una primera sesión real funcional es **alta**. Sigue sin existir evidencia HIL (no hay `tests/hardware/runs|reports|goldens`, los umbrales siguen siendo genéricos), y el README lo declara con honestidad ejemplar.

---

## 2. Calidad del código

**Ha mejorado de forma sustancial y medible.** Lo que antes eran señales de proceso ahora son hechos verificables:

- **674 tests pasando** y una suite de seguridad genuinamente sustantiva: un *harness de invariantes* con proptest que ejecuta 1000 asados aleatorios con inyección de fallos (sonda muerta, desconexión, overtemp, writes fallando, fan roto, SSR atascado) comprobando **7 invariantes por tick**, con semilla de regresión archivada — evidencia de que cazó un bug real. Esto es infraestructura de verificación de nivel profesional.
- **Los tres bloqueadores históricos están correctamente cerrados**, no parcheados a medias: el `flush()` del SPI, el shift de efuse del RWDT y la aritmética saturante en las épocas de tiempo.
- **34 combinaciones de features compiladas** en esta auditoría: 30 pasan.
- La documentación de decisiones de diseño (`SAFETY_BUGS.md`) distingue explícitamente lo corregido de lo mantenido-por-diseño, con test de reproducción por hallazgo. Es la práctica correcta.

**Intenté romper el diseño de seguridad yo mismo y no pude.** Escribí tests de reproducción para dos hipótesis: (a) que un `START` repetido pospusiera indefinidamente los backstops temporales con sonda muerta — en 90 minutos simulados solo hubo **14 s** de calentamiento; (b) que el estado obsoleto del detector probe-stuck hiciera irrecuperable el rearme — el relatch que observé era el guard de RoR actuando correctamente sobre una rampa irreal. **Ambas hipótesis descartadas**: el diseño aguantó.

**Debilidades que persisten:**

1. **Los mocks ocultan la clase de bug más peligrosa.** C1 (y su gemelo, la verificación del `force_heater_off`) viven exactamente en la costura donde el mock devuelve lo que el código espera. Ningún número de tests de host los alcanza. Es la misma lección de rondas anteriores, ahora con un ejemplo caro.
2. **Comentarios que describen código inexistente** — sigue siendo el patrón dominante y en dos casos oculta bugs reales: el comentario de `apply_policy_outcome` afirma "the next tick retries the same value (no silent drop)" y **no hay reintento** (H2); el argumento de robustez del debounce de presencia de calor se basa en una cadencia de un tick, pero el único llamante mantiene un throttle de 1000 ms que la invalida (H7). Cuando el comentario y el código discrepan, aquí gana el comentario en la revisión y pierde el usuario.
3. **Invariantes que se auto-certifican.** `safety_thresholds_are_sane` asserta `WATCHDOG_FEED_INTERVAL_MS < HW_WATCHDOG_TIMEOUT_SECS*1000` — pero ninguna de las dos constantes describe lo real (la cadencia real es un tick de 310 ms; el timeout real se programa con un literal aparte). La aserción se reduce a `100 < 2000`: no puede fallar nunca. Lo mismo con `MAX_SAFE_TEMP`/`MAX_TEMP`, que nada aplica mientras el rango real es un literal duplicado.
4. **Código muerto divergente** que un futuro "conectemos esto" reintroduciría como bug: el `ArtisanFormatter` inyectado por el builder (sin conversión °F, RoR sin timestamps), `RorCalculator`, `run_writer_task`, `process_command_data`.

---

## 3. Bugs pendientes y cómo solucionarlos

Cada hallazgo indica **cómo se verificó**. "Reproducido por mí" = escribí y ejecuté un test que lo demuestra.

### 🔴 CRÍTICO

#### C1 — La verificación del duty del SSR lee el registro *aplicado* (`DUTY_R`): toda rampa de calentador falla y escala a emergencia

**Archivos:** `src/hardware/ledc_bus.rs:84-101` (`read_register` → `duty_r()`), `:250-252` (`read_duty_ticks` lo usa), `src/hardware/ssr.rs:29-74` (`monitor_ledc_after_set`), llamado en `:487` y `:593`; consecuencia en `src/control/roaster_control.rs:841-856`.
**Verificación:** ruta de código completa comprobada por mí; semántica de `DUTY_R` (19 bits, solo lectura, "duty actual de la señal de salida") contra el PAC y el contrato LEDC de esp-hal/ESP-IDF; corroborado por los propios ejemplos HIL del repo, que usan `duty()`. **Pendiente de confirmar en banco** (medir `DUTY_R` a +1 ms y +250 ms tras una escritura a 5 Hz), pero el fix es correcto en cualquier caso.

```rust
// ledc_bus.rs — separar los dos usos: verificación de escritura vs duty aplicado.
    /// Duty APLICADO (lo que sale por el cable). Lag de hasta un periodo PWM
    /// (200 ms a SSR_CONTROL_CYCLE_HZ = 5). Válido para decidir el reinicio de
    /// un fade; NO para verificar una escritura recién hecha.
    fn read_live_register(&self, entry: &ChannelEntry<'a>) -> u16 {
        let regs = unsafe { &*LEDC::ptr() };
        (regs.ch(entry.number as usize).duty_r().read().duty_r().bits() >> 4) as u16
    }

    /// Duty de CONFIGURACIÓN: `set_duty_hw` lo actualiza de forma síncrona.
    /// Este es el registro contra el que debe verificarse una escritura.
    fn read_config_register(&self, entry: &ChannelEntry<'a>) -> u16 {
        let regs = unsafe { &*LEDC::ptr() };
        (regs.ch(entry.number as usize).duty().read().duty().bits() >> 4) as u16
    }

impl<'a> LedcDutyReader for LedcChannelHandle<'a> {
    fn read_duty_ticks(&self) -> u16 {
        self.bus.read_config_register(self.entry())   // antes: read_register (DUTY_R)
    }
    // ...
}
    pub fn live_duty(&self) -> u16 { self.bus.read_live_register(self.entry()) }
```

---

### 🟠 ALTOS

#### H1 — El detector probe-stuck compara BT con el setpoint aunque el PID regule ET → emergencia latcheada espuria a los 120 s

**Archivo:** `src/control/roaster_control.rs:945-948`.
**Verificación: reproducido por mí.** Con `PID;CHAN;1` (ET como PV, configuración soportada y testeada), ET regulado a 199 °C con target 200 y un BT **sano** derivando 0,2 °C en 121 s: `emergency=true state=Error`. El resto del lazo se migró deliberadamente a `status.pv` por este motivo (hay comentarios explícitos sobre canal 1 vs 2 en las líneas 1733-1762); este punto se quedó atrás.

```rust
        let probe_bt = self.status.bean_temp;
        // Comparar contra la variable que el lazo REGULA (status.pv). Con
        // pid_channel == 1 el PID controla ET y BT vive legítimamente decenas
        // de grados por debajo del setpoint, plano: `near_target` sería
        // siempre false y el detector dispararía sobre una sonda sana.
        let regulating = self.status.pid_enabled
            && ((self.status.target_temp - self.status.pv).abs() <= PROBE_STUCK_TARGET_MARGIN_C
                || self.status.pid_channel == 1);
        if self.status.ssr_output > 0.0 && probe_bt.is_finite() && !regulating {
```
(Mejor aún: ejecutar el detector sobre `status.pv` — una sonda atascada en el canal *controlado* es el peligro real; una BT atascada mientras se regula ET es un problema de telemetría.)

#### H2 — Comandos de calentador dentro de la ventana de 100 ms del guard SSR se pierden; el comentario afirma un reintento que no existe

**Archivos:** `src/control/roaster_control.rs:282-295`, `src/control/controllers/actuator.rs`.
**Verificación: reproducido por mí.** `OT1=40` aceptado; `OT1=90` 50 ms después → `Err(ssr_cycle_busy)`; **6 ticks de control más tarde el heater sigue al 40 %**. `drain_commands` procesa hasta 8 comandos seguidos en un mismo tick, cada uno con su `Instant::now()`, así que un arrastre de slider en Artisan entra de lleno. La dirección peligrosa es que se pierda una **reducción** de potencia.

```rust
        if let Some(heater) = outcome.heater_target {
            match self
                .actuator
                .apply_guarded_heater(heater, current_time, true, &mut self.status)
            {
                Ok(_) => {}
                Err(RoasterError::InvalidState { source: Some("ssr_cycle_busy") }) => {
                    // Guard ocupado: adoptar el valor como setpoint manual para que
                    // el siguiente tick de control (que no usa reject_on_busy) lo
                    // aplique, y confirmar al operador en vez de perder el comando.
                    self.dispatch.commit_manual_heater(heater);
                    self.dispatch.disable_pid();
                    self.status.pid_enabled = false;
                    self.status.artisan_control = true;
                    self.dispatch.get_output_manager_mut().enable_continuous_output();
                    return Ok(());
                }
                Err(e) => return Err(e),
            }
```
(Alternativa mínima: pasar `reject_on_busy = false` aquí — el tick de control ya gestiona la ventana ocupada manteniendo el valor.)

#### H3 — El rate-limiter **descarta** la cola de una ráfaga en vez de diferirla, anulando el canal de 16 slots

**Archivos:** `src/application/tasks.rs:247-266`; capacidades: `MAX_COMMANDS_PER_TICK = 8` vs `ARTISAN_CMD_CHANNEL_SIZE = 16`.
**Verificación:** semántica de descarte comprobada por mí en el código (`try_receive()` saca el comando del canal y `continue` lo tira); el agente lo ejecutó: con 12 comandos en ráfaga y `START` en la posición 12, **START nunca se ejecutó y llegaron 0 líneas `ERR`** al host (el canal de salida de 16 ya estaba saturado por las respuestas de los 8 procesados). El comentario del propio canal dice que se dobló a 16 para que "una ráfaga completa permanezca residente durante una ventana de tick" — cosa que este descarte anula.

```rust
// Opción recomendada (una línea): el canal acotado ya limita el trabajo por
// tick; el presupuesto extra solo compra pérdida silenciosa.
pub const MAX_COMMANDS_PER_TICK: usize = ARTISAN_CMD_CHANNEL_SIZE; // = 16
// (manteniendo el bypass de emergencia tal cual)
```
Si se quiere conservar un presupuesto menor, hay que **diferir** en lugar de descartar: presupuestar antes de `try_receive` (mirando `cmd_channel.len()`) o llevar los excedentes a una cola local que el siguiente tick drene primero.

#### H4 — `--features embedded,async-lock-depth-metrics` no compila para riscv32; CI nunca lo construye

**Archivo:** `src/application/service_container.rs:312-324`.
**Verificación: compilado por mí** → 3× `E0599` (`fetch_add`/`fetch_max`/`fetch_sub`): `riscv32imc` no tiene extensión A, así que `core::sync::atomic` no ofrece RMW. La feature existe precisamente para instrumentar en el dispositivo, y nunca ha podido hacerlo. El resto del código ya usa `portable_atomic` por este mismo motivo (`watchdog.rs`, `traceability.rs`).

```rust
-    use core::sync::atomic::{AtomicUsize, Ordering};
+    use portable_atomic::{AtomicUsize, Ordering};
```
```yaml
# .github/workflows/ci.yml
      - name: Build async-lock-depth-metrics for ESP32-C3
        run: cargo build --release --target riscv32imc-unknown-none-elf --features embedded,async-lock-depth-metrics
```

#### H5 — `NotDetected`/`Error` de la fuente de calor es un latch terminal: ninguna ruta puede devolver `Available`

**Archivos:** `src/hardware/ssr.rs:203-258`, `src/hardware/heat_presence.rs:62-79`; consumidores en `src/control/roaster_control.rs:771-823`, `:1707-1710`.

El único punto que escribe `Available` exige `current_duty >= 8192` (50 % de 14 bits), pero cuando el estado no es `Available` el lazo fuerza la salida a 0 % en **cada** tick — así que el duty nunca puede volver a subir hasta el umbral. No hay API de reset ni comando Artisan que lo limpie. Un episodio transitorio de 5 muestras "sin calor" (ver H7) fija el calentador a 0 % el resto de la sesión, con grano caliente y sesión viva; solo se recupera apagando y encendiendo.

```rust
// ssr.rs — dentro del early-return de duty bajo:
        if (self.current_duty as u32) < min_observable_ticks {
            self.heat_absent_count = 0;
            // Recuperación: el latch fija el duty a 0 %, que está por debajo del
            // umbral de observabilidad — sin esto el estado no puede volver a
            // Available (heater muerto hasta ciclo de alimentación). Una muestra
            // LOW es fiable a CUALQUIER duty (significa que circula corriente),
            // así que se honra como evidencia de fuente de calor viva.
            if self.hardware_status != SsrHardwareStatus::Available
                && matches!(read_pin(), Ok(true))
            {
                info!("Heat source re-detected at low duty - clearing latch");
                self.hardware_status = SsrHardwareStatus::Available;
            }
            return Ok(());
        }
```
Añadir además un rearme visible para el operador (por ejemplo, resetear `hardware_status` desde `clear_emergency_explicit`), para que un circuito de sensado genuinamente ausente no se rearme en bucle silenciosamente.

#### H6 — Una verificación de duty fallida deja `current_duty` obsoleto aunque la escritura al hardware sí funcionó

**Archivo:** `src/hardware/ssr.rs:580-613` (idéntico en `:474-504`).

`set_duty_raw` ya devolvió `Ok` — el duty **está** en el LEDC. Solo falló la relectura, pero el `?` se salta la actualización de la caché. Efectos: la telemetría reporta un duty falso, y el gate de observabilidad y el cross-check de presencia de calor evalúan la lógica de seguridad contra un duty que el hardware no está aplicando.

```rust
        self.pwm_channel
            .set_duty_raw(ledc_duty)
            .map_err(|_| SsrError::PwmError { source: "set_duty_failed" })?;
        // La escritura llegó al periférico: registrarla ANTES de verificar, para
        // que la caché no pueda divergir del valor comandado por un fallo de
        // relectura.
        self.base.current_duty = ledc_duty;

        monitor_ledc_after_set(/* … */)?;
```

#### H7 — El argumento que hace "a prueba de aliasing" al debounce de presencia de calor se basa en una cadencia que el código no usa

**Archivos:** `src/hardware/heat_presence.rs:10-16` y `src/hardware/ssr.rs:563-572` (documentan y asumen **un tick** entre muestras) vs `src/control/controllers/actuator.rs:271-283`, que **mantiene un throttle de 1000 ms** y es el único llamante.

El throttle se quitó de `periodic_check` pero sobrevive un nivel más arriba. Con `CONTROL_LOOP_TICK_MS = 310`, el primer tick que alcanza 1000 ms es el 4º → intervalo real **1240 ms** → separación de fase `1240 mod 200 = 40 ms`, no los 130 ms que el argumento requiere. La cota "separación > 100 ms ⇒ como máximo 2 muestras OFF seguidas" es falsa: con paso de 40 ms y ventana OFF de 100 ms salen **3** consecutivas, y degrada sin límite al acercarse el intervalo a un múltiplo de 200 ms. Si el periodo del lazo fuera 300 ms en vez de 310, el paso de fase sería 0 y el detector muestrearía **la misma fase para siempre**.

```rust
// actuator.rs — la cota del debounce (ver hardware/heat_presence.rs) exige UN
// tick de control entre muestras; un gate de 1000 ms lo cuantiza a 4 ticks
// (1240 ms → 40 ms de separación de fase) y rompe el argumento.
    pub fn periodic_health_check(&mut self, now: Instant) {
        self.last_health_check = Some(now);
        self.heater.periodic_health_check(now.as_millis() as u32);
    }
```
Más robusto todavía: dejar de depender de aritmética de fases — muestrear el pin varias veces a lo largo de un periodo PWM completo (o latchear "estuvo LOW en algún momento de este periodo") y alimentar **eso** al debounce.

---

### 🟡 MEDIOS

**M1 — `NaN` llega al cable en la telemetría continua.** La ruta continua aplica `convert_to_display` sin `normalize_read_value`, que sí usan READ y STATUS (23 usos en el mismo archivo). Y la inyección de `NaN` es real: un ET desconectado (configuración soportada) pone `status.env_temp = f32::NAN` tras el debounce. *Verificado por mí.* Resultado: cada línea de telemetría lleva `NaN` mientras un `READ` sobre el mismo cable reporta `0.0` — dos superficies del protocolo en desacuerdo sobre el mismo fallo.
```rust
// artisan.rs — MutableArtisanFormatter::format
let et = ArtisanFormatter::normalize_read_value(
    status.temperature_settings.convert_to_display(status.env_temp));
let bt_display = ArtisanFormatter::normalize_read_value(
    status.temperature_settings.convert_to_display(bt_c));
let gas = ArtisanFormatter::normalize_read_value(status.ssr_output);
let ror_display = ArtisanFormatter::normalize_read_value(ror_display);
```

**M2 — `UartTxDriver::write_bytes` descarta el contador de bytes → truncado silencioso a 128 bytes.** `embedded_io_async::Write::write` es una escritura **parcial** por contrato; esp-hal escribe `min(128 - fifo_count, len)` y devuelve la cuenta, que el `Ok(Ok(_))` tira. Cualquier línea de salida >128 bytes (un STATUS ancho, una fila de `#DUMP`, un `ERR handler_failed` largo) se corta a media y `write_bytes` devuelve `Ok`.
```rust
        // `Write::write` es PARCIAL: usa write_all para no truncar.
        match with_timeout(Duration::from_millis(50), self.tx.write_all(data)).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) | Err(_) => return Err(UartError::TransmissionError),
        }
```

**M3 — El latch de overflow del event-queue descarta el primer comando *limpio* posterior al flush.** *Reproducido por el agente:* 256 bytes de basura sin terminador seguidos de `STOP\r` → la cola contenía solo `STOP\r` y se rechazó con `ERR buffer_overflow`. Es un `Stop`/`EmergencyStop` perdido. El fix es atar el descarte a la posición en el flujo (descartar hasta el terminador de la línea corrupta) en vez de a un `bool`:
```rust
pub struct EventQueueOverflow { pub triggered: bool, pub discarding: bool }
// en push_to_event_queue, al desbordar:
    queue.clear();
    overflow.triggered = true;
    overflow.discarding = !(byte == 0x0D || byte == 0x0A);
// y mientras discarding, consumir bytes hasta el terminador sin encolarlos.
```

**M4 — `wdt_flashboot_mod_en` se *activa*, contra el TRM y contra ambos HAL de vendor.** El TRM (§12.2.2.4) dice que ese bit debe **limpiarse** tras el arranque, *antes* de configurar el RWDT por software; esp-hal escribe `wdt_flashboot_mod_en().bit(false)` (verificado por mí en `rtc_cntl/mod.rs:606`) y esp-idf llama a `rwdt_ll_disable_flashboot_mode()`. Aquí se hace `set_bit()`. No aporta nada (el RWDT ya queda armado por `wdt_en`) y reintroduce una segunda ruta de habilitación en la única red de seguridad del asado.
```rust
-                .wdt_flashboot_mod_en()
-                .set_bit()
+                .wdt_flashboot_mod_en()
+                .clear_bit()
```

**M5 — El driver de tiempo de host lanza un hilo del SO por **poll**, no por timer.** *Verificado por mí en la fuente de embassy-time:* `Timer::poll` llama a `schedule_wake` en cada poll mientras está pendiente (`timer.rs:188`). El agente lo midió: 500 polls de un solo `Timer::after` → **502 hilos vivos**. Cualquier test que haga `select(Timer, canal)` con tráfico sostenido agota `RLIMIT_NPROC`. Solo afecta al host, pero es el arnés del que depende toda la suite. Fix: un hilo worker único con lista de deadlines y dedupe por `waker.will_wake()`.

**M6 — La aserción del margen del watchdog es vacua.** `WATCHDOG_FEED_INTERVAL_MS = 100` no describe la cadencia real (un tick = `CONTROL_LOOP_TICK_MS` = 310 ms) y no se usa en el lazo; `HW_WATCHDOG_TIMEOUT_SECS = 2` no programa nada (el valor real es un literal en `watchdog.rs`). La aserción se reduce a `100 < 2000`. Si alguien sube `MAX31856_CONVERSION_TIME_MS` (ya pasó una vez) y el tick supera ~2,2 s, el RWDT resetea la placa una vez por tick sin que nada lo avise.
```rust
pub const WATCHDOG_FEED_INTERVAL_MS: u64 = CONTROL_LOOP_TICK_MS as u64;
pub const HW_WATCHDOG_STAGE0_CYCLES: u32 = 300_000;   // usada por safety::watchdog::init
pub const RC_SLOW_CLK_HZ: u32 = 136_000;
pub const HW_WATCHDOG_TIMEOUT_MS: u64 =
    HW_WATCHDOG_STAGE0_CYCLES as u64 * 1000 / RC_SLOW_CLK_HZ as u64; // ≈ 2206
const _: () = assert!(WATCHDOG_FEED_INTERVAL_MS * 2 < HW_WATCHDOG_TIMEOUT_MS,
    "el tick debe dejar ≥2x de margen antes de que el RWDT resetee el chip");
```

**M7 — `MAX_SAFE_TEMP`/`MAX_TEMP`/`MIN_TEMP` están muertas y el rango real es un literal duplicado.** Ninguna tiene usos fuera de tests, mientras `is_valid_target_temp` lleva `(50.0..=300.0)` a mano. El riesgo concreto: alguien baja `MAX_TEMP` para un tambor pequeño, el const-assert le obliga a bajar también `OVERTEMP_THRESHOLD`, la build queda verde… y `SETTARGET;300` sigue aceptándose. La edición de seguridad quedó a medias y el test "safety thresholds are sane" la bendijo.
```rust
pub const MIN_TARGET_TEMP: f32 = 50.0;
pub const MAX_TARGET_TEMP: f32 = MAX_TEMP;
pub fn is_valid_target_temp(temp: f32) -> bool {
    temp.is_finite() && (MIN_TARGET_TEMP..=MAX_TARGET_TEMP).contains(&temp)
}
const _: () = assert!(MAX_SAFE_TEMP < OVERTEMP_THRESHOLD);
```
(y cablear `MAX_SAFE_TEMP` a una comprobación real o borrarla: una constante llamada "temperatura máxima segura" que nada aplica es peor que ninguna.)

**M8 — `START` no limpia `bt_charge_history` → `#CHARGE` falso en el siguiente lote.** *Reproducido por el agente.* `stop_streaming` resetea los cinco campos de carga; la ruta de START solo tres. En una cadencia `PREHEAT → START` (lote a lote sin `OFF`, el flujo normal) el primer tick del lote 2 compara con la BT pre-carga del lote 1 (~205 °C) y dispara `#CHARGE` sin grano, además de deshabilitar la detección real el resto del lote.
```rust
            self.charge_detected = false;
            self.charge_time = None;
            self.status.charge_detected = false;
            self.bt_charge_history.clear();       // faltaban
            self.charge_history_tick_div = 0;     // estas dos
```

**M9 — Cada tick borra el marcador `SsrHardwareStatus::Error` de "el heater no se apagó".** `force_heater_off` lo pone cuando fallan todos los reintentos (la señal honesta de que el estado físico es desconocido), pero `update_control` sobreescribe el campo desde el driver en el siguiente tick, y el driver nunca reporta `Error` por esa causa. El marcador vive menos de un tick.
```rust
        let hw = self.actuator.get_ssr_hardware_status();
        if self.status.ssr_hardware_status != SsrHardwareStatus::Error
            || !self.safety.is_emergency_active()
        {
            self.status.ssr_hardware_status = hw;
        }
```

**M10 — El contador de debounce del RoR del PV no se limpia al disparar** (su hermano `check_bt_rate` sí lo hace). Queda clavado en el límite durante todo el periodo latcheado y nada lo resetea en la recuperación, así que en el asado siguiente el guard dispara con **un solo** tick por encima del umbral, sin la confirmación de 3 que su constante promete.
```rust
            if self.pv_ror_exceeded_count >= ROR_EXCEEDED_CONSECUTIVE_LIMIT {
                self.pv_ror_exceeded_count = 0;     // como en check_bt_rate
                return Err(RoasterError::TemperatureOutOfRange {
                    source: Some("rate_of_rise_exceeded"),
                });
            }
```
(y resetear `last_filtered_derivative` / `bt_guard_derivative` en `handle_start_roast` para que un asado no herede el filtro del anterior.)

**M11 — El sub-parser de `PID` rechaza delimitadores espaciados legales.** `PID; SV; 250` → tras normalizar `;`→espacio queda `["SV", "", "250"]` y `parts[1]` vacío → `Err`. PROTOCOL cita el TC4: coma/espacio/punto y coma/igual son legales "para todos los comandos", y `parse_profile_args` ya salta segmentos vacíos. *Reproducido por el agente.*
```rust
    let parts: heapless::Vec<&str, 8> = args
        .split([';', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .take(8)
        .collect();
```

**M12 — El `ArtisanFormatter` muerto divergente.** El formatter que inyecta el builder no se usa en producción, pero si alguien lo cableara (es la abstracción "correcta" a la que uno tiende) reintroduce dos bugs ya arreglados: emite ET/BT en °C crudos ignorando `UNITS;F`, y su RoR es `Δ/(n−1)` por muestra sin el ×60 a °/min. *Verificado por el agente ejecutándolo.* Fix: borrar el impl `OutputFormatter for ArtisanFormatter` + `RorCalculator` y reapuntar los dos tests a `MutableArtisanFormatter`.

**M13 — El ring de asado retiene ~4 min, no los "10-15 min" que documenta.** A 1 Hz, `LOG_CAPACITY = 256` son 256 s; y `DUMP_BUFFER_SIZE = 8192` no alcanza ni para esas 256 filas (~9,3 KB), así que se descartan además ~30 filas antiguas. Un `#DUMP` tras una desconexión a los 9 minutos devuelve los últimos ~3,7 min: carga, dry-end y first crack se han ido. Como subir el ring a 15 min costaría ~43 KB de RAM (no caben con el heap de 72 KB), el fix realista es **corregir el comentario** y/o bajar la cadencia del log a 0,25 Hz para ~17 min con la misma huella.

---

### 🟢 BAJOS

| ID | Qué | Fix |
|---|---|---|
| L1 | `set_duty_raw` no resincroniza `slewing_output`: tras un `OFF`+`OT1` en el mismo lote, el limitador de slew arranca desde un valor obsoleto | asignar `slewing_output` en `set_duty_raw`/`force_heater_off` |
| L2 | `Err` muerto tras `emergency_shutdown(...)?`: el variante documentado (`overtemp_detected`) es inalcanzable | devolver directamente `self.emergency_shutdown(...)` |
| L3 | 6 sitios usan el `duration_since` que puede panicar, mientras sus hermanos ya usan `saturating_duration_since` (no alcanzable hoy en el lazo mono-tarea) | homogeneizar a `saturating_duration_since` |
| L4 | `examples/gpio_roast_test.rs` sin `required-features` → `--examples` falla en riscv32 | `required-features = ["simulated-sensors"]` |
| L5 | `build.rs` indexa `args[2]` comprobando solo `len() > 1` → pánico en la ruta de error del linker | `if let [_, kind, what, ..] = args.as_slice()` |
| L6 | Timeout de 50 ms en la escritura USB cancela a mitad de chunk: parte de la línea ya salió (el comentario dice "línea descartada") | emitir `\r\n` en la ruta de timeout, o trocear en el driver |
| L7 | Caché de duty usa `2^bits − 1` mientras `set_duty` de esp-hal usa `2^bits` (off-by-one en el canal de 8 bits) | derivar de la misma expresión por API, o quedarse solo con `set_duty_raw` |
| L8 | `PID;SV\|CHAN\|CT\|LIMIT` y `PIDGAIN` aceptan basura final (`PID;SV;150;junk` → Ok) | `parts.len() != N`; `take(5)` en `PIDGAIN` para ver el 5º token |
| L9 | PROFILE viaja por un canal lateral global de un solo slot: dos PROFILE en una ráfaga → se aplica el segundo y el otro `SetProfile` es no-op | llevar el payload en el comando, o atar el slot al `TraceId` |
| L10 | `CONTROL_LOOP_PERIOD_MS` no se referencia: el lazo tiene el `100` a mano, y de esa constante se deriva la ventana de `#CHARGE` | `Timer::after(Duration::from_millis(CONTROL_LOOP_PERIOD_MS as u64))` |
| L11 | `queue_metrics` es de solo escritura: sin getter ni emisor, el backlog es inobservable (y el comentario promete telemetría) | añadir `snapshot()` y exponerlo en STATUS, o borrar el módulo |
| L12 | Rama `if` con cuerpo solo-comentario en la ruta de RoR | borrar el `if` |
| L13 | Constantes de RoR mal documentadas en comentarios (ventana 9 s no 4 s; α 0,25 no 0,3; buffer 64 no 32) | referenciar las constantes en vez de duplicar números |
| L14 | Failover por inactividad del multiplexor sin cablear (`is_idle`/`reset` solo se llaman en tests) y el mock devuelve duración 0 en todo build host | cablearlo desde el lazo, o borrarlo y documentar que el failover es por llegada |
| L15 | Campo AMB de `READ` pasa por `convert_to_display`: en °F el placeholder "siempre 0.0" sale como `32.0` | emitir `0.0` literal, o corregir PROTOCOL §4 |
| L16 | PROTOCOL §5 dice "una línea por tick de control"; el código emite a 1 Hz por reloj | corregir a "una por segundo (`DEFAULT_OUTPUT_INTERVAL_MS`)" |
| L17 | Comentarios con números obsoletos: `MAX31856_CONVERSION_TIME_MS` dice 190 y vale 210; `regression.rs` dice watchdog 500 ms y son 1000 | actualizar |
| L18 | Código muerto: `run_writer_task` + `command_pipe` (nunca spawneado, RAM estática), `process_command_data` (pierde todo tras la primera línea, es `pub`), campos `event_queue_size`/`command_pipe_size` de `TransportConfig` | borrar |

---

## 4. Mejoras (respetando la filosofía del proyecto)

1. **La sesión HIL es ahora el cuello de botella único.** C1, H5, H6 y H7 son todos bugs de la costura hardware/mock: invisibles para 674 tests y para cualquier revisión que no baje al contrato del HAL. El playbook ya está escrito. La medición concreta que cierra C1 son 5 minutos: escribir un duty al SSR a 5 Hz y leer `DUTY_R` a +1 ms y a +250 ms.
2. **Un test de integración con LEDC real** (aunque sea un solo `hil_ssr` en CI manual con la placa conectada) cubre la clase entera de C1/H6.
3. **Completar la matriz de features en CI**: falta `embedded,async-lock-depth-metrics` (roto), `embedded,simulated-sensors`, `std` y `regression` sueltos, y `--examples` en riscv32 (roto). Cada celda no construida ha acabado rota; ya es la cuarta vez.
4. **Convertir los invariantes auto-certificados en invariantes reales** (M6, M7): las constantes deben ser las que el código usa, y las aserciones deben poder fallar. Un `const _: () = assert!(...)` sobre los valores reales cuesta cero en runtime.
5. **Regla de oro para los comentarios**: si un comentario afirma un comportamiento (un reintento, una cadencia, una cota), que exista un test que lo demuestre. H2 y H7 son bugs que el comentario correcto habría hecho evidentes.
6. **Borrar el código muerto divergente** (M12, L18): sus tests verdes dan cobertura ilusoria sobre rutas que no existen, y son trampas para el próximo que "conecte lo que ya estaba".
7. **Newtype de unidades** (`Celsius`/`DisplayTemp`): M1, M12, L15 son la enésima aparición de la misma clase; el sistema de tipos la elimina con coste cero.

---

## 5. Veredicto

**Como software, el proyecto ha dado un salto real**: 674 tests, un harness de invariantes con proptest que sí caza bugs, los tres bloqueadores históricos correctamente cerrados, 30 de 34 combinaciones de features compilando, y una documentación de decisiones de seguridad que distingue lo arreglado de lo asumido. Intenté romper el diseño de recuperación de emergencia con dos hipótesis y aguantó las dos.

**Como firmware para hardware real, hay un bloqueador**: C1 impide calentar. El fix son ~6 líneas y separa dos usos que nunca debieron compartir registro.

**Para la alpha 0.1**: cerrar C1 y H1–H3 (los tres de operación normal: emergencia espuria con `PID;CHAN;1`, comandos de heater perdidos, ráfaga descartada), más H4 si se quiere la CI honesta. Con eso, el resto es una lista de "known issues" perfectamente publicable — pero **el smoke HIL debe ir antes del tag**, no después: esta ronda es la prueba de que la clase de bug que queda ya no se encuentra leyendo el código.
