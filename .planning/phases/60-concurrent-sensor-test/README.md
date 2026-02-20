# Phase 60 – Concurrent Sensor Read Integration Test

## Summary

- Provide a host-side integration test that drives multiple `roaster_async_sensor_read()` futures against the same `ServiceContainer` so the async mutex migration is verified under parallel load.
- Close requirement **ASYNC-06** by proving the new lock survives concurrent sensor reads without drops or races.

## Tasks

1. Extend `tests/concurrent_sensor_test.rs` to push several concurrent sensor-read futures through `ServiceContainer::roaster` and await their completion.
2. Assert the test harness detects no dropped locks, no race conditions, and that every future completes even when executed back-to-back.
3. Run the test on the host target and document the new coverage so the milestone audit can reference the harness.
