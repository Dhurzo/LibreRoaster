# Control Loop & Event Management

> Technical documentation of the main control loop and command/event handling in LibreRoaster v0.0.1 Alpha.

---

## 1. Overview

The firmware runs on a **single‑thread cooperative model** (Embassy executor on ESP32‑C3 single‑core). The "heartbeat" is the `control_loop_task`, which executes a tick every ~310‑330 ms (100 ms nominal timer + 210 ms MAX31856 conversion wait).

```
┌─────────────────────────────────────────────────────────────────┐
│                    CONTROL_LOOP_TASK                            │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐            │
│  │ Drain Cmds   │ │ Read Sensors │ │ Update Ctrl  │  ...       │
│  │ (0ms)        │ │ (210ms await)│ │ (1‑2ms)      │            │
│  └──────────────┘ └──────────────┘ └──────────────┘            │
│         │               │               │                        │
│         ▼               ▼               ▼                        │
│  artisan_channel  MAX31856×2      RoasterControl                │
│  (MPSC cap=16)    (async SPI)     (Mutex<RefCell<...>>)        │
└─────────────────────────────────────────────────────────────────┘
         │               │               │
         ▼               ▼               ▼
    output_channel  SystemStatus    Actuators (SSR/Fan)
    (MPSC cap=32)   (snapshot)      (LEDC HW)
         │
         ▼
   dual_output_task (5ms) → USB/UART → Artisan PC
```

---

## 2. Control Loop Task

**File:** `src/application/tasks.rs:1041`

```rust
#[embassy_executor::task]
pub async fn control_loop_task() {
    let mut tick_state = TickState::new();
    let output_channel = ServiceContainer::get_output_channel();

    loop {
        control_loop_tick(&mut tick_state, output_channel).await;
        Timer::after(Duration::from_millis(CONTROL_LOOP_PERIOD_MS)).await; // 100 ms
    }
}
```

**Real cadence:** ~310‑330 ms (the `Timer::after(100 ms)` is re‑armed at the end; sensor reading blocks ~210 ms in `await`).

---

## 3. Tick Stages (`control_loop_tick`)

`src/application/tasks.rs:949` — executed **sequentially** within the same tick.

| # | Stage | Function | Approx. time | Description |
|---|-------|----------|--------------|-------------|
| 1 | **Drain Commands** | `drain_commands()` | ~0 ms | Empties `artisan_channel`, runs `RoasterControl::process_artisan_command()` |
| 2 | **Read Sensors** | `read_sensors()` | **~210 ms** | Reads both MAX31856 in parallel (async), fault debounce 5 ticks, updates `SystemStatus` |
| 3 | **Control Update** | `update_control_stage()` | ~1‑2 ms | Calls `RoasterControl::update_control()` — safety, PID/manual, SSR, fan, charge detection |
| 4 | **LEDC Guard** | `log_ledc_stage()` | ~0 ms | Logs zero‑cross guard timeouts |
| 5 | **Watchdog Feed** | `feed_watchdog_stage()` | ~0 ms | Feeds RTC WDT + BT heartbeat; 2 consecutive failures = emergency |
| 6 | **Telemetry** | `emit_telemetry_stage()` | ~1 ms | Continuous `#` stream @1 Hz + `#DUMP` ring‑buffer drain (4 rows/tick) |
| 7 | **Status LED** | `update_status_led_stage()` | ~0 ms | Pattern based on `RoasterState` + `fault_condition` |

### 3.1 Drain Commands

```rust
// src/application/tasks.rs:222
async fn drain_commands(tick_state: &mut TickState) {
    let cmd_channel = ServiceContainer::get_artisan_channel();
    while let Ok(traced_command) = cmd_channel.try_receive() {
        // RunRegression triggers regression_task before normal handler
        if matches!(traced_command.command, ArtisanCommand::RunRegression) {
            regression::request_regression();
        }
        let outcome = ServiceContainer::with_roaster_async(|roaster| {
            roaster.process_artisan_command(traced_command.command)
        }).await;
        // Emits STATUS/READ responses via output_channel
    }
}
```

### 3.2 Read Sensors

