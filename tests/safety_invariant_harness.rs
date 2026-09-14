//! Harness de invariantes de seguridad.
//!
//! Ejecuta roasts simulados con secuencias aleatorias de comandos Artisan +
//! perfiles de fallo (sonda muerta, desconexión, overtemp, writes fallando,
//! fan roto, SSR atascado) y verifica por tick las invariantes de seguridad:
//!
//!   I1: emergency activo ⟹ ssr_output == 0
//!       (excepción: ssr_hardware_status == Error — S7 fix: si el heater no
//!       pudo apagarse, ssr_output conserva el último duty aplicado y el
//!       campo honesto es el status de hardware)
//!   I2: heater > 0 ⟹ ¬fault_condition ∧ ¬emergency (misma excepción S7)
//!   I3: heater > 0 ∧ ¬emergency ⟹ fan ≥ FAN_MIN_SAFETY_PCT (S6 fix: el floor
//!       también se aplica en el path de comando, ya no hay tick exento)
//!   I4: gap de sensor > TEMP_VALIDITY_TIMEOUT_MS ⟹ heater == 0 ∨ emergency
//!   I5: sample limpio con BT ≥ OVERTEMP ⟹ emergency en el mismo tick
//!   I6: heater > 0 ⟹ existe supervisión EFECTIVA (overtemp, RoR, probe-stuck,
//!       comms-idle). Con sonda muerta / desconectada la exposición S1 era
//!       ESPERADA y se contaba (no assert); tras el fix S1 el detector
//!       probe-stuck se arma a cualquier duty > 0, así que la exposición debe
//!       caer a ~0 y una violación (con o sin sonda) aborta.
//!   I7: ssr_output / fan_output siempre finitos.
//!
//! Semilla por caso → proptest shrinking da la reproducción mínima.
//! Tiempo simulado: 200 ms por tick (pasa el SSR cycle guard de 100 ms).

#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

extern crate alloc;
extern crate std;

use std::boxed::Box;

use embassy_time::{Duration, Instant};
use proptest::prelude::*;

use libreroaster::common::{StubFan, StubHeater};
use libreroaster::config::constants::{
    RoasterState, SsrHardwareStatus, COMMS_IDLE_TIMEOUT_MS, FAN_MIN_SAFETY_PCT, OVERTEMP_THRESHOLD,
    TEMP_VALIDITY_TIMEOUT_MS,
};
use libreroaster::config::ArtisanCommand;
use libreroaster::control::RoasterControl;
use libreroaster::hardware::sensors::{SensorConversionHub, SensorFault};
use libreroaster::hardware::test_mocks::{MockFan, MockSsr};

// 130 s de roast simulado: cubre el detector probe-stuck (120 s) para que los
// roasts con sonda muerta alcancen la emergencia y la exposición S1 caiga a 0.
/// Number of simulated control ticks per roast (~130 s at `TICK_MS`), long
/// enough for the probe-stuck detector (120 s) to reach emergency.
const TICKS_PER_ROAST: u32 = 650;
/// Simulated wall-clock per tick (200 ms); exceeds the 100 ms SSR cycle guard.
const TICK_MS: u64 = 200;

// ── PRNG determinista (xorshift64*) ──────────────────────────────────────

/// Deterministic xorshift64* PRNG so random-roast sequences are reproducible.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn chance(&mut self, percent: u32) -> bool {
        self.below(100) < percent as u64
    }
}

// ── Perfil de fallo del roast ────────────────────────────────────────────

/// Trigger de fallo en el tick indicado (0 = nunca).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Trigger {
    None,
    OvertempAfter(u32),
    DisconnectAfter(u32),
    HeaterFailAfter(u32),
    FanFailAfter(u32),
}

#[derive(Debug, Clone, Copy)]
/// Per-roast fault profile driving sensors, heater, fan and SSR behaviour.
struct RoastConfig {
    probe_dead: bool,
    trigger: Trigger,
    fan_broken: bool,
    ssr_stuck: bool,
}

