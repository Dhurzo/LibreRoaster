# Phase 89: README Update - Research

**Researched:** 2026-03-10
**Domain:** README documentation for embedded Rust project
**Confidence:** HIGH

## Summary

This phase updates `README.md` to reflect current project status, recent changes, accurate build/test instructions, hardware/pinout information, and Artisan connection guidance. The README currently contains comprehensive information but needs updates for v5.0 completion, addition of STATUS/REG commands (per phase 68), alignment with internal documentation, and correction of pinout details.

**Primary recommendation:** Add a "Project Status" section at the top, update "Supported Artisan Commands" table with STATUS/REG rows, refresh "Pinout" table with exact GPIO mapping from constants.rs, ensure build/test instructions reference current scripts (quality-baseline.sh, run-regression-checks.sh), and maintain clear linkage to internalDoc/ARTISAN_CONNECTION.md.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Markdown | CommonMark | Format README.md | Universal for GitHub/GitLab documentation |
| `internalDoc/` | current | Internal documentation (ARCHITECTURE.md, PROTOCOL.md, etc.) | Source of truth for detailed information |
| `scripts/quality-baseline.sh` | current | Quality checks (fmt, clippy, test) | Standardized project quality workflow |
| `scripts/run-regression-checks.sh` | current | Regression test suite | Ensures automated regression testing |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `internalDoc/ARTISAN_CONNECTION.md` | current | Detailed Artisan connection guide | Referenced from README for connection specifics |
| `internalDoc/INSTRUMENTATION_README.MD` | current | STATUS CSV column definitions and regression notes | Essential for automation hook documentation |
| `src/config/constants.rs` | current | GPIO pin assignments and hardware constants | Source for accurate pinout table |
| `hardware.md` | current | Hardware documentation (Spanish) | Cross-reference for pinout and strapping pin notes |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Embedding full hardware details in README | Link to hardware.md | Keeps README concise but requires users to navigate |
| Writing new Artisan guide in README | Link to ARTISAN_CONNECTION.md | Avoids duplication, ensures single source of truth |
| Listing all recent commits in README | Summarize major changes | More readable for users, less maintenance |

**Installation:**
```bash
# No dependencies; update README.md using Markdown editor.
```

## Architecture Patterns

### Recommended README Structure
```
# LibreRoaster
- Project Status
- Core Value
- Features
- Async Architecture
- Supported Artisan Commands (updated with STATUS/REG)
- Quick Start
- Hardware Requirements
- Pinout (updated from constants.rs)
- Artisan Connection (link to internal doc)
- Protocol
- Project Structure
- Development (build/test instructions)
- Safety Features
- License
```

### Pattern 1: Project Status Section
**What:** Add a brief section at the top indicating current version, milestone completion, and upcoming goals.
**When to use:** After major milestones (v5.0) to inform users of project maturity.
**Example:**
```markdown
## Project Status

**Current version:** v5.0 (2026-03-10)
**Milestone:** v5.0 complete – Artisan protocol compatibility, instrumentation, safety features.
**Next:** v5.1 – Documentation update and minor improvements.

### Recent Changes
- Added STATUS command with 18‑field CSV for automation telemetry
- Added REG command for over‑temperature regression testing
- Enhanced watchdog instrumentation and safety logging
- Implemented quality baseline scripts (quality-baseline.sh)
- Refactored unsafe code and improved memory strategy
```

### Pattern 2: Hardware Pinout Table
**What:** Table mapping GPIO numbers to functions, with notes on strapping pins.
**When to use:** Always keep pinout synchronized with constants.rs.
**Example:** See Code Examples section.

### Anti-Patterns to Avoid
- **[Outdated pinout]** – Using incorrect GPIO numbers leads to wiring errors.
- **[Missing command aliases]** – STATUS/STAT alias must be documented.
- **[Broken links]** – Ensure internal documentation links are valid.
- **[Duplicate information]** – Do not copy entire internal docs; link instead.

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Describing STATUS CSV columns | Write new column descriptions in README | Reference `internalDoc/INSTRUMENTATION_README.MD` | Avoids duplication and drift from implementation |
| Creating new hardware diagrams | Draw custom pinout diagram | Use existing table from hardware.md (or export) | Saves time, ensures accuracy |
| Writing full Artisan connection tutorial | Create new guide | Link to `internalDoc/ARTISAN_CONNECTION.md` | Already comprehensive and maintained |
| Generating changelog in README | Manually list commits | Summarize major changes; refer to git history | Reduces maintenance burden |