```rust
// src/application/tasks.rs:332
async fn read_sensors(...) {
    tick_state.sensor_err = ServiceContainer::roaster_async_sensor_read().await.err();
    // Internally:
    // SensorController::read_sensors()
    //   → SensorConversionHub::sample()
    //       → Max31856::read_temperature_async() ×2 (parallel, 210 ms)
}
```

- **Fault debounce:** 5 consecutive ticks per channel (`SENSOR_FAULT_DEBOUNCE = 5`) before NaN + `fault_condition`.
- `last_temp_read` updates **only if at least one channel is clean** (prevents false PID stale‑hold).

### 3.3 Control Update

```rust
// src/application/tasks.rs:378
async fn update_control_stage(...) -> (Option<ControlUpdateSnapshot>, Option<SystemStatus>) {
    ServiceContainer::with_roaster_async(|roaster| roaster.update_control(Instant::now()))
}
```

→ Delegates to **`RoasterControl::update_control()`** (sync, see §4).

### 3.4 Telemetry Emit

```rust
// src/application/tasks.rs:686
async fn emit_telemetry_stage(...) {
    // Continuous # @ 1 Hz (DEFAULT_OUTPUT_INTERVAL_MS = 1000)
    // Epoch aligned to START (not to prior OT1/OT2)
    // MutableArtisanFormatter::format(&status) → output_channel

    // #DUMP ring‑buffer drain (outside 1 Hz gate)
    // Up to 4 rows/tick, re‑push to front if channel full (no loss)
    const MAX_DUMP_ROWS_PER_TICK: usize = 4;
}
```

---

## 4. Control Logic: `RoasterControl::update_control()`

`src/control/roaster_control.rs:554` — **sync function** called from stage 3.

### 4.1 Safety Gates (evaluated in order; any triggers emergency)

```rust
// 1. Staleness guard
if last_temp_read > TEMP_VALIDITY_TIMEOUT_MS (5000 ms)
    → emergency_shutdown("Temperature sensor timeout")

// 2. SSR health check (mid‑roast)
actuator.periodic_health_check(current_time)

// 3. Comms idle
if (heater_energized || roast_active) &&
   now - last_command_received_at_ms > COMMS_IDLE_TIMEOUT_MS (30000)
    → emergency_shutdown("Comms idle timeout")

// 4. MAX_ROAST_TIME (30 min = 1800 s)
//    From profile_start_time (START) OR heat_session_start (manual)
//    EXCLUDES Preheating
if elapsed > MAX_ROAST_TIME_SECS
    → emergency_shutdown("Maximum roast time exceeded")

// 5. Cooldown latch (STOP → fan 100% until BT < 50 °C)
if cooling_active && BT < COOLING_RELEASE_BEAN_TEMP_C (50.0) && BT finite > 0
    → cooling_active = false

// 6. Charge detection (bean drop)
//    BT history: 10 samples / 3 s (CHARGE_SAMPLE_TICK_DIV = 1)
//    Drop > CHARGE_DROP_THRESHOLD_C (10.0) → #CHARGE event

// 7. RoR Guards (TIERED — A‑TC4‑D)
//    HARD (> MAX_BT_RATE_OF_RISE_HARD = 1.0 °C/s): ROR_EXCEEDED_CONSECUTIVE_LIMIT = 3 ticks
//    SOFT (MAX_BT_RATE_OF_RISE = 0.5 .. 1.0 °C/s): ROR_SOFT_DEBOUNCE_LIMIT = 12 ticks (~3.7 s)
//    BT‑only guard INDEPENDENT of PID channel (check_bt_rate vs check_rate_of_rise)

// 8. Probe stuck
//    PID mode: flat BT > PROBE_STUCK_TIMEOUT_SECS (120 s) → emergency
//    Manual: warning at 120 s (ERR probe_stuck_warning), emergency at 300 s (PROBE_STUCK_MANUAL_LATCH_SECS)
```

### 4.2 Heater Output Selection

```rust
let desired_output = if safety.is_emergency_active() {
    0.0                                    // Emergency → heater OFF
} else if status.artisan_control {
    dispatch.artisan_manual_heater()       // Manual OT1/OT2 (if SSR Available)
} else if status.pid_enabled {
    update_pid_control(current_time)       // PID + profile following
} else {
    0.0
};
```

### 4.3 PID Control (`update_pid_control`)

`src/control/roaster_control.rs:1828`

