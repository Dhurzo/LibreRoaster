# Manual and Profile Modes with Artisan

**Last updated:** 2026-09-21

Practical guide to the two roast modes LibreRoaster exposes to the
Artisan application over the serial port (USB CDC or UART0, 115200 8N1).
Protocol reference: `docs/PROTOCOL.md`. Connection setup:
`docs/ARTISAN_CONNECTION.md`, `docs/ARTISAN_CONFIG.md`.

> Key idea: Artisan owns the session (`READ`, sliders, `START`/`STOP`);
> the firmware owns device-side control, telemetry, and safety
> interlocks. Artisan's `.alog` curve file is **display only**: it is
> never sent to the roaster. What the firmware follows is what arrives
> over serial (`PROFILE`/`FANPROFILE`/`PID;SV`/`OT1`/`OT2`).

---

## 1. Manual mode (Artisan sliders)

The operator sets heater and fan duty directly. The firmware computes
nothing: it applies what was requested on every tick.

### Commands

```text
OT1 75     → heater 75 % (0-100, accepts ` `, `;`, `,`, `=` as separator)
OT2 50     → fan 50 % (same; aliases: IO3, DCFAN)
UP/DOWN    → heater ±5 %
OT1,up / OT1,down → same step as UP/DOWN
```

* Without PID, `READ` returns 5 fields `AMB,ET,BT,0.0,0.0`.
* Internal state stays `Idle` (`pid_enabled = false`,
  `artisan_control = true`). Implementation:
  `src/control/roaster_control.rs:805-818` (manual branch of
  `update_control`).
* Out-of-range `OT2` is clamped to `0..100` and reported with
  `ERR OT2_CLAMPED fan=<n> heater_unchanged` without touching the heater
  (`docs/PROTOCOL.md` §6).

### Example manual session

```text
CHAN;1200
UNITS;C      → #OK
READ         → 0.0,120.3,150.5,0.0,0.0
OT1 60       → heater 60 %
OT2 50       → fan 50 %
READ         → 0.0,121.0,151.2,0.0,0.0   (still 5 fields: no PID)
STOP         → heater 0 %, fan 100 % (cooldown)
```

### Rules to know

* **Fan floor:** with heater > 0 % the fan never drops below `20 %`
  (`FAN_MIN_SAFETY_PCT`), even if you send `OT2 0`.
* **Safety backstops also apply in manual mode:** comms-idle (15 s
  without commands while heating), max roast time (30 min), two-stage
  stuck-probe detector (`ERR probe_stuck_warning` after 120 s of flat
  BT, latch at 300 s).
* **Artisan sends this on its own:** what stock Artisan sends without
  help is `CHAN`/`UNITS`/`FILT` on connect, `READ` polling, and the
  `OT1`/`OT2` sliders. See fixtures
  `tests/fixtures/artisan_transcripts/software_pid_session_ascii.txt`.

---

## 2. Profile mode (firmware-side curve)

The firmware follows a time→temperature recipe on its own (plus an
optional time→fan curve) by driving the PID. Useful for repeatable
roasts without moving sliders by hand.

### Commands

```text
PROFILE;0,50;120,150;300,200;480,225
FANPROFILE;0,30;60,60;300,80
START            → start the profile roast (enables PID + following)
PID;OFF          → stop and disable PID
STOP             → stop: heater 0 %, fan 100 %
```

* Up to `MAX_PROFILE_SETPOINTS = 16` points per profile
  (`src/config/constants.rs:306`).
* Temperatures are in **display units** (`UNITS;C/F`): converted to °C
  internally and validated against `50–300 °C`. A point outside the
  range rejects the profile with
  `ERR handler_failed invalid_state:profile_temp_out_of_range`.
* Linear interpolation between points (`RoastProfile::target_at`,
  `src/config/constants.rs:465-488`); before the first point returns
  the first one, after the last point **holds** the last one.
* With PID enabled, `READ` returns 8 fields
  `AMB,ET,BT,0.0,0.0,HEATER,FAN,SV`, where `SV` is the current profile
  point. Each PID cycle recomputes `target_temp` from the profile
  (`src/control/roaster_control.rs:1850-1862`) and the fan from
  `fan_profile.target_at(elapsed)` (`:911-913`).

### Example profile session

```text
CHAN;1200
UNITS;C            → #OK
PROFILE;0,50;120,150;300,200;480,225
FANPROFILE;0,30;60,60;300,80
START              → PID starts with SV = profile at t=0
READ               → 0.0,120.3,150.5,0.0,0.0,75.0,45.0,150.0
...                → SV rises on its own along the curve
STOP               → done: heater 0 %, fan 100 %
```

### Rules to know

* `START` pins `profile_start_time = now` (`handle_start_roast`,
  `src/control/roaster_control.rs:1218-1310`). A repeated `START` while
  `Heating`/`Stable` is ignored: the clock is not restarted.
* `STOP` clears `profile_start_time` but **keeps** both loaded
  profiles: the next roast reuses them without resending
  (`src/control/roaster_control.rs:472-476`).
* Without `FANPROFILE`/`OT2` the fan would fall to 0: the 20 % floor
  while heating prevents that anyway.
* Artisan does **not** send `PROFILE` on its own. You have to trigger
  it (see §3).

---

## 3. From Artisan or only from a console?

**From Artisan, no extra program needed.** The firmware accepts the same
commands over the same port Artisan already uses; no cable or port
change is required.

With one nuance:

| Artisan sends on its own | You have to trigger |
|---|---|
| `CHAN`/`UNITS`/`FILT` (on connect), `READ` (polling), `OT1`/`OT2` sliders, `PID;SV`, `START`/`STOP` | `PROFILE;...`, `FANPROFILE;...` |

Ways to send the profile:

1. **Programmable Artisan button/event** (recommended for routine use):
   create a button sending the literal
   `PROFILE;0,50;120,150;300,200` and another one sending
   `FANPROFILE;0,30;60,60;300,80`; then press `START`.
2. **Console on the same port** (`picocom`, PuTTY, Python script) with
   Artisan **disconnected** to preload; the profile stays in RAM (lost
   only on reboot), then connect Artisan and send `START`.

> Do not open the same port from two programs at once, and do not try
> Artisan on USB + console on UART at the same time: the multiplexer
> serves only **one active transport** (the first one sending a valid
> command; the other is ignored until 60 s of inactivity,
> `src/input/multiplexer.rs:85-128`).

---

## 4. Quick cheat sheet

```text
# Manual
OT1 60 / OT2 50 / UP / DOWN / STOP

# Profile
PROFILE;0,50;120,150;300,200
FANPROFILE;0,30;60,60;300,80
START / STOP / PID;OFF

# Observe
READ     → 5 fields (manual) or 8 fields (profile/PID: ...,HEATER,FAN,SV)
STATUS   → 20 fields, last one = safety latch (0/1)
```
