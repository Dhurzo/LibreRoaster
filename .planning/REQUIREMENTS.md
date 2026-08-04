# Requirements: LibreRoaster

**Defined:** 2026-04-22
**Core Value:** Artisan can read temperatures and control heater/fan during a roast session via serial connection.

> ✅ **Milestone v5.4 completed 2026-04-22** — all 16 requirements shipped (see `ROADMAP.md` phases 110-115).

## v5.4 Requirements

Requirements for the `v5.4 Architecture Decomposition & Quality Fixes` milestone.

### Quality Quick Wins

- [x] **CLP-01**: All 24 pre-existing clippy warnings on the ESP32 target are resolved (zero warnings with `-D warnings`).
- [x] **CLP-02**: Host target clippy remains clean (zero warnings with `-D warnings`).
- [x] **TST-01**: The `guard_rejects_commands_while_busy` test in `tests/ssr_scheduler.rs` is fixed and passes.

### SRP Decomposition

- [x] **SRP-01**: RoasterControl is decomposed into focused controllers with clear single responsibilities.
- [x] **SRP-02**: Each controller owns a bounded set of fields and methods — no cross-controller field access.
- [x] **SRP-03**: The existing handler chain pattern (`src/control/handlers/`) is preserved and extended.
- [x] **SRP-04**: All existing callers of RoasterControl methods are updated to use the new controller interfaces.
- [x] **SRP-05**: Artisan protocol responses remain byte-identical after decomposition.

### Dependency Injection

- [x] **DIP-01**: ServiceContainer singleton (`static_cell`) is replaced with constructor-injected dependencies.
- [x] **DIP-02**: Dependencies flow downward through constructors — no upward static access.
- [x] **DIP-03**: Embassy task signatures accept injected dependencies as parameters.
- [x] **DIP-04**: All 6+ ServiceContainer:: call sites are updated to use injected references.

### Verification

- [x] **VER-01**: `cargo build --release --target riscv32imc-unknown-none-elf --features embedded` produces zero errors and zero warnings.
- [x] **VER-02**: All host tests pass (`cargo test --target x86_64-unknown-linux-gnu --features test --lib --tests --no-fail-fast`).
- [x] **VER-03**: Host clippy is clean (`cargo clippy --target x86_64-unknown-linux-gnu --features "std,test" -- -D warnings`).
- [x] **VER-04**: ESP32 clippy is clean (`cargo clippy --release --target riscv32imc-unknown-none-elf --features embedded -- -D warnings`).

## Future Requirements

Deferred beyond this milestone.

### Further Architecture

- **ARCH-01**: Evaluate whether RoasterControl decomposition enables standalone mode (no Artisan dependency).
- **ARCH-02**: Consider event-driven architecture for inter-controller communication.

## Out of Scope

Explicitly excluded from `v5.4`.

| Feature | Reason |
|---------|--------|
| New Artisan protocol commands | This milestone is for architectural cleanup, not feature addition |
| Performance optimization beyond clippy fixes | Not the goal — decomposition may incidentally improve cache locality |
| Test infrastructure overhaul | Only fix the one broken test, don't refactor the test framework |
| Changing public API of embedded binary | The binary target has no public API — only Artisan protocol matters |

## Traceability

Which phases cover which requirements.

| Requirement | Phase | Status |
|-------------|-------|--------|
| CLP-01 | Phase 110 | Done |
| CLP-02 | Phase 110 | Done |
| TST-01 | Phase 110 | Done |
| SRP-01 | Phase 111 | Done |
| SRP-02 | Phase 111 | Done |
| SRP-03 | Phase 111 | Done |
| SRP-04 | Phase 112 | Done |
| SRP-05 | Phase 112 | Done |
| DIP-01 | Phase 113 | Done |
| DIP-02 | Phase 113 | Done |
| DIP-03 | Phase 114 | Done |
| DIP-04 | Phase 114 | Done |
| VER-01 | Phase 115 | Done |
| VER-02 | Phase 115 | Done |
| VER-03 | Phase 115 | Done |
| VER-04 | Phase 115 | Done |

**Coverage:**
- v5.4 requirements: 16 total
- Mapped to phases: 6 phases (110-115)
- Mapped: 16/16

---
*Requirements defined: 2026-04-22*
*Milestone completed: 2026-04-22*
*Last updated: 2026-08-04 (statuses flipped to Done after v5.4 shipped)*