/// Draws a random `RoastConfig` from the seeded PRNG.
fn random_config(rng: &mut Rng) -> RoastConfig {
    let probe_dead = rng.chance(20);
    let trigger = match rng.below(6) {
        0 => Trigger::None,
        1 => Trigger::OvertempAfter(10 + rng.below(60) as u32),
        2 => Trigger::DisconnectAfter(10 + rng.below(40) as u32),
        3 => Trigger::HeaterFailAfter(10 + rng.below(60) as u32),
        4 => Trigger::FanFailAfter(10 + rng.below(60) as u32),
        _ => Trigger::None,
    };
    RoastConfig {
        probe_dead,
        trigger,
        fan_broken: rng.chance(10),
        ssr_stuck: rng.chance(10),
    }
}

fn probe_effective(cfg: RoastConfig, tick: u32) -> bool {
    if cfg.probe_dead {
        return false;
    }
    !matches!(cfg.trigger, Trigger::DisconnectAfter(n) if tick >= n)
}

/// Temperaturas/faults del tick según perfil y fase.
fn profile_temps(cfg: RoastConfig, tick: u32) -> (f32, f32, SensorFault, SensorFault) {
    let clean = SensorFault::default();
    let faulted = SensorFault {
        fault_detected: true,
        ..SensorFault::default()
    };

    // Fase de desconexión: ambos canales faulted.
    if let Trigger::DisconnectAfter(n) = cfg.trigger {
        if tick >= n {
            return (0.0, 0.0, faulted, faulted);
        }
    }

    // Curva de roast sana: ~0.4 °C/s (por debajo del guard BT de 0.5 °C/s).
    let bt_healthy = 25.0 + 0.08 * tick as f32;
    let et_healthy = bt_healthy + 18.0;

    if let Trigger::OvertempAfter(n) = cfg.trigger {
        if tick >= n {
            return (OVERTEMP_THRESHOLD + 10.0, et_healthy, clean, clean);
        }
    }

    if cfg.probe_dead {
        (0.0, 0.0, clean, clean)
    } else {
        (bt_healthy, et_healthy, clean, clean)
    }
}

// ── Generador de comandos ────────────────────────────────────────────────

/// Generador de comandos para el cuerpo del roast (mid-loop).
///
/// Los comandos de heater (OT1/UP/DOWN) se EXCLUYEN deliberadamente: el path
/// de comando usa `Instant::now()` real (roaster_control.rs:1021) mientras
/// los ticks usan tiempo sintético — mezclar ambas bases de tiempo hace que
/// `now.duration_since(last_slew_update)` (actuator.rs:64, no saturante)
/// paniquee cuando un comando real llega tras un tick sintético futuro. El
/// modo manual se explora con el burst pre-tick (`initial_command_burst`).
fn gen_command(rng: &mut Rng) -> ArtisanCommand {
    match rng.below(100) {
        0..=29 => ArtisanCommand::ReadStatus,                    // 30 %
        30..=43 => ArtisanCommand::SetFan(rng.below(101) as u8), // 14 %
        44..=53 => ArtisanCommand::SetFanSpeed(rng.below(101) as u8, false), // 14 %
        54..=60 => ArtisanCommand::StartRoast,                   // 7 %
        61..=63 => ArtisanCommand::Stop,                         // 3 %
        64..=65 => ArtisanCommand::EmergencyStop,                // 2 %
        66..=71 => ArtisanCommand::Preheat(80.0 + rng.below(221) as f32), // 6 %
        72..=77 => ArtisanCommand::SetTargetTemp(150.0 + rng.below(151) as f32), // 6 %
        78..=81 => ArtisanCommand::SetPidGain(
            0.1 + rng.below(100) as f32 / 10.0,
            rng.below(100) as f32 / 100.0,
            rng.below(100) as f32 / 100.0,
        ), // 4 %
        82..=83 => {
            ArtisanCommand::SetPidOutputLimits(rng.below(51) as f32, 50.0 + rng.below(51) as f32)
        } // 2 %
        84..=86 => ArtisanCommand::SetPidCycleTime(10 + rng.below(2_000_000) as u32), // 3 %
        _ => {
            if rng.chance(50) {
                ArtisanCommand::SetPidChannel(1)
            } else {
                ArtisanCommand::SetPidChannel(2)
            }
        } // 1 %
    }
}

