# Roadmap: LibreRoaster v5.4

## Milestones

- ✅ **v5.2 Architecture Hardening & Validation** - Phases 95-103 (shipped 2026-03-20)
- 📋 **v5.3 Deep Bug Analysis & Defect Report** - Phases 104-109 (deferred)
- 🚧 **v5.4 Architecture Decomposition & Quality Fixes** - Phases 110-115 (in progress)

## Phases

### Phase 110: Quality Quick Wins (Clippy + Test Fix)
**Goal**: Fix all 24 pre-existing clippy warnings and the broken ssr_scheduler test.
**Depends on**: Nothing (first phase, fully independent)
**Requirements**: CLP-01, CLP-02, TST-01
**Success Criteria**:
  1. `cargo clippy --release --target riscv32imc-unknown-none-elf --features embedded -- -D warnings` exits 0.
  2. `cargo clippy --target x86_64-unknown-linux-gnu --features "std,test" -- -D warnings` exits 0.
  3. `cargo test --target x86_64-unknown-linux-gnu --features "std,test" --test ssr_scheduler` passes all 3 tests.
  4. Full test suite (244 tests) still passes.
**Plans**: 3 plans

Plans:
- [ ] 110-01: Fix clippy issues in app_builder.rs (redundant closures, new_without_default, unwrap_or_default) — 13 lints
- [ ] 110-02: Fix clippy issues in hardware/ files (ledc_bus.rs, ssr_ledc.rs, fan.rs, conversion.rs, ssr.rs, usb_cdc/driver.rs) — 11 lints
- [ ] 110-03: Fix ssr_scheduler test (guard_rejects_commands_while_busy — wrong time expectation)

### Phase 111: RoasterControl Decomposition — Controller Extraction
**Goal**: Extract focused controller types from RoasterControl while preserving all behavior.
**Depends on**: Phase 110
**Requirements**: SRP-01, SRP-02, SRP-03
**Success Criteria**:
  1. New controller types exist: `TemperatureController`, `HeaterController`, `FanController`, `SafetyController`.
  2. Each controller owns a bounded set of fields and methods from RoasterControl.
  3. The handler chain pattern is preserved for command dispatch.
  4. RoasterControl still works as a facade that delegates to controllers (backward-compatible).
  5. All existing tests pass without modification.
**Plans**: 3 plans

Plans:
- [ ] 111-01: Extract TemperatureController (read_sensors, update_temperatures, is_temperature_valid, last_sensor_sample)
- [ ] 111-02: Extract HeaterController (apply_guarded_heater, update_guard_busy_ms, capture_ssr_monitor_metrics, busy_window_ms, last_desired_heater_output)
- [ ] 111-03: Extract FanController (get_fan_speed) + SafetyController (emergency_shutdown, mark_overtemp_regression_active, apply_safety_outcome)

### Phase 112: RoasterControl Integration — Call Site Migration
**Goal**: Update all callers to use the new controller interfaces and remove the RoasterControl facade.
**Depends on**: Phase 111
**Requirements**: SRP-04, SRP-05
**Success Criteria**:
  1. All callers in tasks.rs, regression.rs, and tests use controller references directly.
  2. RoasterControl facade is removed or reduced to a thin composition root.
  3. Artisan protocol responses are byte-identical (verified by existing tests).
  4. Full test suite passes.
**Plans**: 2 plans

Plans:
- [ ] 112-01: Update tasks.rs control loop to use controller references
- [ ] 112-02: Update all test files and regression.rs to use controller references

### Phase 113: ServiceContainer — Constructor Injection
**Goal**: Replace ServiceContainer singleton with constructor-injected dependencies.
**Depends on**: Phase 112
**Requirements**: DIP-01, DIP-02
**Success Criteria**:
  1. `static_cell` singleton removed from ServiceContainer.
  2. Dependencies flow through constructors — no upward static access.
  3. ServiceContainer becomes a plain struct with owned fields.
  4. All tests pass.
**Plans**: 2 plans

Plans:
- [ ] 113-01: Refactor ServiceContainer from singleton to owned struct with constructor injection
- [ ] 113-02: Update app_builder.rs to wire dependencies through constructors

### Phase 114: ServiceContainer — Call Site Migration
**Goal**: Update all Embassy tasks and call sites to receive injected dependencies.
**Depends on**: Phase 113
**Requirements**: DIP-03, DIP-04
**Success Criteria**:
  1. Embassy task signatures accept dependencies as parameters.
  2. All 6+ ServiceContainer:: call sites use injected references.
  3. No remaining `ServiceContainer::get_*()` static access.
  4. All tests pass.
**Plans**: 2 plans

Plans:
- [ ] 114-01: Update Embassy tasks in tasks.rs to receive injected dependencies
- [ ] 114-02: Update regression.rs and remaining call sites to use injected references

### Phase 115: Full Verification & Clean Build
**Goal**: Verify the entire milestone passes all quality gates on both targets.
**Depends on**: Phase 114
**Requirements**: VER-01, VER-02, VER-03, VER-04
**Success Criteria**:
  1. ESP32 release build: zero errors, zero warnings.
  2. All 244+ host tests pass.
  3. Host clippy: clean.
  4. ESP32 clippy: clean.
**Plans**: 1 plan

Plans:
- [ ] 115-01: Run full verification suite (build + test + clippy on both targets)

## Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 110. Quality Quick Wins | 0/3 | Not started | - |
| 111. Controller Extraction | 0/3 | Not started | - |
| 112. Call Site Migration | 0/2 | Not started | - |
| 113. Constructor Injection | 0/2 | Not started | - |
| 114. DI Call Site Migration | 0/2 | Not started | - |
| 115. Full Verification | 0/1 | Not started | - |

---

*Roadmap created: 2026-04-22*
*For milestone: v5.4 Architecture Decomposition & Quality Fixes*
