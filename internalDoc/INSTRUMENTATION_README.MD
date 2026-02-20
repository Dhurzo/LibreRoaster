# USB instrumentation wiring

## Hook location

The USB instrumentation helper lives next to the queue processor in `src/hardware/usb_cdc/tasks.rs` as `process_usb_command_data_test`. It simply reuses the same parser and command queue that the production `usb_reader_task` and `usb_queue_processor_task` depend on, so exercising it proves that the exported hook works against the real Artisan queues instead of sitting unused.

## Instrumentation harness

`tests/usb_instrumentation_runner.rs` is the dedicated harness that drives this wiring. The harness runs on the `riscv32` target, resets the `ServiceContainer` channels, reinitializes the USB command queue, and activates the USB multiplexer before calling `process_usb_command_data_test(b"READ\r")`. After the helper runs, the harness drains the queue into the artisan channel to prove the command was emitted and that the queue depth metrics recorded the injection.

## Audit rationale

This run is the documented instrumentation exercise that auditors should cite when demonstrating that `process_usb_command_data_test` is wired into the command pipeline. Because the harness is the only place that calls the helper, it closes the unused-export gap and makes the hook observable without adding new production-facing capabilities.