- **Throttle:** `pid_cycle_time_ms` (default 1000 ms, configurable via `PID;CT`, floor 10 ms)
- **Profile following:** interpolates `active_profile.target_at(elapsed_secs)`
- **Stale hold:** if sensor data > 500 ms old → hold last *applied* output (not PID intent)
- **Anti‑windup:** `PidFeedback { desired, applied, guard_busy }` → `dispatch.set_pid_feedback()`

### 4.4 SSR Write (Zero‑Cross Guarded)

```rust
// src/control/controllers/actuator.rs
actuator.apply_guarded_heater(desired, now, reject_on_busy, &mut status)
// - control_loop: reject_on_busy = false (never rejects, slew‑limits)
// - manual path: reject_on_busy = true (adopts setpoint for next window if busy)
```

### 4.5 Fan Selector (with Safety Interlock)

```rust
let fan_output = if safety.is_emergency_active() || cooling_active {
    100.0
} else if let (Some(profile), Some(start)) = (&fan_profile, profile_start_time) {
    profile.target_at(elapsed).unwrap_or(20.0)
} else {
    dispatch.artisan_manual_fan()
};

// HEATER↔FAN INTERLOCK: if heater > 0% and fan < 20% → force fan = 20%
if desired_output > 0.0 && fan_output < FAN_MIN_SAFETY_PCT (20.0) {
    fan_output = FAN_MIN_SAFETY_PCT;
}
```

---

## 5. Event / Command Management

### 5.1 Input Path (Artisan → Firmware)

```
Artisan PC
    │ USB CDC / UART0 (GPIO20/21)
    ▼
uart_reader_task / usb_reader_task     [Event‑driven: RX interrupt]
    │ parse_line() → ArtisanCommand enum
    ▼
artisan_channel.try_send(TracedCommand)  [MPSC bounded cap=16]
    │
    ▼ control_loop_task → drain_commands()
ServiceContainer::with_roaster_async(|roaster| {
    roaster.process_artisan_command(cmd)
})
```

### 5.2 Command Dispatch (`process_artisan_command`)

`src/control/roaster_control.rs:1101`

```rust
// WHITELIST when fault_condition active: READ, STATUS, STOP, EMERGENCY_STOP, START, PREHEAT, CHAN, UNITS, FILT, STREAM
// Others → ERR fault_condition_active

match command {
    ArtisanCommand::StartRoast       → handle_start_roast()
    ArtisanCommand::SetHeater(v)     → forward_artisan_manual_command(SetHeaterManual(v))
    ArtisanCommand::SetFan(v)        → forward_artisan_manual_command(SetFanManual(v))
    ArtisanCommand::Stop             → if fault: clear_emergency_explicit(); handle_stop()
    ArtisanCommand::EmergencyStop    → handle_emergency_stop()  // Latch, NO auto‑recover
    ArtisanCommand::Preheat(t)       → handle_preheat(t)
    ArtisanCommand::SetProfile       → handle_set_profile()     // Loads buffered profile
    ArtisanCommand::SetFanProfile    → handle_set_fan_profile()
    ArtisanCommand::SetPidGain(kp,ki,kd) → dispatch.set_pid_gains()
    ArtisanCommand::SetTargetTemp(t) → handle_set_target_temp() // Enables PID
    ArtisanCommand::SetStreaming(on) → enable/disable continuous `#` output
    // ... PID;CHAN, PID;SV, PID;LIMIT, PID;CT, UNITS, FILT, CHAN, REG, DUMP
}
```

### 5.3 Internal Command Flow (`process_command`)

`src/control/roaster_control.rs:243`

```rust
fn process_command(command: RoasterCommand, now: Instant) {
    if command == StopRoast {           // ONLY explicit recovery path
        stop_streaming();
        clear_emergency_explicit();     // Unlatch emergency, rearm SSR
        return;
    }

    // 1. Safety policy (emergency stop, overtemp, etc.)
    if safety.can_handle(command) {
        let outcome = safety.evaluate(command, &mut status);
        if outcome.emergency_active { apply_safety_outcome(); return Err }
        return Ok;
    }

    // 2. Manual policy (OT1/OT2, PID;OFF, etc.)
    if dispatch.can_handle_manual(command) {
        let outcome = dispatch.evaluate_manual_policy(command, &mut status);
        if outcome.success { apply_policy_outcome() } else { Err }
        return;
    }

    // 3. Dispatch (profile, streaming, config)
    match dispatch.process_command(command, now, &mut status) {
        CommandDispatchResult::StopStreaming → stop_streaming(),
        CommandDispatchResult::Handled(r) → r,
    }
}
```

### 5.4 Output Path (Firmware → Artisan)

```
control_loop_task / command handlers
    │
    ▼ output_channel.try_send(formatted_string)  [MPSC bounded cap=32]
    │
    ▼ dual_output_task (Timer 5 ms, drains ≤4 msg/tick)
