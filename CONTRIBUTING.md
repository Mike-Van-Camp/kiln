# Contributing to Kiln

Thank you for your interest in contributing to Kiln! This document covers everything you need to get started.

## Building from Source

### Prerequisites

- **Rust** stable toolchain — install via [rustup](https://rustup.rs/)
- **C compiler** — required for the `capstone-sys` build dependency
  - Linux: `gcc` or `clang` (usually pre-installed)
  - macOS: Xcode command line tools (`xcode-select --install`)
  - Windows: MSVC via Visual Studio Build Tools
- **Linux only**: `libgtk-3-dev` for native file dialogs (`sudo apt install libgtk-3-dev`)

### Build

```bash
git clone https://github.com/Mike-Van-Camp/kiln.git
cd kiln
cargo build
```

For an optimized release build:

```bash
cargo build --release
```

## Running Tests

```bash
cargo test
```

All tests must pass before submitting a pull request.

## Code Style

We use the standard Rust formatting and linting tools:

### Formatting

```bash
cargo fmt
```

All code must be formatted with `rustfmt`. Check formatting without modifying files:

```bash
cargo fmt --check
```

### Linting

```bash
cargo clippy --all-targets
```

All clippy warnings must be resolved. Do not use `#[allow(...)]` unless there is a clear justification documented in a comment.

## Submitting Changes

### Pull Requests

1. **Fork** the repository and create a feature branch from `main`.
2. Make your changes in focused, well-scoped commits.
3. Ensure `cargo fmt`, `cargo clippy --all-targets`, `cargo test`, and `cargo build` all pass.
4. Open a pull request with a clear title and description of what changed and why.
5. Reference any related issues in the PR description (e.g., "Fixes #42").

### Commit Messages

- Use clear, imperative-style messages (e.g., "Add hex search to disasm view").
- Keep the first line under 72 characters.
- Add a blank line before any extended description.

## Reporting Issues

When opening an issue, please include:

- **Summary**: A brief description of the bug or feature request.
- **Steps to reproduce** (for bugs): What binary you loaded, what you clicked, what happened.
- **Expected behavior**: What you expected to happen.
- **Actual behavior**: What actually happened, including any error messages.
- **Environment**: OS, Rust version (`rustc --version`), Kiln version.

## Architecture Overview

Kiln is organized as a Cargo workspace with three crates:

| Crate | Type | Purpose |
|-------|------|---------|
| `kiln-core` | Library | Binary loading (`goblin`), disassembly (`capstone`), control flow analysis, data model |
| `kiln-gui` | Binary | Interactive GUI application built with `egui`/`eframe` |
| `kiln-project` | Library | Project persistence — save/load annotations and metadata as `.kproj` files |

### Data Flow

1. **kiln-core** loads a binary file into a `BinaryImage` (sections, symbols, raw data).
2. **kiln-core** disassembles executable sections into `Instruction` objects.
3. **kiln-core** runs control flow analysis to build `Function` and `BasicBlock` structures plus a cross-reference database.
4. **kiln-gui** renders the data through six view modules (hex, disasm, graph, strings, imports, exports).
5. **kiln-project** manages annotations (comments, labels) and project save/load.

### Key Source Files

- `crates/kiln-core/src/model.rs` — Core data types (`BinaryImage`, `Section`, `Symbol`, `Instruction`)
- `crates/kiln-core/src/disasm.rs` — Capstone-based disassembly engine
- `crates/kiln-core/src/analysis.rs` — `AnalysisDatabase`, CFG construction, function detection, xrefs
- `crates/kiln-gui/src/app.rs` — Main application state, menus, dialogs, keyboard shortcuts
- `crates/kiln-gui/src/views/` — Individual view modules (hex_view, disasm_view, graph_view, etc.)
- `crates/kiln-project/src/lib.rs` — `Project` struct, annotations, bincode serialization

## License

By contributing to Kiln, you agree that your contributions will be licensed under the project's [AGPL-3.0 license](LICENSE).
