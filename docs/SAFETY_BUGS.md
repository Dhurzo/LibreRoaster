# SAFETY_BUGS.md — Informe de caza de bugs críticos (seguridad de tostado)

**Fecha:** 2026-08-05 (bug hunt + fixes)
**Plan:** `BUG-CATCH-PLAN.md` (fases 0–4 y 6 ejecutadas; fase 5 pendiente de placa sin instrumentos)
**Suite:** 674 tests host verdes, 0 fallos, 0 ignored · clippy estricto limpio · fmt limpio · build embedded `riscv32imc-unknown-none-elf` OK
**Repros rojos:** 0 — los 2 repros `#[ignore]` (S5, S10) fueron des-ignorados y pasan tras los fixes.

---

## 1. Veredicto

### ✅ **Apto para tostar** — todos los hallazgos corregidos (2026-08-05)

- **Ningún bug BLOQUEANTE** quedó abierto (el único candidato, el diff P-TC4 sin commitear / EC-A1, está resuelto en `47e4942` y verificado en Fase 0).
- **El CRÍTICO-SEGURIDAD S1 fue corregido**: el detector probe-stuck ahora se arma con cualquier duty > 0, cerrando la ventana de roast manual con sonda muerta. El harness (1000 roasts aleatorios) pasó de 165/1000 roasts con exposición a **0**.
- **Todos los hallazgos OPERATIVOS (S3, S4, S6, S8) y LATENTES (S5, S9, S10) corregidos**; S7 (telemetría honesta) corregido junto con S4.
- **Una decisión de diseño se mantiene (S2)**: START/PREHEAT/OFF desarman el latch de emergencia por diseño (puerta de recovery del operador, Bug P3) — ver §4.
- **Pendiente: Fase 5 hardware** (checklist §5) — verificación física con la placa real, sin instrumentos.

El firmware cumple el diseño de seguridad multicapa: todo Err interno de `update_control` pre-escala a emergency latcheada, el watchdog se alimenta incondicionalmente, el boot arranca todo-off, y ningún input serial puede energizar el heater fuera del protocolo documentado. Los fixes se verificaron con tests de reproducción (rojo pre-fix → verde post-fix) y no introdujeron nuevas violaciones en el harness de invariantes.

---

## 2. Método (cómo se minimizaron los falsos negativos)

6 aproximaciones complementarias; cada hallazgo exigió test de reproducción (rojo pre-fix, verde post-fix):

| Fase | Aproximación | Resultado |
|---|---|---|
| 0 | Baseline: fmt + clippy estricto + suite completa + build embedded; verificación del fix P-TC4 en HEAD | EC-A1 cerrado |
| 1 | Verificación de candidatos estáticos de la auditoría previa, cada uno con repro | S1–S6 (S5 rojo) |
| 2 | Inyección de fallos mid-roast con mocks ampliados (emergency fan fallible, N-de-M, status configurable, NaN) | 6 tests + 3 de transporte + **S9 nuevo** |
| 3 | Proptests hostiles: parser (bytes/NUL), PID (NaN/±Inf/huge), actuador (f32 arbitrarios), RoastCurve (waypoints), formatters (status hostil) | **S10 nuevo** (rojo) |
| 4 | Harness de invariantes: 1000 roasts aleatorios con fallos inyectados, 7 invariantes por tick | 0 violaciones inesperadas; **S1 expuesto 165/1000** → 0 tras el fix |
| 6 | Consistencia doc-código | PROTOCOL.md EC-A4, TESTING.md actualizados |

Seams explotados: traits `Heater`/`Fan` (inyectables), `update_temperatures_with_fault`, `process_event_queue` (byte-level), `WatchdogFeeder`.

---

## 3. Hallazgos rankeados (estado: RESUELTO el 2026-08-05)

### CRÍTICO-SEGURIDAD

