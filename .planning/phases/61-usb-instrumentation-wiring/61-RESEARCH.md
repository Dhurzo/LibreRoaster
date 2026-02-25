# Phase 61 – Research Notes

## Summary
- `process_usb_command_data_test` currently only lives in `src/hardware/usb_cdc/tasks.rs` behind the `test` feature and is not referenced anywhere else in the tree.
- The goal for this phase is to wire that export into a concrete instrumentation consumer so the host-side instrumentation run actually executes the hook, satisfying the audit gap about the unused helper.
- The hardware tasks already expose the USB command queue, parser helper, and instrumentation entry point we need to reuse, so the research focus is on which harness or run should own the call and how to document it.

## Findings
1. The exported helper just calls `process_usb_command_data`, which pushes parsed commands onto the USB queue that the `usb_queue_processor_task` already drains, so invoking the helper from a harness should reuse the same queue and queue depth metrics without any production changes.
2. There is no existing instrumentation runner that references `process_usb_command_data_test`; for wiring we can either add the call into the host-side instrumentation/test harness used to verify USB command handling or introduce a dedicated host instrumentation runner that invokes the export.
3. The roadmap success criteria emphasize exercising the wiring during a documented instrumentation run/CI test so that the hook is no longer unused and the wiring is documented, so the chosen harness must be reproducible for auditors.
4. Documentation should live in `INSTRUMENTATION_README.MD` (per the context discussion) and describe the hook’s location, why it exists, and where to execute the instrumentation run without restating the success criteria or providing runnable steps beyond naming the run.

## Open Questions for Planning
- Which existing host instrumentation or CI script is the best place to invoke the hook so that the run can be reproduced? Does `process_usb_command_data_test` need a new wrapper or can the instrumentation run call it directly?
- Should the harness share the production `ServiceContainer` context or run in an isolated container to avoid affecting live USB command processing?
- Does the instrumentation run need gating (flag/handshake) before invoking the hook, or should it fire unconditionally during the documented integration test?
- What documentation fragment (INSTRUMENTATION_README.MD section) best captures the hook without duplicating success criteria?

## Next Steps
- Decide where the hook runs and ensure the instrumentation path is part of the documented run listed in the roadmap.
- Capture the wiring decision and documentation pointer in the plan so the checker can verify the implementation honors the context.
