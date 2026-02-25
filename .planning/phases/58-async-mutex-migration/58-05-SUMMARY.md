---
phase: 58-async-mutex-migration
plan: 05
subsystem: test/integration
tags: [gap-closure, async, embassy-sync, testing]

# Phase 58 Plan 05 Summary

**Goal:** Close ASYNC-06 by proving that multiple asynchronous sensor reads can safely share the `embassy_sync::Mutex`.

## Accomplishments

- Added `tests/concurrent_sensor_test.rs`, which initializes the `ServiceContainer`, populates the async mutex, and uses a `futures::ThreadPool` to spawn five concurrent `roaster_async_sensor_read()` futures.
- Introduced a host stub for `_embassy_time_now()` so the `embassy-time` driver links cleanly on x86_64 builds and added a `futures` dev-dependency with the `thread-pool` feature to drive the integration harness.
- Verified that every concurrent read completes without panicking or returning `ContainerError`, satisfying the ASYNC-06 requirement for an automated concurrency check.

## Tests

- `cargo test --target x86_64-unknown-linux-gnu --test concurrent_sensor_test` *(passes, library builds emit existing deprecation and dead-code warnings but no failures)*

## Issues Encountered

- None; the only visible warnings are pre-existing (deprecated `with_roaster()` usage in `service_container` and unused USB CDC driver helpers).
