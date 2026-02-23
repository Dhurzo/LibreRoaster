# Phase 64 Documentation Consistency Verification

## Evidence

`grep -n "target/riscv32imc-unknown-none-elf/release/libreroaster.bin" internalDoc/FLASH_GUIDE.md`
63:2. Browse to the firmware binary (usually `target/riscv32imc-unknown-none-elf/release/libreroaster.bin`)
72:espflash flash target/riscv32imc-unknown-none-elf/release/libreroaster.bin
75:espflash flash --port /dev/ttyACM0 target/riscv32imc-unknown-none-elf/release/libreroaster.bin
78:espflash flash --monitor target/riscv32imc-unknown-none-elf/release/libreroaster.bin
99:# The binary will be at target/riscv32imc-unknown-none-elf/release/libreroaster.bin
162:espflash flash --partition-table partitions.csv target/riscv32imc-unknown-none-elf/release/libreroaster.bin
170:espflash flash --dual-bank target/riscv32imc-unknown-none-elf/release/libreroaster.bin

`grep -n "riscv32imc-unknown-none-elf" README.md`
200:   rustup target add riscv32imc-unknown-none-elf
227:cargo build --release --target riscv32imc-unknown-none-elf
230:**Output location:** `target/riscv32imc-unknown-none-elf/release/libreroaster.bin`
232:> **Note:** The `--target riscv32imc-unknown-none-elf` flag is required because LibreRoaster is an embedded application (no stdlib), not a host application.

`rg -n -i "macOS" README.md | head -n 3`
76:1. Identify the USB port (ttyACM on Linux, /dev/cu.usbmodem-* on macOS, COM on Windows)

## Summary

### Evidence-backed conclusions

1. `internalDoc/FLASH_GUIDE.md` now cites every `target/riscv32imc-unknown-none-elf/release/libreroaster.bin` path that anchors the binary build and flash guidance, and the Evidence section above lists the exact grep command that returned each line number.
2. `README.md` confirms the granite target triple (`riscv32imc-unknown-none-elf`) across the toolchain install, build, and output sections while also surfacing the macOS `/dev/cu.usbmodem-*` port path described in the audit; the Evidence block ties each statement back to the README grep output so reviewers can see the source lines.

### Audit closure

This report proves the Phase 64 documentation consistency fixes with concrete grep traces so the milestone audit can mark the binary path, target triple, and macOS port claims as verified. The README and `internalDoc/FLASH_GUIDE.md` entries referenced here are the same files cited in the audit trace, satisfying the required evidence trail and closing the audit gap for Phase 64.
