# TEST-01: SSR Guard Verification

**Objective:** Confirm every Artisan SSR command reaches the LEDC channel within ±2 ticks, the monitor retries once when hardware drift occurs, and the ≥1 s cycle guard reports a busy window when extra commands arrive too quickly.

## Preconditions

1. Flash the current firmware onto the ESP32‑C3 board and open the Artisan+ serial console (UART or USB CDC).
2. Ensure logging is enabled at `info` level so hardware monitor messages appear in the console or log buffer.

## Step-by-step Validation

1. **Validate ±2 tick accuracy**
   - Send `OT1 50` to drive the heater to 50 %.
   - Immediately run `READ` (`READ` command) and parse the CSV (ET,BT,Power,Fan). At the end of the line you now have `ssr_last_duty_delta_ticks` and `ssr_retry_count` in telemetry—confirm `|ssr_last_duty_delta_ticks| ≤ 2` and `ssr_retry_count` is `0`.
   - Repeat the sequence a few times in different duty ranges (e.g., `OT1 20`, `OT1 75`) to ensure the delta stays within ±2 ticks.

2. **Trigger monitor logging (optional)**
   - While staying within safe duty ranges, manually induce a drift (if hardware allows) or watch the `info!` log for lines such as:
     `SSR monitor delta X ticks, retries Y`
   - Confirm the log appears only when `ssr_last_duty_delta_ticks` exceeded ±2 and that `ssr_retry_count` equals `1` for that cycle.

3. **Verify cycle guard**
   - Issue `OT1 100` to enter a heating cycle; immediately issue another `OT1 100` before 1 s has elapsed.
   - Run `READ` and confirm `ssr_cycle_guard_busy_until_ms` reports a non-zero busy window (e.g., 800 ms) and the second command did not move `ssr_output` beyond the original value.
   - Wait until the busy window expires (`ssr_cycle_guard_busy_until_ms` reads `0`) and send `OT1 100` again—this command should be accepted, and `ssr_output` should reflect the new command.

## PASS Criteria

- `ssr_last_duty_delta_ticks` never exceeds ±2 inside the normal 0‑100 % range.
- When the hardware drifts beyond tolerance, `ssr_retry_count` increments to `1`, and the `SSR monitor delta…` log line appears.
- The cycle guard sets `ssr_cycle_guard_busy_until_ms` to a value > 0 whenever two commands arrive within 1 s, and commands are rejected or ignored until that window closes.

## Notes

- Use the Artisan READ response or log output for quick observations—`ssr_last_duty_delta_ticks`, `ssr_retry_count`, and `ssr_cycle_guard_busy_until_ms` are the new telemetry knobs that keep watchdogs calm.
- If tests are performed in automation, script the READ/OT1 sequence and parse the CSV instead of manually scanning logs.
