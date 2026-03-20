# 95-build-verification

Documented audit run that proves `cargo build --release --target riscv32imc-unknown-none-elf --features embedded` still produces the ELF and flash artifacts claimed by BUILD-01/Phase 95.

## Build fingerprint

- **Run started:** 2026-03-20T17:04:33Z (UTC)
- **rustc:** `rustc 1.92.0 (ded5c06cf 2025-12-08)`
- **cargo:** `cargo 1.92.0 (344c4567c 2025-10-21)`
- **Commit:** `87f7c46a601b4479d8ed9645fa1217624ee1dae2` (see log for full SHA)

## Command trace

### `cargo build --release --target riscv32imc-unknown-none-elf --features embedded`

- Standard `cargo build` flow issued from repo root; the log highlights a handful of existing unused-import/unused-static warnings but the compile succeeds with the ELF produced.
- Full console output captured in `.planning/execution/99-01/task1.log` for auditors who want to replay or inspect the exact timing, warnings, and target directories.

### Artifact snapshot: `target/riscv32imc-unknown-none-elf/release/libreroaster`

- `ls -lh` reports `-rwxrwxr-x 2 juan juan 3,0M ... target/riscv32imc-unknown-none-elf/release/libreroaster`, confirming a 3.0 MB ELF binary matching Phase 95 claims.

### `espflash save-image --chip esp32c3 target/riscv32imc-unknown-none-elf/release/libreroaster libreroaster.bin`

- espflash reports the ESP32-C3 chip type, no merge/padding flags, and `App/part. size: 164,672/4,128,768 bytes (3.99%)` before logging `[2026-03-20T17:04:36Z INFO ] Image successfully saved!`.
- `ls -lh` shows `-rw-rw-r-- 1 juan juan 161K ... libreroaster.bin`, so the flashable binary is 161 KB.

## Artifacts for auditors

- `target/riscv32imc-unknown-none-elf/release/libreroaster` – verified ELF binary referenced by BUILD-01.
- `libreroaster.bin` – the flash-ready image produced by the espflash command.
- `.planning/execution/99-01/task1.log` – raw build + espflash output plus environment fingerprint so auditors can quote the run verbatim.

## BUILD-01 context

- This run revalidates the exact embedded build command celebrated in Phase 95's summary (`.planning/phases/95-fix-critical-build-blockers/95-01-SUMMARY.md`), so the audit artifact can be cited whenever README/DEVELOPMENT references BUILD-01.
- Both `README.md` and `internalDoc/DEVELOPMENT.md` already document this command (`README.md` under “Building for ESP32-C3” and `internalDoc/DEVELOPMENT.md` under “Building Firmware”), so the audit log ties the documented workflow directly back to the proved binary production.

## References

1. Phase 95 summary: `.planning/phases/95-fix-critical-build-blockers/95-01-SUMMARY.md`
2. Public build instructions: `README.md` (see the “Building for ESP32-C3” section that lists the same command)
3. Internal development guide: `internalDoc/DEVELOPMENT.md` (build section mirrors the verified command)