**Key insight:** The README should serve as a portal to detailed documentation, not replace it. Focus on discoverability and accuracy.

## Common Pitfalls

### Pitfall 1: Incorrect GPIO mapping
**What goes wrong:** Users wire components incorrectly because README pinout doesn't match firmware.
**Why it happens:** Pin assignments changed in constants.rs but README wasn't updated.
**How to avoid:** Extract pinout table directly from constants.rs (or verify manually).
**Warning signs:** Discrepancies between README and hardware.md.

### Pitfall 2: Missing automation hooks
**What goes wrong:** Automation engineers miss STATUS/REG commands because they're not in the command table.
**Why it happens:** README not updated after phase 68.
**How to avoid:** Ensure Supported Artisan Commands table includes REG and STATUS/STAT rows with links to instrumentation guide.
**Warning signs:** No mention of STATUS or REG in README.

### Pitfall 3: Outdated build instructions
**What goes wrong:** Build/test commands fail because they reference old scripts or flags.
**Why it happens:** Scripts evolved but README wasn't refreshed.
**How to avoid:** Validate each command against current scripts.
**Warning signs:** README mentions deprecated flags (e.g., `--features std` without `--target`).

### Pitfall 4: Broken internal links
**What goes wrong:** Links to internal documentation lead to missing files.
**Why it happens:** File renamed or moved.
**How to avoid:** Test all links after updates.
**Warning signs:** 404 errors in rendered README.

## Code Examples

### Project Status Section Example
```markdown
## Project Status

**Current version:** v5.0 (2026-03-10)
**Milestone:** v5.0 complete – Artisan protocol compatibility, instrumentation, safety features.
**Next:** v5.1 – Documentation update and minor improvements.

### Recent Changes
- Added STATUS command with 18‑field CSV for automation telemetry
- Added REG command for over‑temperature regression testing
- Enhanced watchdog instrumentation and safety logging
- Implemented quality baseline scripts (quality‑baseline.sh)
- Refactored unsafe code and improved memory strategy
- Updated all internal documentation (ARCHITECTURE.md, PROTOCOL.md, etc.)
```

### Pinout Table Example (derived from constants.rs)
```markdown
| GPIO | Function | Description | Note |
|------|----------|-------------|------|
| 3 | MAX31856 #1 CS | Environment Temperature (ET) | SPI bus shared |
| 4 | MAX31856 #2 CS | Bean Temperature (BT) | SPI bus shared |
| 5 | SPI MOSI | Master Out Slave In | Shared between MAX31856 chips |
| 6 | SPI MISO | Master In Slave Out | Shared |
| 7 | SPI SCLK | Serial Clock | Shared |
| 9 | Fan PWM | Fan speed control (25kHz) | Strapping pin – safe with pull‑up |
| 10 | SSR PWM | Heater control (1Hz) | |
| 1 | Heat Detection | SSR feedback input | Pull‑up enabled |
| 20 | UART TX | Artisan communication (to Artisan) | |
| 21 | UART RX | Artisan communication (from Artisan) | |
```

### Updated Supported Artisan Commands Table
```markdown
| Command | Description |
|---------|-------------|
| `READ` | Request telemetry (ET, BT, heater%, fan%) |
| `STATUS`/`STAT` | Automation telemetry snapshot (18‑field CSV with watchdog/guard/regression metrics) – see `internalDoc/INSTRUMENTATION_README.MD` for column definitions |
| `REG` | Over‑temperature regression trigger – emits `SAFETY OT‑REGRESSION` log |
| `OT1 [0‑100]` | Set heater power percentage |
| `OT2 [0‑100]` | Set fan speed percentage (auto‑cuts heater if out of range) |
| `IO3 [0‑100]` | Set fan speed percentage |
| `UP` | Increase heater by 5% |
| `DOWN` | Decrease heater by 5% |
| `START` | Begin roasting, enable continuous output |
| `STOP` | Emergency stop, disable outputs |
| `CHAN [rate]` | Set communication rate (legacy) |
| `UNITS [C/F]` | Set temperature units (Celsius/Fahrenheit) |
| `FILT [value]` | Set filter value (legacy) |
```

