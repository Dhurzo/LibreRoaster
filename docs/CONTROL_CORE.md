# Control core — structure and rules (CORE-2026-10-09)

`RoasterControl` (`src/control/roaster_control.rs`) is still the single object that turns protocol intent into hardware behaviour. Since the CORE refactor, three parts of it live in small pure modules, with unit tests, instead of being spread through the 3000-line file.

## Module map

```
src/control/roaster_control.rs   RoasterControl — tick loop, command handlers, the ONLY hardware writer
  ├─ mode.rs         ControlMode   — who drives the heater, derived from the stored flags (pure)
  ├─ batch.rs        BatchState    — one batch of beans: charge detection, CHARGE/DROP markers,
  │                                  batch weight, RoR-follow generator (pure)
  ├─ probe_stuck.rs  ProbeStuckDetector — dead-probe rules, returns a Verdict (pure)
  ├─ ror_follow.rs   RorFollower   — RoR profile → setpoint (pure)
  ├─ autotune.rs     StepTest      — TUNE step test (pure)
  └─ controllers/    sensor, actuator, safety (latch), dispatch (PID + manual handlers)
```

"Pure" means no hardware, no clock reads, no wire output. The caller passes `now` and the readings. The caller also executes what the module returns: a log line, a wire line, or a latch.

## Who drives the heater: `ControlMode`

| `pid_enabled` | `artisan_control` | extra | `ControlMode` |
|---|---|---|---|
| false | false | — | `Off` |
| false | true | TUNE queued/running | `ManualTune` |
| false | true | — | `Manual` |
| true | false | RoR follower present | `RorFollow` |
| true | false | — | `FirmwarePid` |
| true | true | — | `Transitional` (inside a handler only, never at the end of a tick) |

These are the only places that write the two flags:
- `RoasterControl::enter_operator_manual` and `leave_firmware_pid`;
- `start_roast_handoff`;
- `CommandDispatcher::enable_pid` and `stop_streaming`;
- the policy outcomes in `policies.rs` and `handlers/temperature.rs`.

The emergency latch is not a mode. It overrides every mode (`SafetyController`).

## Lifecycle of a batch: `BatchState`

Each event has exactly one method, and its doc comment carries the full contract table. In short:

| Event | detection | pending CHARGE | explicit | dropped | RoR-follow |
|---|---|---|---|---|---|
| STOP (`on_stop`) | reset | cleared | cleared | cleared | stopped |
| latch recovery (`on_recovery`) | reset | **kept** | **kept** | cleared | stopped |
| new roast (`on_new_roast`) | reset | **kept** | cleared | cleared | stopped |
| DROP (`on_drop`) | reset, but `charge_time` kept, in a roast only | cleared | cleared | **set** | stopped |
| PREHEAT (`on_preheat`) | — | cleared unless already preheating | — | — | — |
| `PID;SV` (`on_setpoint_override`) | — | — | — | — | stopped if ramping past the grace window; re-arm cancelled |

RoR-follow arms only through `BatchState::arm_follower(follower, can_arm)`, where `can_arm = RoasterControl::ror_can_arm()`. That requires a profile loaded, the firmware PID in control, the BT channel, and Heating/Stable.

## Dead-probe detector: `ProbeStuckDetector`

| Mode | Flat BT | Result |
|---|---|---|
| Firmware PID | 120 s | latch |
| Manual / Artisan software PID | 120 s / 300 s | warning / latch |
| Equilibrium (BT hot, ET flat) | — | clock re-anchors |
| … firmware PID at ≥ 50 % duty (N4) | 300 s / 600 s | warning / latch |

`update_control` only executes the verdict: the warning line first, then `emergency_shutdown("Probe stuck")`.

## The golden trace (`tests/core_golden.rs`)

The core runs on synthetic time, through `process_artisan_command_at(cmd, now)`. The production wrapper `process_artisan_command(cmd)` passes the real clock.

`tests/core_golden.rs` drives the core through:
- 20 scripted scenarios, covering every mode, every recovery path and the bugs of the 2026-10 audits;
- 24 seeded random operator sessions.

For each one it hashes a quantized per-tick trace. The trace includes the private state exposed by `core_snapshot()` (test feature only). The same harness checks 11 invariants on every tick.

**Rules:**
- A change that is meant to be a refactor must keep every hash.
- A change that is meant to change behaviour must say so in its commit. Its author regenerates the table with `CORE_GOLDEN_PRINT=1`, explains in the commit message which scenarios changed and why, and adds or updates a targeted test for the new behaviour.
- Never regenerate the table just to make a red test green.

**Bug hunt with fresh seeds** (invariants only, no hashes):
```bash
CORE_FUZZ_SEEDS=500 CORE_FUZZ_BASE=1000 cargo test --target x86_64-unknown-linux-gnu --features test --test core_golden -- --ignored
```
