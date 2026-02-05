# Stack Research

**Domain:** Rust Embedded Documentation Tools
**Researched:** 2026-02-05
**Confidence:** HIGH

## Recommended Stack

### Core Documentation Tools

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **mdbook** | 0.5.x | Markdown-based book/documentation generator | Industry standard for Rust projects; used by rust-lang.org, embedded WG; 7.7M+ downloads; low maintenance, simple workflow |
| **cargo-readme** | 3.3.x | Generate README.md from doc comments | Keeps documentation in sync with code; 703K downloads; supports embedded-friendly patterns; no_std compatible for library docs |
| **rustdoc** | Built-in | API documentation from doc comments | Part of Cargo, no setup required; validates code examples; integrates with docs.rs hosting |

### Supporting Tools

| Tool | Purpose | When to Use |
|------|---------|-------------|
| **mdbook-rust-doc** | Embed Rust doc comments into mdBook chapters | For detailed API documentation integrated with prose; useful when you want code examples alongside narrative |
| **svd2rust** | Generate Rust API from SVD files | For peripheral register documentation in embedded projects; LibreRoaster uses ESP32-C3 SVD files |
| **cargo-generate** | Project template generator | For consistent documentation structure across new embedded projects |

## Why This Stack for LibreRoaster

### mdbook for Main Documentation
mdbook is the standard tool in the Rust ecosystem for project documentation. The embedded Rust Working Group uses it for all their books (docs.rust-embedded.org). For LibreRoaster, this means:
- **Low maintenance**: Simple markdown files, no complex build pipelines
- **Embedded-friendly**: Works perfectly with no_std projects since it generates HTML, not code
- **Search and navigation**: Built-in search, navigation sidebar, syntax highlighting
- **GitHub Pages deployment**: Single command to build, deploy to GitHub Pages

### cargo-readme for README Generation
For firmware projects with public crates, cargo-readme keeps your README synchronized with source code documentation:
```bash
cargo readme > README.md
```
This ensures the README on crates.io and GitHub matches the actual crate documentation. Works with no_std since it only processes doc comments, not runtime code.

### rustdoc for API Docs
The built-in `cargo doc` command generates API documentation from `///` doc comments. For embedded projects:
- Use `#[doc(hidden)]` for internal-only docs
- Configure `docs.rs` metadata for proper hosting
- Works with no_std when using `#![no_std]` attribute on crates

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|------------------------|
| mdbook | GitHub Wiki | Only for trivial projects without CI/CD; lacks search, version control, and deployment automation |
| cargo-readme | Manual README | For single-person projects with infrequent updates; manual is simpler initially but diverges over time |
| mdbook | Docusaurus/other | For teams already using JS frameworks; adds maintenance burden without Rust ecosystem benefits |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| **GitBook** | Non-Rust tool, requires JS dependencies, adds maintenance overhead | mdbook - native Rust, no external dependencies |
| **AsciiDoctor** | More complex setup, Ruby dependencies, less Rust integration | mdbook - Cargo integration, simpler workflow |
| **Custom doc generators** | Maintenance burden, potential bitrot | rustdoc/cargo-doc - supported by Rust team |

## Installation

```bash
# mdbook - documentation site generator
cargo install mdbook

# cargo-readme - README generator from doc comments
cargo install cargo-readme

# cargo-generate - project templates
cargo install cargo-generate

# For ESP32-C3 peripheral docs (optional)
cargo install svd2rust
```

### Optional Preprocessors

```bash
# Embed Rust docs in mdbook
cargo install mdbook-rust-doc
```

## LibreRoaster-Specific Configuration

### mdbook Configuration (book.toml)

```toml
[book]
title = "LibreRoaster Documentation"
authors = ["Your Name"]
description = "ESP32-C3 firmware with Artisan serial protocol compatibility"

[build]
build-dir = "docs"
```

### cargo-readme Setup

Add to `Cargo.toml`:
```toml
[package.metadata.cargo-readme]
# Custom templates available
```

### GitHub Actions for Documentation

```yaml
name: Build Documentation
on:
  push:
    branches: [main]
    paths: ['**.md', 'book.toml', 'src/**']

jobs:
  docs:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Build mdBook
        run: |
          cargo install mdbook
          mdbook build docs/
      - name: Deploy to Pages
        uses: peaceiris/actions-gh-pages@v4
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./docs/book
```

## Stack Patterns by Project Type

**For single-crate embedded libraries:**
- `cargo doc` for API docs
- `cargo-readme` for README
- No mdbook needed unless you have extensive tutorials

**For firmware applications with documentation:**
- `mdbook` for main documentation
- `cargo doc` for API docs
- `cargo-readme` for crate-level README
- Deploy mdbook to GitHub Pages

**For complex embedded projects (like LibreRoaster):**
- `mdbook` for architecture, usage, and development guides
- `cargo doc` for API reference
- `svd2rust` for hardware register documentation if needed
- Keep `internalDoc/` as supplementary markdown files that can be integrated into mdBook later

## Version Compatibility

| Tool | Rust Version | Notes |
|------|--------------|-------|
| mdbook 0.5.x | 1.56+ | Current stable, well-tested |
| cargo-readme 3.3.x | 1.56+ | Works with all modern Rust |
| svd2rust 0.37.x | 1.61+ (MSRV) | Check ESP32-C3 SVD compatibility |

## LibreRoaster Documentation Structure Recommendation

```
LibreRoaster/
├── README.md              # Auto-generated from lib crate docs
├── CHANGELOG.md           # Manual, updated per release
├── docs/                  # mdBook output (gitignored)
├── book/                  # mdBook source
│   ├── src/
│   │   ├── README.md      # Project overview
│   │   ├── architecture.md # System design
│   │   ├── usage.md       # User guide
│   │   └── development.md # Contributor guide
│   └── book.toml          # mdBook config
└── internalDoc/           # Legacy, migrate to book/ as needed
```

## Sources

- **mdBook Documentation** — https://rust-lang.github.io/mdBook/ — Official documentation, 7.7M+ downloads, industry standard
- **cargo-readme on crates.io** — https://crates.io/crates/cargo-readme — 703K downloads, actively maintained
- **Embedded Rust Bookshelf** — https://docs.rust-embedded.org/ — Uses mdbook for all documentation, validates approach for embedded projects
- **Rustdoc Book** — https://doc.rust-lang.org/rustdoc/ — Built-in documentation system, part of Cargo
- **mdbook-rust-doc** — https://github.com/mythmon/mdbook-rust-doc — For embedding Rust API docs in mdBook chapters

---

*Stack research for: Rust Embedded Documentation Tools*
*Researched: 2026-02-05*