### Build/Test Instructions Example
```markdown
### Prerequisites
- Rust stable toolchain (install via rustup)
- ESP32‑C3 target: `rustup target add riscv32imc‑unknown‑none‑elf`
- espflash: `cargo install espflash`

### Build Commands
```bash
# Build for ESP32‑C3
cargo build --release --target riscv32imc‑unknown‑none‑elf

# Build for host tests
cargo build --target x86_64‑unknown‑linux‑gnu
```

### Test Commands
```bash
# Run all unit tests
cargo test

# Run host integration tests
cargo test --target x86_64‑unknown‑linux‑gnu --test command_multiplexer_concurrency
cargo test --features async‑lock‑depth‑metrics --target x86_64‑unknown‑linux‑gnu --test concurrent_sensor_test
cargo test --target x86_64‑unknown‑linux‑gnu --test artisan_integration_test

# Quality baseline (fmt, clippy, test)
scripts/quality‑baseline.sh

# Regression suite
scripts/run‑regression‑checks.sh
```

### Artisan Connection Summary
```markdown
## Artisan Connection

LibreRoaster supports two connection methods:

| Method | Description |
|--------|-------------|
| **USB CDC** | Native ESP32‑C3 USB (recommended) – no adapter needed |
| **UART0** | GPIO20/21 at 115200 baud – requires USB‑to‑UART adapter |

**Key settings:** 115200 baud, Arduino/RPi mode, no handshake required.

For detailed wiring diagrams, port identification, and troubleshooting, see [ARTISAN_CONNECTION.md](internalDoc/ARTISAN_CONNECTION.md).
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| README lacked project status section | Add Project Status with version and recent changes | 2026‑03‑10 | Users immediately understand project maturity |
| Pinout table generic (GPIO 5‑7 as "SPI") | Specific GPIO mapping with notes | 2026‑03‑10 | Accurate wiring guidance |
| Missing STATUS/REG commands in table | Added rows with instrumentation links | 2026‑02‑23 (phase 68) | Automation hooks discoverable |
| Build instructions omitted host test commands | Include host integration and regression test commands | 2026‑03‑10 | Clear testing workflow |
| Artisan connection details embedded in README | Link to dedicated guide | 2026‑03‑10 | Reduced duplication, easier maintenance |

**Deprecated/outdated:**
- Rust toolchain version pinned to v1.88 (now stable)
- Generic SPI pin description (now specific MOSI/MISO/SCLK)

## Open Questions

1. **Should the pinout table include strapping pin warnings?**
   - What we know: GPIO9 is a strapping pin used for fan PWM; hardware.md indicates it's safe with pull‑up.
   - What's unclear: Whether users need explicit warning about boot mode.
   - Recommendation: Include a note column with brief warning and reference hardware.md.

2. **How detailed should recent changes be?**
   - What we know: Git log shows many commits; summarizing major features is sufficient.
   - What's unclear: Whether to include links to specific commits or issues.
   - Recommendation: Keep bullet list high‑level; refer to git history for details.

## Sources

### Primary (HIGH confidence)
- `README.md` (current) – existing structure and content
- `internalDoc/ARCHITECTURE.md` – updated 2026‑03‑10, reflects v5.0
- `internalDoc/PROTOCOL.md` – updated 2026‑03‑10
- `internalDoc/ARTISAN_CONNECTION.md` – detailed connection guide
- `internalDoc/INSTRUMENTATION_README.MD` – STATUS CSV definitions
- `src/config/constants.rs` – GPIO pin assignments
- `hardware.md` – hardware documentation with pinout notes
- `scripts/quality‑baseline.sh`, `run‑regression‑checks.sh` – current test workflows

### Secondary (MEDIUM confidence)
- Git log (recent commits) – infer recent changes
- Phase 68 research – command table updates

### Tertiary (LOW confidence)
- External README best practices (makeareadme.com) – general guidance

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH – Markdown and internal docs are well‑understood
- Architecture: HIGH – README structure follows established patterns
- Pitfalls: HIGH – based on observable discrepancies
- Pinout: HIGH – derived from constants.rs and hardware.md
- Build/test instructions: HIGH – validated against existing scripts

**Research date:** 2026‑03‑10
**Valid until:** 2026‑04‑10 (30 days for stable documentation)