# Roadmap: LibreRoaster

## Milestones

- ✅ **v3.0 Critical Safety Fixes** — Phases 49-57 (shipped 2026-02-19)
- ✅ **v4.0 Async Sensor Race Condition Fix** — Phases 58, 60-61 (shipped 2026-02-20) — see `.planning/milestones/v4.0-ROADMAP.md`
- ✅ **v4.x Command Transport Resilience** — Phase 59 (shipped 2026-02-19)

## Phases

<details>
<summary>✅ v3.0 Critical Safety Fixes (Phases 49-57) — SHIPPED 2026-02-19</summary>

- [x] Phase 49: Safety Static Fixes (1/1 plan) — completed 2026-02-18
- [x] Phase 50: Test Fix (1/1 plan) — completed 2026-02-18
- [x] Phase 51: Documentation (1/1 plan) — completed 2026-02-18
- [x] Phase 52: Performance Fixes (2/2 plans) — completed 2026-02-18
- [x] Phase 53: Integrate Async Temp (3/3 plans) — completed 2026-02-18
- [x] Phase 54: Clean Up Tech Debt (5/5 plans) — completed 2026-02-18
- [x] Phase 55: Fix Fan Telemetry (1/1 plan) — completed 2026-02-18
- [x] Phase 56: Complete Phase 51 Verification (1/1 plan) — completed 2026-02-19
- [x] Phase 57: Update Protocol References (1/1 plan) — completed 2026-02-19

</details>

<details>
<summary>✅ v4.0 Async Sensor Race Condition Fix (Phases 58-61) — SHIPPED 2026-02-20</summary>

See `.planning/milestones/v4.0-ROADMAP.md` for the full phase archive.

</details>

### ✅ v4.x Command Transport Resilience (Phase 59) — COMPLETE

**Goal:** Force the USB CDC and UART0 command producers to share the queue without dropping, reordering, or stalling Artisan commands when both sockets fire simultaneously, and give operators a reproducible host test plus instrumentation so transport regressions are easy to catch.

**Requirements:** COMMAND-01, COMMAND-02, DOC-01

#### Success Criteria

1. **Concurrent bursts survive** — USB + UART command bursts go through `queue_processor_task` without drop under the host integration test.
2. **Instrumentation observes backlog** — Queue depth/backlog counters surround the async loop and expose data for the test.
3. **Operators know how to rerun the test** — README documents how to run the new host test and interpret queue metrics.

### 🧪 v4.0 Concurrent Sensor Read Integration Test (Phase 60) — COMPLETE 2026-02-20

**Goal:** Add a host-side integration test that concurrently drives multiple `roaster_async_sensor_read` futures so the async mutex migration proves safe under parallel load.

**Requirements:** ASYNC-06

**Gap Closure:** Closes the audit requirement for a concurrent sensor-read integration test (ASYNC-06).

#### Success Criteria

1. **Parallel harness** — `tests/concurrent_sensor_test.rs` spawns several sensor-read tasks against the same `ServiceContainer` instance and joins them with `ThreadPool` handles.
2. **Race detection** — Harness asserts every task completes without `ContainerError`, instrumentation reports `max_async_lock_depth <= 1`, and metrics reset before/after the batch.
3. **CI adoption** — The host test runs under `async-lock-depth-metrics` and README guidance ties it to ASYNC-06 so the milestone audit can cite the run.

### 🎛️ v4.0 USB Instrumentation Wiring (Phase 61) — COMPLETE 2026-02-20

**Goal:** Wire the `process_usb_command_data_test` export into an actual consumer so the instrumentation hook is executed and validated during the milestone.

**Requirements:** (none; integration wiring)

**Gap Closure:** Closes the integration gap caused by the unused test hook export; the riscv32 instrumentation harness and README now document where the hook lives and why it is exercised.

#### Success Criteria

1. **Hook consumer** — Add a caller that invokes `process_usb_command_data_test` during a test or instrumentation run.
2. **Exercise integration** — Ensure the invocation path is covered by CI or local acceptance harness.
3. **Document wiring** — Update `hardware/usb_cdc/tasks.rs` or the roadmap to note where the hook lives and why it exists.

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 49. Safety Static Fixes | v3.0 | 1/1 | Complete | 2026-02-18 |
| 50. Test Fix | v3.0 | 1/1 | Complete | 2026-02-18 |
| 51. Documentation | v3.0 | 1/1 | Complete | 2026-02-18 |
| 52. Performance | v3.0 | 2/2 | Complete | 2026-02-18 |
| 53. Integrate Async | v3.0 | 3/3 | Complete | 2026-02-18 |
| 54. Tech Debt | v3.0 | 5/5 | Complete | 2026-02-18 |
| 55. Fan Telemetry | v3.0 | 1/1 | Complete | 2026-02-18 |
| 56. Phase 51 Verify | v3.0 | 1/1 | Complete | 2026-02-19 |
| 57. Protocol Refs | v3.0 | 1/1 | Complete | 2026-02-19 |
| 58. Async Mutex Migration | v4.0 | 4/4 | Complete | 2026-02-19 |
| 59. Command Transport Resilience | v4.x | 1/1 | Complete | 2026-02-19 |
| 60. Concurrent Sensor Read Test | v4.0 | 1/1 | Complete | 2026-02-20 |
| 61. USB Instrumentation Wiring | v4.0 | 1/1 | Complete | 2026-02-20 |

---

_Roadmap archived: See .planning/milestones/v3.0-ROADMAP.md and .planning/milestones/v4.0-ROADMAP.md for full details_