multiplexer.get_active_channel() → USB or UART
    │
    ▼ usb_cdc_write_bytes() / uart_write_bytes() + CRLF
    ▼
Artisan PC
```

### 5.5 Formatters (`src/output/artisan.rs`)

| Command | Function | Format |
|---------|----------|--------|
| `READ` | `format_read_response_full()` | `BT,ET,heater%,fan%,target,MV,SP,PID` (8 fields PID on) / 5 fields PID off |
| `STATUS` | `format_status_response()` | Human‑readable multi‑line |
| `CHAN` | `format_chan_ack(rate)` | `# CHAN <rate>` |
| `UNITS`/`FILT`/`STREAM` | `format_handshake_ack()` | `# Changed ...` |
| Continuous `#` | `MutableArtisanFormatter::format()` | `# time,BT,ET,heater,fan,target,RoR` @ 1 Hz |
| `#DUMP` | `roast_logger::dump()` | Ring buffer 256 rows, drained 4/tick |

---

## 6. State Machine (RoasterState)

`src/config/constants.rs`

```
Idle ──PREHEAT──► Preheating ──START──► Heating ──converged (±2 °C)──► Stable
  ▲                    │                    │                    │
  │                    │                    ▼                    ▼
  │                    └────STOP──────────► Idle ◄──────────────┘ (hysteresis ±3 °C)
  │                         │
  └────EMERGENCY/FAULT──────► Error (latched)
                                │
                                ▼
                         StopRoast (PID;OFF) → clear_emergency_explicit() → Idle
```

- **Emergency latch** (`safety.is_emergency_active()` + `status.fault_condition`): set by safety traps, cleared **ONLY** by `StopRoast` → `clear_emergency_explicit()`.
- **Cooldown latch** (`cooling_active`): set by `STOP`, cleared by BT < 50 °C, new roast, or explicit recovery.

---

## 7. Key Data Structures

| Struct | File | Purpose |
|--------|------|---------|
| `SystemStatus` | `src/config/` | Live snapshot: temps, heater%, fan%, PID state, faults, SSR hw status |
| `RoasterControl` | `src/control/roaster_control.rs` | Facade owning 4 controllers + roast state + profiles + charge detection |
| `SensorController` | `src/control/controllers/sensor.rs` | Sampling, fault debounce, RoR guards (PV + BT‑only) |
| `ActuatorController` | `src/control/controllers/actuator.rs` | SSR (zero‑cross), fan (PWM), slew/guard, health check |
| `SafetyController` | `src/control/controllers/safety.rs` | Evaluates safety commands → `SafetyPolicyOutcome` |
| `CommandDispatcher` | `src/control/controllers/dispatch.rs` | PID state, manual setpoints, continuous output manager |
| `ServiceContainer` | `src/application/service_container.rs` | Singleton: `Mutex<RoasterControl>`, channels, watchdog, multiplexer |

---

## 8. Concurrency & Synchronization

| Resource | Protection | Access Pattern |
|----------|------------|----------------|
| `RoasterControl` | `Mutex<RefCell<Option<...>>>` in `ServiceContainer` | `with_roaster_async(|r| { ... }).await` (brief lock) |
| `artisan_channel` | `Channel<CriticalSectionRawMutex, 16>` | `try_send` (readers), `try_receive` (control_loop) |
| `output_channel` | `Channel<CriticalSectionRawMutex, 32>` | `try_send` (control/handlers), `try_receive` (dual_output) |
| `multiplexer` | `Mutex<RefCell<Option<CommMultiplexer>>>` | `critical_section::with` (dual_output) |
| Watchdog | `WatchdogFeeder` in `ServiceContainer` | `with_watchdog(|w| w.feed_async(bt)).await` |