#### S1 — Manual mode sin supervisión con sonda muerta finita (RESUELTO)
- **Síntoma**: termopar cortocircuitado lee ~0.0 °C, un valor *válido* (`is_temperature_valid`, sensor.rs:278-280) sin fault bit → overtemp, RoR y NaN no disparan. En manual (`OT1 n` con n < 50), el detector probe-stuck estaba desarmado (roaster_control.rs exigía `ssr_output ≥ 50`). El comms-idle (15 s) quedaba neutralizado porque Artisan hace polling `READ` cada ~1 s y **cada comando resetea** `last_command_received_at_ms` (roaster_control.rs:992-994). Resultado: heater a ciegas hasta MAX_ROAST_TIME (30 min).
- **Fix**: el detector probe-stuck se arma con **cualquier duty > 0** (roaster_control.rs, gate `ssr_output > 0.0` en vez de `>= PROBE_STUCK_HEATER_MIN_PCT`). BT plano ≤ 1 °C durante `PROBE_STUCK_TIMEOUT_SECS` (120 s) con el heater on → emergency latcheada. Trade-off documentado en el código: una sesión manual que sostiene BT plano < 1 °C por 2 min a baja potencia dispara una emergency recuperable (fail-safe por diseño).
- **Repro**: `dead_probe_manual_roast_now_trips_probe_stuck` (verifica la emergencia; pre-fix corría a ciegas); harness I6: 165/1000 roasts con exposición → **0/1000**.
- **Actualización A-TC4-C (2026-08-12)**: en modo manual / software-PID el detector pasa a **dos etapas**: a los 120 s emite `ERR probe_stuck_warning` en el wire **sin latchar** (un final de tostado legítimamente lento puede sostener BT plano 2 min a baja potencia), y solo a los **300 s** (`PROBE_STUCK_MANUAL_LATCH_SECS`) escala a la emergency (`ERR safety_fault Probe stuck`). El modo firmware-PID conserva el latch original de 120 s (el desarme por regulación cercana al target ya protege los holds sanos). El backstop de sonda muerta sigue cerrado en ambos modos: la exposición máxima en manual pasa de 2 a 5 min, muy por debajo de MAX_ROAST_TIME (30 min). Pins: `probe_stuck_manual_mode_two_stage_warns_then_latches` (in-crate), `s3_probe_stuck_manual_flat_bt_trips` (full_roast_verification, ahora verifica aviso a ~120 s + latch a ~300 s), `dead_probe_manual_roast_now_trips_probe_stuck` (harness, extendido a 300 s con parcheo de reloj sintético).

### OPERATIVO

#### S3 — `PID;CT` sin cota congela la regulación (RESUELTO)
- `parse_pid_subcommand` aceptaba `u32` sin tope (parser.rs:390-402); `PID;CT 4294967295` → `cycle_ms = 4294967295.max(10)` → el PID nunca volvía a computar; el heater mantenía el último output.
- **Fix**: cota `10..=60_000` ms en el parser (`PID;CT` fuera de rango → `ERR out_of_range`).
- **Repro**: `s3_pid_cycle_time_huge_freezes_regulation` (verifica la congelación pre-fix y el comportamiento vivo tras el fix).

#### S4 — Trampas internas absorben el fallo total del fan (RESUELTO)
- `emergency_shutdown` → `actuator.emergency_shutdown` ignoraba el resultado de `force_fan_100`: el Err devuelto era solo `EmergencyShutdown`, nada informaba de un fan muerto. Los paths de comando sí propagaban.
- **Fix**: `emergency_shutdown` ahora retorna `Err(HardwareError { source: "emergency_fan_failed" })` cuando el fan no alcanza 100 % tras `EMERGENCY_FAN_RETRIES` (actuator.rs), alineado con B-E/B-H/`stop_streaming` — el control loop emite ERR a Artisan ("no fan means unsafe to continue").
- **Repro**: `s4_internal_trap_absorbs_fan_failure` y test in-crate `emergency_shutdown_fan_total_failure_keeps_fan_output_honest` (ambos esperan ahora el nuevo error).

#### S6 — `OT2 0` con heater on anula el floor de fan hasta el siguiente tick (RESUELTO)
- El path de comando escribía el fan sin el floor `FAN_MIN_SAFETY_PCT`; ventana ~330 ms + hasta ~1.1 s de fade físico sin airflow.
- **Fix**: el floor se aplica también en el path de comando (`apply_policy_outcome`, branch de fan) siempre que `ssr_output > 0`; el floor del tick queda como segunda línea de defensa.
- **Repro**: `s6_ot2_zero_bypasses_fan_floor_until_next_tick` (ahora espera `fan_output == FAN_MIN_SAFETY_PCT` inmediato); harness I3 estricto (sin tick exento).

