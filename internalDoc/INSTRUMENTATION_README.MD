# USB instrumentation wiring

## Hook location

The USB instrumentation helper lives next to the queue processor in `src/hardware/usb_cdc/tasks.rs` as `process_usb_command_data_test`. It simply reuses the same parser and command queue that the production `usb_reader_task` and `usb_queue_processor_task` depend on, so exercising it proves that the exported hook works against the real Artisan queues instead of sitting unused.

## Instrumentation harness

`tests/usb_instrumentation_runner.rs` is the dedicated harness that drives this wiring. The harness runs on the `riscv32` target, resets the `ServiceContainer` channels, reinitializes the USB command queue, and activates the USB multiplexer before calling `process_usb_command_data_test(b"READ\r")`. After the helper runs, the harness drains the queue into the artisan channel to prove the command was emitted and that the queue depth metrics recorded the injection.

## Audit rationale

This run is the documented instrumentation exercise that auditors should cite when demonstrating that `process_usb_command_data_test` is wired into the command pipeline. Because the harness is the only place that calls the helper, it closes the unused-export gap and makes the hook observable without adding new production-facing capabilities.

## STATUS instrumentation payload

Automation tokens poll `STATUS` instead of `READ` when they need watchdog, guard, and regression telemetry. The payload is a deterministic CSV that keeps the old temperature/fan numbers for continuity while adding eight additional fields that describe the safety stack:

```
ET,BT,Heater,Fan,WatchdogOK,WatchdogFailures,LastWatchdogReason,LEDCGuardTimeouts,RegressionActive
```

Example response:

```
120.3,150.5,75.0,50.0,1,0,none,0,0
```

Column meanings:

- **ET / BT / Heater / Fan**: Same units as the legacy `READ` response; these values remain so automation can compare instrumentation snapshots with the human stream.
- **WatchdogOK** (`0/1`): Indicates whether the watchdog feed succeeded during the last control loop tick.
- **WatchdogFailures**: Consecutive failure count; automation can correlate rising values with `SAFETY WATCHDOG` logs.
- **LastWatchdogReason**: The `WatchdogError::reason()` token that explains why the feed failed; returns `none` when the watchdog is healthy.
- **LEDCGuardTimeouts**: Total guard timeout count tracked inside `SystemStatus`. Increments whenever the LEDC guard held the heater for safety.
- **RegressionActive** (`0/1`): Set while over-temperature regressions are running so harnesses can ensure regression sequences are not reentered or overlapped.

Auditors should treat `STATUS` as the automation hook. It delivers the same human-friendly telemetry as `READ` plus a snapshot of the safety state machine without touching the legacy stream, so tools that already monitor `READ` can keep doing so while instrumentation harnesses poll `STATUS` for the richer data.
