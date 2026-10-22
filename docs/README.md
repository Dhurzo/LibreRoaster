# LibreRoaster Documentation

This is the canonical index for LibreRoaster documentation. Each topic has one owner document;
other documents link to it instead of restating its tables, commands, or invariants.

Start with the path that matches your goal:

| I want to... | Read |
|---|---|
| Understand what LibreRoaster is and whether it fits my build | [`../README.md`](../README.md), [`PHILOSOPHY.md`](PHILOSOPHY.md) |
| Connect the official Artisan app | [`ARTISAN_CONNECTION.md`](ARTISAN_CONNECTION.md) |
| Run a manual or profile roast from Artisan | [`ARTISAN_MODES.md`](ARTISAN_MODES.md) |
| Use CHARGE/DROP, RoR-follow, or autotune | [`ARTISAN_ADVANCED.md`](ARTISAN_ADVANCED.md) |
| Understand the runtime and control core | [`ARCHITECTURE.md`](ARCHITECTURE.md), [`CONTROL-LOOP.en.md`](CONTROL-LOOP.en.md), [`CONTROL_CORE.md`](CONTROL_CORE.md) |
| Look up the serial protocol | [`PROTOCOL.md`](PROTOCOL.md) |
| Interpret STATUS and diagnostics | [`INSTRUMENTATION.md`](INSTRUMENTATION.md) |
| Build or wire the hardware | [`HARDWARE.md`](HARDWARE.md), [`pinout.md`](pinout.md) |
| Choose USB or UART | [`CONNECTION_TYPES.md`](CONNECTION_TYPES.md) |
| Build, flash, test, or run quality gates | [`DEVELOPMENT.md`](DEVELOPMENT.md) |
| Prepare a board for first power-on | [`preflight-check.md`](preflight-check.md) |
| Test without hardware or in Wokwi | [`SIMULATION.md`](SIMULATION.md) |
| Inspect a Wokwi F2 preset | [`WOKWI_ROAST_PRESET.md`](WOKWI_ROAST_PRESET.md) |
| Change simulated temperature curves | [`simulated-curve-test.md`](simulated-curve-test.md) |
| Find where a feature is implemented | [`FEATURE_MAP.md`](FEATURE_MAP.md) |
| Check hazards and their automated evidence | [`SAFETY_CASE.md`](SAFETY_CASE.md) |
| Read historical audits and fix-round notes | [`archive/README.md`](archive/README.md) |

## Reading Paths

### New contributor

1. [`../CONTEXT.md`](../CONTEXT.md) for current state and constraints.
2. [`ARCHITECTURE.md`](ARCHITECTURE.md) for the system model.
3. [`CONTROL-LOOP.en.md`](CONTROL-LOOP.en.md) for tick-level behavior.
4. [`FEATURE_MAP.md`](FEATURE_MAP.md) for code navigation.
5. [`DEVELOPMENT.md`](DEVELOPMENT.md) for the verification workflow.

### Artisan operator

1. [`ARTISAN_CONNECTION.md`](ARTISAN_CONNECTION.md) for device settings and the handshake.
2. [`ARTISAN_MODES.md`](ARTISAN_MODES.md) for manual and profile workflows.
3. [`ARTISAN_ADVANCED.md`](ARTISAN_ADVANCED.md) for CHARGE/DROP, RoR-follow, and TUNE.
4. [`PROTOCOL.md`](PROTOCOL.md) only when you need exact command grammar.

### Hardware builder

1. [`HARDWARE.md`](HARDWARE.md) for the firmware pin map and electrical constraints.
2. [`pinout.md`](pinout.md) for physical wiring and board safety.
3. [`CONNECTION_TYPES.md`](CONNECTION_TYPES.md) for USB/UART and GPIO9 boot behavior.
4. [`preflight-check.md`](preflight-check.md) before first power-on.

## Canonical Ownership

| Topic | Owner | Other documents must link, not duplicate |
|---|---|---|
| Runtime architecture, boot, task graph | `ARCHITECTURE.md` | README, CONTEXT, CONTROL_LOOP |
| Tick stages, PID flow, event handling | `CONTROL-LOOP.en.md` | ARCHITECTURE |
| Normative control-core rules | `CONTROL_CORE.md` | ARCHITECTURE, DEVELOPMENT |
| Hazard → guard → test evidence | `SAFETY_CASE.md` | README, ARCHITECTURE, Artisan guides |
| Wire grammar, commands, responses, errors | `PROTOCOL.md` | Artisan guides, README, INSTRUMENTATION |
| STATUS field semantics and diagnostics | `INSTRUMENTATION.md` | PROTOCOL, Artisan guides |
| Artisan connection and device setup | `ARTISAN_CONNECTION.md` | README, PROTOCOL |
| Manual and profile roast workflows | `ARTISAN_MODES.md` | PROTOCOL |
| CHARGE/DROP, RoR-follow, TUNE workflows | `ARTISAN_ADVANCED.md` | PROTOCOL |
| Firmware pin map and electrical limits | `HARDWARE.md` | README, pinout, CONNECTION_TYPES |
| Physical wiring and board safety | `pinout.md` | HARDWARE |
| USB/UART choice and GPIO9 boot behavior | `CONNECTION_TYPES.md` | README, HARDWARE |
| Build, flash, test, quality gates | `DEVELOPMENT.md` | README, CONTEXT, preflight |
| First-power-on hardware checklist | `preflight-check.md` | DEVELOPMENT, HARDWARE |
| Wokwi simulation and scenarios | `SIMULATION.md` | WOKWI_ROAST_PRESET, simulated-curve-test |
| F2 preset and heat-sense switch runbook | `WOKWI_ROAST_PRESET.md` | SIMULATION |
| Simulated temperature curves | `simulated-curve-test.md` | SIMULATION |
| Code navigation and change recipes | `FEATURE_MAP.md` | CONTEXT, ARCHITECTURE |
| Historical snapshots | `archive/README.md` | Never canonical |

## Language and Maintenance

- Active documentation is written in English, in direct technical prose.
- Use links for cross-cutting details; do not copy command tables, pin maps, or STATUS schemas.
- Prefer stable symbols (`RoasterControl::update_control`) over line numbers.
- Keep volatile test counts in one verification snapshot in [`DEVELOPMENT.md`](DEVELOPMENT.md).
- When a document is renamed, update every relative link and any source-code comment that names it.
