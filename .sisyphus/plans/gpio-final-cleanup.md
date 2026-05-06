# Final Plan: Complete GPIO Pinout Testing + Cleanup

## Current State (What's Done)
- `src/config/pinout.rs` — PinSpec table, PIN_TABLE, WIRING_MANIFEST, helper functions ✅
- `tests/pinout_validation.rs` — 14 tests ✅
- `tests/pin_digital_twin.rs` — 10 tests ✅
- `scripts/preflight-check.sh` — pre-flash gate ✅
- `src/hardware/init.rs` — runtime assertions added ✅
- All 252 tests pass (229 lib + 14 pinout + 10 digital twin) ✅

## Remaining Gaps

### Gap 1: GPIO8 (Status LED) is orphaned
**Problem:** `PIN_TABLE` has GPIO8 (Status LED) but `init.rs` doesn't initialize it. Not in `InitPeripherals`, not in `HardwareHandles`.
**Fix:** Since GPIO8 is a strapping pin (JTAG enable) and push-pull only, add initialization in `init.rs` OR remove from `PIN_TABLE` if not implemented.
**Decision:** Add init code for GPIO8 — it's documented in `pinout.md` and safety-critical (JTAG strapping).

### Gap 2: `init.rs` typed peripherals vs constants
**Problem:** `peripherals.gpio9` is typed as `GPIO9` — can't use `FAN_PWM_PIN` directly. Runtime `assert_eq!` added, but compile-time link is missing.
**Fix:** The esp-hal type system IS the compile-time check — `GPIO9` type can only be `gpio9`. Add a compile-time doc comment linking to constants, and ensure the runtime check is there (already done).

### Gap 3: Pre-flash check end-to-end verification
**Problem:** `preflight-check.sh` written but never executed.
**Fix:** Run it, verify output.

### Gap 4: Pre-existing embedded build error
**Problem:** `ArtisanCommand::Stop` variant missing — `cargo check` fails.
**Fix:** Document as pre-existing, NOT caused by my changes. Optionally fix it since we're here.

---

## Execution Plan

| Step | Action | File(s) | Priority |
|------|--------|---------|----------|
| **1** | Add GPIO8 (Status LED) init in `init.rs` | `src/hardware/init.rs` | HIGH |
| **2** | Add GPIO8 to `InitPeripherals` + `HardwareHandles` | `src/hardware/init.rs` | HIGH |
| **3** | Remove tautological `input_pins_reject_set_output` test (tests mock, not hardware) | `tests/pin_digital_twin.rs` | MEDIUM |
| **4** | Run `preflight-check.sh` end-to-end | shell | MEDIUM |
| **5** | Fix pre-existing `ArtisanCommand::Stop` error (optional, but enables embedded check) | `src/config/constants.rs` | LOW |
| **6** | Final verification: all tests pass, preflight passes | shell | HIGH |

---

## Step Details

### Step 1-2: GPIO8 Status LED Init
Add to `init.rs`:
```rust
// In InitPeripherals: pub gpio8: GPIO8<'static>,
// In init_hardware:
let status_led = Output::new(peripherals.gpio8, Level::High, OutputConfig::default());
// Add to HardwareHandles: pub status_led: Output<'static>,
// Apply push-pull constraint verification in test
```

### Step 3: Remove Mock-Only Test
`input_pins_reject_set_output` only verifies `VirtualBoard` direction flag — tautological. Remove it.

### Step 4: Run preflight-check.sh
Execute `./scripts/preflight-check.sh` and verify output shows all ✅.

### Step 5: Fix ArtisanCommand::Stop (Optional)
Add `Stop` variant to `ArtisanCommand` enum — enables embedded build to succeed.

### Step 6: Final Verification
```bash
cargo test --target x86_64-unknown-linux-gnu --lib --test pinout_validation --test pin_digital_twin
./scripts/preflight-check.sh
```

---

## Success Criteria
- [ ] GPIO8 initialized in `init.rs`
- [ ] All 252+ tests pass
- [ ] `preflight-check.sh` prints "PREFLIGHT PASSED"
- [ ] Embedded build succeeds (if Step 5 done) or documents pre-existing error
- [ ] `PIN_TABLE` matches `init.rs` for all 11 pins
