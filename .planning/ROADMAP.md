# Project Roadmap

**Milestone:** v4.1 Documentation Update — **SHIPPED** ✓

This roadmap outlines the phases for updating the project documentation to reflect the current codebase state and provide clear build/test instructions.

## Phases

### Phase 62: Documentation Cleanup
**Goal:** Remove outdated information and align documentation with the current codebase state.
**Requirements:** CLN-01, CLN-02, CLN-03
**Dependencies:** None
**Plans:** 3 plans

**Plans:**
- [x] 62-01-PLAN.md — Clean up README.md (outdated info, async/safety docs)
- [x] 62-02-PLAN.md — Clean up internalDoc/ (broken links, outdated info)
- [x] 62-03-PLAN.md — Fix broken links (create missing FLASH_GUIDE.md and ARTISAN_CONNECTION.md)

**Success Criteria:**
- User cannot find any outdated Artisan command information in the README.
- User cannot find any outdated pinout or hardware information in the README.
- User can read documentation that accurately describes the latest async and safety improvements.

### Phase 63: Build and Test Documentation
**Goal:** Provide clear instructions for developers to build, test, and run the project.
**Requirements:** BLD-01, BLD-02, BLD-03
**Dependencies:** Phase 62
**Plans:** 1 plan

**Plans:**
- [x] 63-01-PLAN.md — Add comprehensive build, test, and development flags documentation

**Success Criteria:**
- User can follow step-by-step instructions to successfully build the firmware.
- User can run the test suite and host integration tests using the provided commands.
- User can understand and use development flags like `async-lock-depth-metrics` based on the documentation.

## Progress

| Phase | Goal | Status |
|-------|------|--------|
| 62 | Remove outdated information and align documentation | Complete |
| 63 | Provide clear instructions for developers to build, test, and run the project | Complete |
| 64 | Fix documentation inconsistencies identified in audit | Complete |

### Phase 64: Documentation Consistency Fixes
**Goal:** Fix documentation inconsistencies identified in audit.
**Requirements:** N/A (tech debt closure)
**Dependencies:** Phase 63

**Plans:** 1 plan

**Plans:**
- [x] 64-01-PLAN.md — Fix documentation inconsistencies (binary paths, target name, macOS ports)

**Gap Closure:** Closes audit gaps:
- FLASH_GUIDE.md binary path fix (target/riscv32imc-unknown-none-elf/release)
- README.md target name fix (riscv32imc)
- README.md macOS port reference
