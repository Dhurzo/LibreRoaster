# BUG-CATCH-PLAN — Caza de bugs críticos: seguridad operativa de tostado

**Fecha:** 2026-08-05
**Estado:** EN EJECUCIÓN
**Objetivo:** Comprobar que esta versión puede tostar café de forma segura. Encontrar y confirmar (con reproducción) los bugs **bloqueantes** (impiden tostar) y de **seguridad operativa** (riesgo de sobrecalentamiento, cortes de emergencia que no disparan, watchdog inoperante). El resto de mejoras queda para después.

---

## 1. Alcance y criterios de salida

- **Dentro:** bugs BLOQUEANTES y CRÍTICO-SEGURIDAD + OPERATIVOS que afecten un roast real. Verificación por múltiples aproximaciones independientes para minimizar falsos negativos.
- **Fuera:** mejoras no críticas, refactors, persistencia, perfiles, telemetría (salvo que interfieran con la seguridad).
- **Entregables:**
  1. `docs/SAFETY_BUGS.md` — lista rankeada con severidad, evidencia file:line, reproducción (test o procedimiento), fix sugerido, y decisiones de diseño registradas.
  2. Test de reproducción para cada bug confirmado (rojo pre-fix, verde post-fix).
  3. Veredicto final: "apto para tostar" / "no apto hasta resolver X".
- **No se aplican fixes en esta iteración** (solo reproducciones + recomendaciones), salvo el commit del working tree pendiente (Fase 0) y decisiones explícitas del usuario.
- **Hardware:** placa disponible **sin instrumentos** → Fase 5 limitada a verificaciones sin osciloscopio (RWDT, monitor SSR, pull-down con multímetro, checklist visual de GPIO9).

## 2. Método de triaje

| Severidad | Definición | Ejemplo |
|---|---|---|
| **BLOQUEANTE** | Impide tostar: no arranca, brick, handshake roto, emergency permanente sin recovery | EC-A1 |
| **CRÍTICO-SEGURIDAD** | Riesgo de sobrecalentamiento/incendio: heater sin supervisión, corte no dispara, reset inoperante | S1 (sonda muerta en manual) |
| **OPERATIVO** | Interrumpe/degrada el roast sin peligro físico | S3 (PID;CT congela regulación) |

## 3. Estrategia anti-falsos-negativos

Seis aproximaciones complementarias; un bug solo se acepta con prueba de reproducción o procedimiento de verificación:

1. **Fase 1 — Verificación estática dirigida:** los candidatos ya detectados en auditorías previas (S1–S8), cada uno con test de reproducción.
2. **Fase 2 — Inyección de fallos dirigida:** ampliar mocks con modos de fallo realistas y tests de fallo a mitad de roast.
3. **Fase 3 — Fuzzing/propiedades:** proptest hostil sobre parser, PID, curva, actuador, formatters.
4. **Fase 4 — Harness de invariantes:** roasts simulados con flujos aleatorios de comandos + fallos; invariantes de seguridad verificadas cada tick. (Pieza central.)
5. **Fase 5 — Embedded/hardware:** verificación en placa sin instrumentos + clippy riscv32 (hoy sin gate).
6. **Fase 6 — Consistencia doc-código y proceso.**

## 4. Candidatos estáticos a verificar (Fase 1)