**No shared mutable state without sync primitives.** All cross‑task communication via channels.

---

## 9. Critical Constants (`src/config/constants.rs`)

| Constant | Value | Use |
|----------|-------|-----|
| `CONTROL_LOOP_PERIOD_MS` | 100 | Nominal timer (real ~310‑330 ms) |
| `MAX31856_CONVERSION_TIME_MS` | 210 | One‑shot conversion wait |
| `TEMP_VALIDITY_TIMEOUT_MS` | 5000 | Staleness guard |
| `COMMS_IDLE_TIMEOUT_MS` | 30000 | Comms idle emergency |
| `MAX_ROAST_TIME_SECS` | 1800 | 30 min budget |
| `OVERTEMP_THRESHOLD` | 300.0 | °C overtemp latch |
| `FAN_MIN_SAFETY_PCT` | 20.0 | Interlock minimum with heater>0 |
| `SENSOR_FAULT_DEBOUNCE` | 5 | Fault latch threshold |
| `MAX_BT_RATE_OF_RISE` | 0.5 | °C/s soft band start |
| `MAX_BT_RATE_OF_RISE_HARD` | 1.0 | °C/s hard band |
| `ROR_EXCEEDED_CONSECUTIVE_LIMIT` | 3 | Hard band ticks |
| `ROR_SOFT_DEBOUNCE_LIMIT` | 12 | Soft band ticks |
| `PROBE_STUCK_TIMEOUT_SECS` | 120 | Warning (manual) / latch (PID) |
| `PROBE_STUCK_MANUAL_LATCH_SECS` | 300 | Latch stage (manual) |
| `PROBE_STUCK_VARIATION_C` | 1.0 | Minimum °C movement |
| `COOLING_RELEASE_BEAN_TEMP_C` | 50.0 | Cooldown latch release |
| `DEFAULT_OUTPUT_INTERVAL_MS` | 1000 | Telemetry continuous rate |
| `LOG_CAPACITY` | 256 | Ring buffer samples |
| `HEAT_SESSION_OFF_DEBOUNCE_SECS` | 60 | Heat session end debounce |

---

## 10. Testing the Control Loop

```bash
# Full tick with simulated sensors (L3 pipeline)
cargo test --target x86_64-unknown-linux-gnu --features test,simulated-sensors \
  --lib control_loop_tick_simulated_sensors_full_pipeline

# Replay real Artisan transcripts (wire contract)
cargo test --target x86_64-unknown-linux-gnu --features test \
  --test artisan_transcript_replay

# Full roast verification (18 deterministic tests: preheat, charge, profile, safety, STOP, 2× roasts)
cargo test --target x86_64-unknown-linux-gnu --features test \
  --test full_roast_verification

# Race check (forces interleaving on shared container)
cargo test --target x86_64-unknown-linux-gnu --features test \
  --lib --tests --test-threads=1 --no-fail-fast

# Regression numeric suite (sensor_conversion: 19‑bit two's‑complement math)
cargo test --target x86_64-unknown-linux-gnu --features test,regression \
  --test sensor_conversion --no-fail-fast
```

---

## 11. Quick Reference Files

| What you need | File |
|---------------|------|
| Main task & tick loop | `src/application/tasks.rs` |
| Per‑tick control logic | `src/control/roaster_control.rs::update_control()` |
| Controllers (sensor, actuator, safety, dispatch) | `src/control/controllers/*.rs` |
| Command parsing | `src/input/parser.rs` |
| Artisan formatting | `src/output/artisan.rs` |
| UART/USB readers | `src/hardware/uart/tasks.rs`, `src/hardware/usb_cdc/tasks.rs` |
| Watchdog | `src/safety/watchdog.rs` |
| Regression task | `src/safety/regression.rs` |
| Status LED | `src/hardware/status_led.rs` |
| Roast ring buffer | `src/logging/roast_logger.rs` |
| Traceability / Instrumentation | `src/logging/traceability.rs`, `src/application/stage_instrumentation.rs` |
| ServiceContainer (singleton) | `src/application/service_container.rs` |

---

*Last updated: 2026‑09‑11. Synced with `develop` branch.*