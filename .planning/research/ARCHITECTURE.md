# Documentation Architecture Patterns

**Domain:** Documentation Update (LibreRoaster v4.1)
**Researched:** 2026-02-20

## Recommended Architecture

The architecture of the documentation system for LibreRoaster relies on updating existing `README.md` and `internalDoc/` files to accurately reflect the v4.1 codebase state (async paradigms, transport resilience, ESP32 firmware build instructions). No new infrastructure (like static site generators) is introduced; it remains a Markdown-based repository documentation model.

### Component Boundaries

| Component | Responsibility | Communicates With |
|-----------|---------------|-------------------|
| `README.md` | Primary entry point, high-level overview | Users, New Contributors |
| `internalDoc/` | Deep-dive design docs, protocols, hardware | Core Contributors |
| `examples/` | Demonstrable usage patterns | Users |
| `tests/` | Usage documentation via test suites | CI, Contributors |

### Integration Points

1.  **Codebase (src) -> Documentation:** The documentation must reflect the current state of `src/` (specifically the new async changes and transport resilience features).
2.  **Build System (`build-firmware.sh`, `Cargo.toml`) -> README:** Build and test instructions in the documentation must strictly map to the execution flows of the `build-firmware.sh` script.

## Data Flow

Since this is a documentation update, "data flow" represents the information flow for a developer reading the repo:

1.  **Entry:** Developer lands on `README.md`.
2.  **Setup:** Developer follows Build/Test Instructions (cargo build, esp-idf requirements).
3.  **Deep-Dive:** Developer refers to `internalDoc/PROTOCOL.md` and `ARCHITECTURE.md` for internal systems.
4.  **Reference:** Developer checks `examples/` for API consumption.

## Patterns to Follow

### Pattern 1: Single Source of Truth
**What:** Build instructions should not be duplicated across multiple files.
**When:** Writing build/test commands.
**Example:** Link to `build-firmware.sh` usage instead of copying exact shell commands if they change frequently, or provide the exact stable cargo wrapper commands.

### Pattern 2: Contextual Linking
**What:** `README.md` links out to `internalDoc/` files for deep-dives rather than bloating the main page.
**When:** Describing the protocol or hardware instrumentation.

## Anti-Patterns to Avoid

### Anti-Pattern 1: Orphaned Internal Docs
**What:** Having files in `internalDoc/` that are not referenced in the `README.md`.
**Why bad:** Contributors will not find them.
**Instead:** Ensure the main `README.md` has a "Documentation Directory" or "Internal Documentation" section linking to these assets.

### Anti-Pattern 2: Outdated Code Snippets
**What:** Leaving old synchronous code examples in `README.md` while the `src/` uses `async`.
**Why bad:** Breaks trust in documentation.
**Instead:** Review all inline code blocks against `examples/` and `src/`.

## Suggested Build Order (Phase Implementation)

1.  **Audit:** Identify all outdated `README.md` and `internalDoc/` content (specifically looking for old sync patterns).
2.  **Update README:** Rewrite main features, status, and Setup/Build instructions.
3.  **Update Internal Docs:** Refresh `ARCHITECTURE.md` or protocol files to reflect transport resilience.
4.  **Verification:** Test all build/test instructions mentioned in the updated documentation.

## Sources
- LibreRoaster codebase (`src/`, `examples/`, `tests/`)
- Existing `internalDoc/` directory structure
- LibreRoaster v4.1 Milestone Definitions