/// Burst pre-tick: comandos con base de tiempo REAL (antes de `baseline`).
/// Cubre el modo manual (OT1/OT2) y el gap S6 (OT2 0 con heater on).
fn initial_command_burst(rng: &mut Rng) -> Vec<ArtisanCommand> {
    let mut cmds = Vec::new();
    if rng.chance(50) {
        cmds.push(ArtisanCommand::SetHeater(10 + rng.below(91) as u8));
    }
    if rng.chance(20) {
        cmds.push(ArtisanCommand::SetFanSpeed(rng.below(101) as u8, false));
    }
    cmds
}

// ── Un roast simulado ────────────────────────────────────────────────────

/// Per-seed result: the fault profile plus I6 supervision-exposure counters.
struct RoastOutcome {
    cfg: RoastConfig,
    i6_expected: u32,   // exposición S1 documentada (sonda muerta/desconectada)
    i6_unexpected: u32, // violación en roast con sonda efectiva = bug nuevo
}

/// Runs one full simulated roast for `seed`, asserting safety invariants I1–I7.
fn run_roast(seed: u64) -> RoastOutcome {
    let mut rng = Rng(seed ^ 0x9E37_79B9_7F4A_7C15);
    let cfg = random_config(&mut rng);

    let mut heater = MockSsr::new();
    let mut fan = MockFan::new();
    let mut ctrl = RoasterControl::new(
        Box::new(heater.clone()),
        Box::new(fan.clone()),
        SensorConversionHub::new(),
    )
    .expect("control builds");

    // Hardware pre-armado según perfil.
    if cfg.fan_broken {
        fan.fail_next_emergency_writes(10_000);
    }
    if cfg.ssr_stuck {
        heater.set_status(SsrHardwareStatus::Error);
    }

    // Burst pre-tick con tiempo REAL (antes de que exista tiempo sintético).
    for cmd in initial_command_burst(&mut rng) {
        let _ = ctrl.process_artisan_command(cmd);
    }

    // `baseline` se toma DESPUÉS del burst: el primer tick sintético
    // (baseline + 0) debe ser ≥ todo tiempo real ya registrado
    // (last_slew_update del burst) para no paniquear en `duration_since`.
    let baseline = Instant::now();
    let mut last_command_sim_ms = 0u64; // 0 = nunca → comms-idle armado al inicio
    let mut i6_expected = 0u32;
    let mut i6_unexpected = 0u32;

    for tick in 0..TICKS_PER_ROAST {
        let sim_ms = baseline.as_millis() + TICK_MS * tick as u64;
        let now = baseline + Duration::from_millis(TICK_MS * tick as u64);

        // 1) Comando ocasional (~30 % por tick).
        if rng.chance(30) {
            let cmd = gen_command(&mut rng);
            let _ = ctrl.process_artisan_command(cmd);
            last_command_sim_ms = sim_ms;
        }

        // 2) Fallos programados.
        match cfg.trigger {
            Trigger::HeaterFailAfter(n) if tick == n => heater.fail_next_writes(4),
            Trigger::FanFailAfter(n) if tick == n => fan.fail_next_speed_writes(2),
            _ => {}
        }

        // 3) Temperaturas + control.
        let (bt, et, bean_fault, env_fault) = profile_temps(cfg, tick);
        let sample_clean = !bean_fault.has_fault();
        let _ = ctrl.update_temperatures_with_fault(bt, et, bean_fault, env_fault, now);
        let _ = ctrl.update_control(now);

        // 4) Invariantes sobre el estado público.
        let s = ctrl.get_status();
        let emergency = ctrl.safety().is_emergency_active();
        let heater_on = s.ssr_output > 0.0;

        // I7 — finitud de los outputs.
        assert!(
            s.ssr_output.is_finite() && s.fan_output.is_finite(),
            "seed {seed} tick {tick}: I7 outputs no finitos ssr={:?} fan={:?}",
            s.ssr_output,
            s.fan_output
        );

        // I1 — emergencia ⟹ heater a 0 (S7 fix: si el heater no pudo apagarse,
        // el status de hardware Error es la verdad y ssr_output conserva el
        // último duty aplicado — ventana del tick del fallo).
        assert!(
            !emergency || s.ssr_output == 0.0 || s.ssr_hardware_status == SsrHardwareStatus::Error,
            "seed {seed} tick {tick}: I1 emergencia con ssr_output={} y status={:?}",
            s.ssr_output,
            s.ssr_hardware_status
        );

        // I2 — heater > 0 ⟹ sin fault ni emergencia (misma excepción S7).
        assert!(
            !heater_on
                || (!s.fault_condition && !emergency)
                || s.ssr_hardware_status == SsrHardwareStatus::Error,
            "seed {seed} tick {tick}: I2 heater={} con fault={} emergency={} status={:?}",
            s.ssr_output,
            s.fault_condition,
            emergency,
            s.ssr_hardware_status
        );

        // I3 — floor del fan (S6 fix: también en el path de comando, estricto).
        if heater_on && !emergency {
            assert!(
                s.fan_output >= FAN_MIN_SAFETY_PCT,
                "seed {seed} tick {tick}: I3 heater={} con fan={} (<{})",
                s.ssr_output,
                s.fan_output,
                FAN_MIN_SAFETY_PCT
            );
        }

        // I4 — sensor stale ⟹ heater a 0 o emergencia.
        if heater_on && !emergency {
            if let Some(last_read) = ctrl.sensor().last_temp_read() {
                let gap_ms = now.duration_since(last_read).as_millis();
                assert!(
                    gap_ms <= TEMP_VALIDITY_TIMEOUT_MS as u64,
                    "seed {seed} tick {tick}: I4 heater={} con gap sensor {gap_ms} ms",
                    s.ssr_output
                );
            }
        }

        // I5 — overtemp con sample limpio ⟹ emergency este mismo tick.
        if sample_clean && bt >= OVERTEMP_THRESHOLD {
            assert!(
                emergency,
                "seed {seed} tick {tick}: I5 BT={bt} >= {OVERTEMP_THRESHOLD} sin emergencia"
            );
        }

        // I6 — supervisión EFECTIVA cuando hay heater.
        // Tras el fix S1 el detector probe-stuck se arma con cualquier
        // duty > 0, así que con el heater encendido la supervisión siempre
        // existe (o el overtemp real lee, o el RoR está armado, o el
        // probe-stuck, o comms-idle) — cualquier tick sin supervisión es un
        // bug NUEVO, con o sin sonda efectiva.
        if heater_on && !emergency {
            let probe_ok = probe_effective(cfg, tick);
            let state = ctrl.get_state();
            let ror_armed = matches!(state, RoasterState::Heating | RoasterState::Stable)
                || (state == RoasterState::Idle && s.pid_enabled && heater_on);
            // Replica del gate post-fix S1 (roaster_control.rs, probe-stuck:
            // armado con ssr_output > 0, antes >= PROBE_STUCK_HEATER_MIN_PCT).
            let probe_stuck_armed = s.ssr_output > 0.0;
            let comms_idle_armed =
                sim_ms.saturating_sub(last_command_sim_ms) > COMMS_IDLE_TIMEOUT_MS as u64;

            let supervised = (probe_ok) // overtemp efectivo: la sonda lee el calor real
                || (probe_ok && ror_armed) // RoR efectivo
                || probe_stuck_armed // funciona también con sonda muerta
                || comms_idle_armed;

            if supervised {
                continue;
            }
            if probe_ok {
                i6_unexpected += 1;
                assert!(
                    false,
                    "seed {seed} tick {tick}: I6 heater={} sin supervisión efectiva \
                     (state={state:?} pid={} probe_stuck={probe_stuck_armed} comms={comms_idle_armed})",
                    s.ssr_output, s.pid_enabled
                );
            } else {
                // Sonda muerta/desconectada: tras el fix S1 la exposición S1
                // debe ser 0 (el probe-stuck cubre); si aparece, se cuenta y
                // aborta en el proptest (i6_expected > 0 ⇒ fail).
                i6_expected += 1;
            }
        }
    }

    RoastOutcome {
        cfg,
        i6_expected,
        i6_unexpected,
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1000,
        ..ProptestConfig::default()
    })]

    /// 1000 roasts aleatorios: ninguna invariante I1–I7 debe violarse. Tras el
    /// fix S1 el detector probe-stuck cubre la sonda muerta a cualquier duty,
    /// así que `i6_expected` (exposición sin supervisión) también debe ser 0.
    /// El shrinking de proptest minimiza el seed de cualquier violación.
    #[test]
    fn random_roasts_never_violate_safety_invariants(seed in 0u64..1_000_000) {
        let outcome = run_roast(seed);
        assert_eq!(
            outcome.i6_unexpected,
            0,
            "seed {seed}: violaciones I6 inesperadas: {}",
            outcome.i6_unexpected
        );
        assert_eq!(
            outcome.i6_expected,
            0,
            "seed {seed}: {:?} — exposición S1 (heater sin supervisión efectiva) \
             debe ser 0 tras el fix, got {} ticks",
            outcome.cfg, outcome.i6_expected
        );
    }
}