#### S8 — UART TX sin timeout (EC-A2, RESUELTO)
- `write_bytes` UART sin `with_timeout` vs USB 50+20 ms; un `flush()` colgado congelaba `dual_output_task` y todo el output.
- **Fix**: timeout simétrico de 50 ms (write) + 50 ms (flush) en `UartTxDriver::write_bytes` (uart/driver.rs) → `UartError::TransmissionError` a tiempo.

### LATENTE (invariantes no reforzados — ahora reforzados)

#### S5 — NaN envenena `ssr_output` y desarma los backstops (RESUELTO)
- `apply_guarded_heater(NaN)` aceptaba NaN (clamp/slew lo pasan) → `ssr_output = NaN` → comms-idle y MAX_ROAST_TIME inertes (`NaN > 0.0 == false`). Físicamente el heater quedaba OFF, pero la supervisión moría.
- **Fix**: guard `if !desired.is_finite() { return Err(InvalidState(non_finite_heater_output)) }` al inicio de `apply_guarded_heater` (actuator.rs). Fail-safe: en el loop → emergency; en comando → `ERR handler_failed`.
- **Repro**: `s5_nan_input_poisons_ssr_output_and_disarms_backstops` (des-ignorado, ahora pasa, incluye +Inf/−Inf); proptest `guarded_heater_output_stays_finite_and_clamped`.

#### S9 — Sentinel del SW watchdog: feed en t=0 desarma el timeout para siempre (RESUELTO)
- `LAST_FEED_MS == 0` era a la vez sentinel y timestamp real; un feed en el primer ms del baseline desarmaba el timeout para siempre.
- **Fix**: sentinel explícito `NEVER_FED = u64::MAX` (watchdog.rs: `LAST_FEED_MS` arranca y se reinicializa a `NEVER_FED`; guards `last != NEVER_FED` en `feed_async`/`is_alive`).
- **Repro**: `software_watchdog_times_out_after_missed_feeds` (con el sentinel fijo ya no necesita el priming para escapar la ventana; sigue pasando).

#### S10 — Formato READ trunca números finitos enormes → token corrupto en el wire (RESUELTO)
- `normalize_read_value` solo mapeaba no-finitos a 0.0; un f32 finito enorme formatea ~40 chars y el buffer `HeaplessString<REPORT_BUFFER_SIZE>` truncaba a mitad de número (`0.0,424044142750642532820598128640.0,-`).
- **Fix**: `normalize_read_value` clampa finitos a ±1000.0 (artisan.rs) — todo token emitido queda corto y parseable.
- **Repro**: `src/output/artisan.rs::format_read_never_panics_with_hostile_status` (des-ignorado, ahora pasa).

### MENOR / TELEMETRÍA

#### S7 — `ssr_output = 0.0` incondicional en emergency con SSR posiblemente atascado (RESUELTO)
- El campo honesto es `ssr_hardware_status = Error` (actuator.rs:143). Tras el fix, `ssr_output` solo se escribe a 0.0 si `force_heater_off` tuvo éxito; si el heater no pudo apagarse, la telemetría conserva el último duty aplicado y el status de hardware dice `Error`.
- **Repro**: T1 `heater_write_failure_mid_roast_escalates_to_latched_emergency` (ahora espera `ssr_output > 0.0` + `ssr_hardware_status == Error`).

---

## 4. Decisiones de diseño registradas

| # | Decisión | Estado |
|---|---|---|
| S2 | START/PREHEAT/OFF desarman el latch de emergencia y re-energizan (whitelist roaster_control.rs:1004-1019, 1125-1127). Es la puerta de recovery del operador (Bug P3) | **Mantener** (compatibilidad con el flujo manual de Artisan; el latch no es una barrera contra el host serial por diseño). Documentar en PROTOCOL.md: "el host serial es el operador; la seguridad física depende de supervisión humana + backstops temporales". Repro: `s2_serial_start_clears_latched_emergency_and_reenergizes` |
| S1 | Manual mode confía en el operador | **Corregido** con el detector probe-stuck a cualquier duty (fail-safe); la exposición de 2 min a BT plano con heater on queda cubierta |
| S4 | Trampas internas no escalaban el fallo de fan | **Corregido**: ahora propagan `HardwareError(emergency_fan_failed)` |