| # | Severidad | Candidato | Evidencia |
|---|---|---|---|
| **S1** | CRÍTICO-SEGURIDAD (candidato) | Manual mode sin supervisión de sensor con sonda muerta finita (TC corto ≈ 0 °C válido, sin fault bit): overtemp/RoR/staleness no disparan; probe-stuck solo arma con heater ≥ 50 %; comms-idle neutralizado por polling READ (resetea `last_command_received_at_ms` con cada comando) → OT1 < 50 % con sonda muerta = heater a ciegas hasta MAX_ROAST_TIME (30 min) | `sensor.rs:278-280`; `roaster_control.rs:917-952, 519-535, 992-994` |
| **S2** | HIGH (diseño) | Whitelist: `START`/`PREHEAT`/`OFF` desarman el latch de emergencia (`clear_emergency_explicit`) y re-energizan durante `fault_condition` | `roaster_control.rs:1004-1019, 1048-1050, 1123-1127, 1522-1524` |
| **S3** | OPERATIVO | `PID;CT` acepta u32 sin cota → `PID;CT 4294967295` congela el throttling, heater mantiene último output sin regulación | `parser.rs:390-402`; `roaster_control.rs:1673` |
| **S4** | OPERATIVO | Emergencia por trampa interna (overtemp/NaN/RoR) absorbe fallo total de fan en silencio (solo logs); los paths de comando sí propagan | `actuator.rs:188-189` vs `roaster_control.rs:1270-1278` |
| **S5** | LATENTE | NaN en `apply_guarded_heater` envenenaría `ssr_output = NaN` y desarmaría comms-idle/max-roast (`NaN > 0.0` = false). Hoy inalcanzable; invariante no reforzado | `actuator.rs:42-91`; `roaster_control.rs:519, 580` |
| **S6** | OPERATIVO (bajo) | Interlock fan: `OT2 0` con heater on escribe 0 directamente en el path de comando sin el floor FAN_MIN_SAFETY_PCT hasta el siguiente tick (~330 ms + fade hasta ~1.1 s) | `roaster_control.rs:314-332` vs `882-889`; `fan.rs:60-63, 94-109` |
| **S7** | MENOR | `status.ssr_output = 0.0` incondicional en emergency con SSR posiblemente atascado on (campo honesto: `ssr_hardware_status = Error`) | `actuator.rs:178, 143` |
| **S8** | OPERATIVO | UART TX sin timeout (asimetría con USB 50+20 ms) → `dual_output` congelado cuelga todo el output | `uart/driver.rs:49-58` vs `usb_cdc/driver.rs:70-89` (EC-A2) |

## 5. Fases de ejecución

### Fase 0 — Baseline y proceso (~0.5 d)
1. Quality gates: `cargo fmt --all -- --check` + clippy estricto + `cargo test --target x86_64-unknown-linux-gnu --features test --lib --tests` + build embedded release.
2. **EC-A1 (verificar):** el diff P-TC4 (handshake `#OK`, delimitadores) pudo quedar sin commitear; un build limpio fallaría el handshake de Artisan → READ nunca funciona → no se puede tostar. Verificar estado real de HEAD vs working tree y **commitear lo pendiente** para que la versión auditada sea reproducible.
3. Verificar empíricamente cualquier sospecha de BLOQUEANTE con stash + build.

### Fase 1 — Verificación de candidatos estáticos (~1–1.5 d)
Cada candidato S1–S8 (tabla §4) con test de reproducción en el suite host (`--features test`), excepto S7 (documentar). Para S1/S2: test que documenta el comportamiento + análisis de decisión de diseño.

### Fase 2 — Inyección de fallos dirigida (~1–1.5 d)
- Ampliar `src/hardware/test_mocks.rs`: `emergency_set_speed` fallible (hoy imposible, `:136-139`), fallo N-de-M intermitente, lecturas NaN, `get_status` configurable (SSR stuck), `RoastCurve` con fallos mid-roast.
- Tests nuevos:
  1. Fallo de write de heater a mitad de roast → escalado a emergency + heater off (path Bug B / EC-23, hoy sin cobertura).
  2. Desconexión de sensor mid-roast (exhaustión de 5 fallbacks → corte ~1.65 s).
  3. Timeout del SW watchdog (rama `watchdog.rs:78-81`, hoy sin test) — con reloj real (sleep ~1.1 s) o clock injection.
  4. Interleaving byte a byte USB+UART (gap `TESTING.md:180`).
  5. Fan roto + latch → reintento de fan 100 % cada tick (`roaster_control.rs:866-901`).

### Fase 3 — Fuzzing y propiedades (~1 d)
- Parser: bytes no-UTF8, NUL embebidos, delimitadores raros, números enormes (`1e39`, `u32::MAX`), prefijos ambiguos (`OT10`, `PIDOFF`, `#DUMP;garbage`). Invariante: nunca panic + nunca `SetHeater > 100` desde un no-comando.
- PID: PV y targets NaN/±Inf/1e38 → output finito clamped [0,100] (hoy solo rangos finitos en `pid.rs:327-429`).
- `RoastCurve::temperatures_at` con waypoints arbitrarios (zero-range fue bug real C4b).
- Actuador con f32 arbitrarios → `ssr_output`/`fan_output` nunca NaN (refuerza S5).
- Formatters con `SystemStatus` NaN/Inf.