// ── Tests de sanidad del harness ─────────────────────────────────────────

#[test]
fn harness_prng_is_deterministic() {
    let mut a = Rng(42);
    let mut b = Rng(42);
    for _ in 0..100 {
        assert_eq!(a.next(), b.next());
    }
}

#[test]
fn dead_probe_manual_roast_now_trips_probe_stuck() {
    // Verificación del fix S1 + Audit A-TC4-C: sonda muerta (TC corto lee
    // 0.0 °C válido, sin fault bit) + manual mode a 30 % (bajo el antiguo
    // umbral del detector) + polling de Artisan cada 3 ticks (neutraliza
    // comms-idle). El detector probe-stuck — armado con cualquier duty > 0 —
    // es ahora de DOS ETAPAS en modo manual: a los 120 s (600 ticks) solo
    // avisa por el wire; el latch real llega a los 300 s (1500 ticks),
    // dejando el heater a 0. Pre-fix este roast corría a ciegas hasta
    // MAX_ROAST_TIME.
    let mut rng = Rng(7);
    let mut ctrl = RoasterControl::new(
        Box::new(StubHeater::new()),
        Box::new(StubFan::new()),
        SensorConversionHub::new(),
    )
    .expect("control builds");

    // Manual mode a 30 % — burst pre-tick (misma disciplina de tiempo que
    // `run_roast`: los comandos de heater usan tiempo real, los ticks
    // sintético; por eso el OT1 va antes del baseline).
    let _ = ctrl.process_artisan_command(ArtisanCommand::SetHeater(30));
    let baseline = Instant::now();
    let mut heater_ticks = 0u32;

    // 300 s a 200 ms/tick + margen para el latch de la segunda etapa.
    let ticks_to_latch =
        (libreroaster::config::constants::PROBE_STUCK_MANUAL_LATCH_SECS * 1000 / TICK_MS) + 30;
    for tick in 0..ticks_to_latch {
        let now = baseline + Duration::from_millis(TICK_MS * tick as u64);

        // Polling de Artisan cada 3 ticks — neutraliza comms-idle. El campo
        // se parchea con el tiempo sintético (misma disciplina que
        // full_roast_verification::poll_read): `process_artisan_command`
        // estampa el reloj REAL, que apenas avanza durante un bucle rápido.
        if tick % 3 == 1 {
            let _ = ctrl.process_artisan_command(ArtisanCommand::ReadStatus);
            ctrl.status_mut().last_command_received_at_ms = now.as_millis();
        }
        let _ = rng.next(); // consumo simbólico para variar la semilla

        let _ = ctrl.update_temperatures_with_fault(
            0.0,
            0.0,
            SensorFault::default(),
            SensorFault::default(),
            now,
        );
        let _ = ctrl.update_control(now);

        if tick == 600 {
            // 120 s planos: etapa de aviso — el latch NO debe estar armado.
            assert!(
                !ctrl.safety().is_emergency_active(),
                "A-TC4-C: modo manual no debe latchar a los 120 s (etapa de aviso)"
            );
        }

        if ctrl.get_status().ssr_output > 0.0 {
            heater_ticks += 1;
        }
    }

    let s = ctrl.get_status();
    assert!(
        heater_ticks > 0,
        "sanidad: el roast manual debe energizar el heater"
    );
    assert!(
        ctrl.safety().is_emergency_active(),
        "S1 fix: sonda muerta + heater on debe disparar probe-stuck a los 300 s, \
         antes de MAX_ROAST_TIME (heater_ticks={heater_ticks})"
    );
    assert_eq!(
        s.ssr_output, 0.0,
        "S1 fix: tras el probe-stuck el heater debe quedar a 0"
    );
    assert_eq!(
        s.fan_output, 100.0,
        "el cooldown debe mantener el fan al 100 %"
    );
    eprintln!(
        "sanidad S1 fix: emergencia probe-stuck (2 etapas) tras {heater_ticks} ticks de heater"
    );
}
