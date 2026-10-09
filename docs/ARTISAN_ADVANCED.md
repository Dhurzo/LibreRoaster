# Advanced Artisan setup — CHARGE/DROP markers, RoR-follow, RoR curves, autotune

All features below use stock Artisan: TC4 device, *Serial Command* button actions and the
`+ArduinoTC4_34` extra device. No wiring change. Menu names can differ slightly between
Artisan versions.

## 1. CHARGE / DROP markers (recommended for every roast)
In Artisan's Events configuration, set the action of the default event buttons:

| Button | Action | Command |
|---|---|---|
| CHARGE | Serial Command | `CHARGE;{WEIGHTin}` |
| DROP | Serial Command | `DROP` |

`{WEIGHTin}` is replaced by the batch weight in grams (0 if not entered). Effects:
- the 30-min roast budget of a `PID;ON` session anchors to the real charge;
- in manual mode the 90-min heat-session budget restarts at each charge (multi-batch days);
- DROP re-arms charge detection for the next batch and stops RoR-follow.
Both commands are pure markers: they never change heater or fan, and they are accepted while
the safety latch is armed. A marker pressed before the roast starts (during PREHEAT, or around
Artisan's own *pidOnCHARGE* `PID;ON`, in either order) is kept and applied when the roast starts;
a marker with no roast within 5 s is dropped, and a new PREHEAT discards a marker sent before it.
Press CHARGE when the beans go in: the RoR ramp needs to see BT drop at least 20 °C below its
value at the CHARGE (a door or tryer dip never does, so a false automatic charge cannot start a
ramp in an empty drum). A late press re-anchors the budget and keeps a ramp that is already running.

## 2. RoR-follow (firmware PID follows a rate-of-rise profile)
Preconditions (all three):
- Artisan's PID dialog: **source = 2** (BT with the default `CHAN;1200`, where ET = channel 1
  and BT = channel 2). On the ET channel RoR-follow never arms (the profile still loads, with no
  ERR), and `TUNE` is refused (`ERR handler_failed …:tune_needs_bt_channel`).
- Artisan's **ramp/soak and background-follow must be OFF**: both send `PID;SV` repeatedly, and
  every `PID;SV` is treated as an operator override that ends RoR-follow.
- The CHARGE marker of §1 (automatic detection also works, but the marker is exact; after a
  DROP only the marker arms RoR-follow — the BT fall of the drop looks like a charge).

Steps:
1. Add a custom event button with action *Serial Command*, e.g.
   `RORPROFILE;0,15;300,10;600,6` (seconds since CHARGE, °C/min — °F/min if Artisan is in °F).
   Limits: up to 16 points, 1–30 °C/min (1.8–54 °F/min in °F), strictly increasing times. The loaded profile stays
   loaded across roasts, STOP and latches until `RORPROFILE;OFF` or a reboot.
2. Start the roast with *PID ON* as usual.
3. Press CHARGE. RoR-follow arms; after the turning point (BT has dropped ≥ 20 °C below its
   value when CHARGE was pressed and turned up again — no drop, no ramp, also after the 180 s
   timeout) the PID setpoint ramps at the profile RoR, never more than 3 °C away from BT.
4. Moving the `OT1` slider suspends RoR-follow (you are in manual); *PID ON* resumes it.
   Moving the SV slider (`PID;SV`), DROP, PID OFF or `RORPROFILE;OFF` end it.
RoR-follow never acts in manual mode. A `PID;SV` before the turning point or in the first ≈ 3 s
of the ramp (Artisan sends one right after *PID ON*) does not end it; later it does. If the CHARGE
marker is pressed during an `OT1` takeover, the next *PID ON* re-arms RoR-follow; nothing else
re-arms it (not an automatic `#CHARGE`, not a DROP, not a `PID;SV` override). While it ramps, the
probe-stuck detector stays armed (a frozen BT latches like in plain PID mode).

## 3. RoR target and measured RoR as Artisan curves
Add the extra device `+ArduinoTC4_34` (Devices → Extra devices). Artisan then sends `CHAN;1234`
and the firmware fills READ channels 3/4:
- T3 = RoR target (°C/min or °F/min; 0 when RoR-follow is not active);
- T4 = measured RoR of the PID process value (BT by default).
Without the extra device nothing changes on the wire.

## 4. TUNE — step-test autotune
1. Run it **with a typical batch loaded**, in manual mode, after the turning point (BT roughly
   120–170 °C, well before first crack), heater steady at a duty between 10 and 80 %, fan fixed.
   The baseline drift (BT still rising) is measured and subtracted. An empty-drum TUNE also works
   but gives a different (faster) plant: use it only as a starting point. The test aborts by
   itself at BT ≥ 230 °C.
2. Press a custom button `TUNE;20` (step of +20 %; allowed 5–40, base + step ≤ 100).
3. Do not touch any slider for ~3–6 min (any command aborts the test; so does a latch).
4. On success the firmware prints `#TUNE kp=… ki=… kd=… k=… theta=… locked=1`, applies the gains
   and locks them, so Artisan's *PID ON* (`PID;T`) does not overwrite them.
5. `TUNE;STATUS` reprints the result, `TUNE;UNLOCK` accepts Artisan's gains again, `TUNE;ABORT`
   stops a running test. Results are RAM-only: copy them into Artisan's PID dialog to keep them
   across reboots.
A test that ends without gains prints `ERR tune_<reason>` (`bad_base_duty`, `bad_step`,
`probe_cold`, `no_response`, `implausible`, `too_hot`, `aborted`, `apply_failed`). These lines and
`#TUNE …` are spontaneous (they ignore `STREAM`) and can cost Artisan one READ sample. A refused `TUNE;<n>` prints
`ERR handler_failed <token>:tune_needs_manual_mode` (or `:tune_needs_bt_channel` on `PID;CHAN;1`,
`:tune_cooling_active`, `:fault_condition_active` while latched). Changing the PID channel
(`PID;CHAN;1` ↔ `PID;CHAN;2`) unlocks tuned gains; re-sending the same channel does not.