### Fase 4 — Harness de invariantes con flujos aleatorios (~2 d) — pieza central
- Driver host: roasts simulados completos con secuencias aleatorias de comandos Artisan + fallos inyectados; N=1000, seed fijo reproducible + semillas variables.
- Fuentes de eventos: `OT1 n`, `OT2 n`, `UP`/`DOWN`, `START`, `STOP`, `PREHEAT t`, `PID;SV t`, `PID;OFF`, `SETTARGET`, `READ`, `STATUS`, `PROFILE`, `FANPROFILE`, garbage; fallos: sensor desconectado N ticks, sensor congelado finito, writes N-de-M, fan roto, OVERTEMP+10.
- Invariantes por tick (sobre `SystemStatus` + estado del harness):
  1. `emergency_latch ⟹ ssr_output == 0` (incluye el mismo tick del trap).
  2. `heater > 0 ⟹ ¬fault_condition ∧ ¬emergency`.
  3. `heater > 0` en estado estacionario ⟹ `fan ≥ FAN_MIN_SAFETY_PCT` (tolerancia 1 tick, S6).
  4. `¬sensor_fresh > 1 s ⟹ heater == 0` en PID.
  5. `BT ≥ OVERTEMP ⟹ emergency` en ≤ 1 tick.
  6. **`heater > 0 ⟹ existe supervisión activa`** (overtemp ∨ RoR ∨ probe-stuck ∨ staleness ∨ comms-idle ∨ max-roast) — detecta S1 sistemáticamente.
  7. `ssr_output`/`fan_output` siempre finitos (S5).
- Cada violación → caso mínimo reproducible (seed + secuencia de eventos).

### Fase 5 — Embedded/hardware sin instrumentos (~1 d, placa disponible)
1. **Pull-down entrada SSR (GPIO10):** medición con multímetro en el circuito real, SSR desconectado. La ventana reset→`init.rs:157` deja GPIO10 flotante; sin pull-down externo el heater puede encenderse en boot → CRÍTICO-SEGURIDAD hardware documentado. El firmware no puede arreglarlo; el plan documenta la exigencia y el procedimiento.
2. **RWDT:** test de hang deliberado mid-roast → confirmar reset ~2.2 s y heater off tras reset (LEDC no sobrevive reset).
3. **Monitor SSR:** readback LEDC con SSR real; stuck-on (duty 0 + heat ×10) con SSR de prueba si se dispone.
4. **GPIO9 strap:** verificación visual de la red (10 kΩ pull-up + 1 kΩ serie + 10 kΩ gate pull-down, `pinout.md:253-270`) — sin osciloscopio queda como checklist.
5. **Clippy riscv32:** `cargo clippy --target riscv32imc-unknown-none-elf --features embedded` — los módulos hardware (init.rs, ssr.rs, fan.rs, ledc_bus.rs) no pasan ningún gate de clippy; los `assert_eq!` de pines de `init.rs:78-104` compilan a producción.

### Fase 6 — Consistencia doc-código y proceso (~0.5 d)
- Cross-check PROTOCOL.md ↔ parser/handlers (discrepancias menores conocidas: EC-A4 cadencia telemetría, EC-A3 drops de canal de salida).
- Actualizar `docs/EDGE_CASES_DETECTED.md` con el estado real de los hallazgos abiertos (EC-A1…EC-A6).

## 6. Estimación y orden

| Fase | Duración |
|---|---|
| 0. Baseline + proceso | 0.5 d |
| 1. Verificación de candidatos | 1–1.5 d |
| 2. Inyección de fallos | 1–1.5 d |
| 3. Fuzzing/propiedades | 1 d |
| 4. Harness de invariantes | 2 d |
| 5. Embedded/hardware | 1 d (sin instrumentos) |
| 6. Doc-código | 0.5 d |
| **Total** | **~7–8 d** |

**Orden:** 0 → 1 (S1/S2/S3 primero) → 2 → 4 (3 en paralelo) → 6 → 5.

## 7. Decisiones tomadas

- ✅ Commit del working tree pendiente en Fase 0 (versión auditada reproducible) — aprobado por el usuario.
- ✅ Solo informe + tests de reproducción; sin fixes en esta iteración — aprobado por el usuario.
- ✅ Fase 5 limitada a verificaciones sin instrumentos (placa sí, osciloscopio no) — aprobado por el usuario.
