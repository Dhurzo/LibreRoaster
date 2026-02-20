# Phase 61 – USB Instrumentation Wiring

## Summary

- Hook the exported `process_usb_command_data_test` from `hardware/usb_cdc/tasks.rs` into a real consumer so the instrumentation path gets exercised during the milestone.
- Ensure the integration gap noted in the audit is closed by wiring and documenting the hook.

## Tasks

1. Identify a suitable test harness or instrumentation run (host-level or CI) that can call `process_usb_command_data_test` and add the invocation.
2. Verify the exported hook executes and reports its results, so the audit can confirm the export is not left unused.
3. Update the roadmap or relevant documentation to describe the hook’s purpose, expected inputs, and where it is consumed.