---

## 5. Checklist hardware (Fase 5 — placa sin instrumentos)

Pendiente de ejecución con la placa real (no requiere osciloscopio):
1. **Pull-down en la entrada del SSR (GPIO10)**: medir con multímetro que la línea está en LOW entre reset y `init.rs:157` (ventana flotante; el único punto donde el heater podría encenderse). Si no hay pull-down externo → CRÍTICO-SEGURIDAD hardware.
2. **RWDT**: prueba de hang deliberado mid-roast → confirmar reset ~2.2 s y heater off tras reset (LEDC no sobrevive reset).
3. **Monitor SSR**: readback LEDC con SSR real (margen 128 ticks); stuck-on con SSR de prueba si se tiene.
4. **Red de GPIO9** (strap): verificación visual de 10 kΩ pull-up + 1 kΩ serie + 10 kΩ gate pull-down (pinout.md:253-270).
5. **Realimentación del detector de presencia**: confirmar que está en el lado de carga del SSR (si es del lado de entrada, el detector es ciego a SSR atascado-on).
6. **Clippy sobre riscv32**: `cargo clippy --target riscv32imc-unknown-none-elf --features embedded -- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic` (los módulos hardware no pasan gate de clippy en CI; `assert_eq!` de pines en init.rs:78-104 compilan a producción).

---

## 6. Fixes aplicados (2026-08-05)

| # | Fix | Archivo | Verificación |
|---|---|---|---|
| S1 | probe-stuck armado a cualquier duty > 0 | src/control/roaster_control.rs | harness 0/1000 + `dead_probe_manual_roast_now_trips_probe_stuck` |
| S3 | `PID;CT` cota 10..=60_000 ms | src/input/parser.rs | `s3_pid_cycle_time_huge_freezes_regulation` |
| S4 | trampas internas propagan fallo de fan | src/control/controllers/actuator.rs | `s4_internal_trap_absorbs_fan_failure` + test in-crate |
| S5 | `!desired.is_finite()` → Err en `apply_guarded_heater` | src/control/controllers/actuator.rs | `s5_nan_input_poisons_ssr_output_and_disarms_backstops` (verde) |
| S6 | floor `FAN_MIN_SAFETY_PCT` en path de comando de fan | src/control/roaster_control.rs | `s6_ot2_zero_bypasses_fan_floor_until_next_tick` (verde) + I3 estricto |
| S7 | `ssr_output` honesto en emergency (solo 0 si `force_heater_off` OK) | src/control/controllers/actuator.rs | T1 (espera `ssr_output > 0` + status Error) |
| S8 | timeout 50+50 ms en UART TX | src/hardware/uart/driver.rs | (sin test host — driver riscv32-only) |
| S9 | sentinel `NEVER_FED = u64::MAX` en SW watchdog | src/safety/watchdog.rs | `software_watchdog_times_out_after_missed_feeds` |
| S10 | clamp ±1000 en `normalize_read_value` | src/output/artisan.rs | proptest `format_read_never_panics_with_hostile_status` (verde) |

**Re-verificación post-fix (sin nuevos bugs)**: suite completa 674/674 (0 failed, 0 ignored), clippy estricto 0 warnings, fmt limpio, build embedded release OK, harness 1000 roasts aleatorios con 0 violaciones de las 7 invariantes.

---

*Este informe es el entregable del bug hunt. Los fixes se aplicaron el 2026-08-05 por instrucción del usuario y se verificaron con los tests de reproducción (rojos pre-fix, verdes post-fix). Tests: `tests/safety_repro_tests.rs`, `tests/safety_injection_midroast_tests.rs`, `tests/safety_invariant_harness.rs`, proptests en `src/input/parser.rs`, `src/control/pid.rs`, `src/control/controllers/actuator.rs`, `src/hardware/sensors/simulated.rs`, `src/output/artisan.rs`, tests de transporte en `src/hardware/transport_tasks.rs`.*
