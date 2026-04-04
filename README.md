# Kiln — Open-Source Interactive Disassembler

An IDA Pro-style interactive disassembler built entirely in Rust.

![Rust](https://img.shields.io/badge/language-Rust-orange)
![License](https://img.shields.io/badge/license-AGPL--3.0-blue)

> **Note:** Screenshots coming soon. Kiln features a full egui-based GUI with hex view, disassembly listing, CFG graph, and more.

## Features

- **Binary Loading** — ELF, PE, and Mach-O format support via `goblin`
- **Multi-Architecture Disassembly** — x86, x86-64, ARM, AArch64, RISC-V via `capstone-rs`
- **Linear & Recursive Descent Disassembly** — Full executable section analysis with control flow recovery
- **Hex View** — Virtual-scrolling hex dump with offset / hex bytes / ASCII columns
- **Disassembly View** — Syntax-colored listing with addresses, bytes, mnemonics, operands, and inline symbols
- **Control Flow Graph** — Interactive per-function CFG visualization
- **Strings View** — Extracted strings with section, offset, and clickable navigation
- **Imports & Exports Views** — Browse imported and exported symbols
- **Annotations** — Add comments (`;`) and rename labels (`N`) at any address
- **Project Persistence** — Save/load `.kproj` project files with all annotations
- **Undo / Redo** — Full undo/redo for all annotation changes (Ctrl+Z / Ctrl+Y)
- **Navigation** — Go-to-address (Ctrl+G), back/forward history (Alt+←/→), clickable addresses
- **Search** — Text search across mnemonics/operands and hex byte pattern search (Ctrl+F)
- **Cross-References** — Xref database with clickable to/from references
- **Dark / Light Theme** — Toggle via View menu
- **Keyboard Shortcut Help** — Press F1 for a full shortcut cheat sheet
- **CLI Harness** — Inspect binary metadata and disassembly from the command line

## Architecture

```
kiln/
├── Cargo.toml               # Workspace root
├── crates/
│   ├── kiln-core/            # Binary loading, disassembly, control flow analysis (library)
│   │   ├── src/
│   │   │   ├── lib.rs        # Public API: load_binary()
│   │   │   ├── model.rs      # BinaryImage, Section, Symbol, Instruction
│   │   │   ├── disasm.rs     # Capstone-based disassembly engine
│   │   │   └── analysis.rs   # AnalysisDatabase, CFG, functions, xrefs
│   │   └── examples/
│   │       └── cli.rs        # CLI harness
│   ├── kiln-gui/             # egui/eframe GUI application (binary)
│   │   └── src/
│   │       ├── main.rs       # eframe entry point
│   │       ├── app.rs        # Application state, menus, dialogs, shortcuts
│   │       └── views/        # View modules (hex, disasm, graph, strings, imports, exports)
│   └── kiln-project/         # Project save/load, annotations (library)
│       └── src/
│           └── lib.rs        # Project, Annotation, save/load with bincode
├── CONTRIBUTING.md
├── CHANGELOG.md
├── LICENSE                   # AGPL-3.0
└── README.md
```

### Key Dependencies

| Crate | Purpose |
|-------|---------|
| `goblin` | ELF, PE, Mach-O binary parsing (pure Rust) |
| `capstone` | Multi-arch disassembly (x86, ARM, RISC-V) |
| `eframe`/`egui` | Immediate-mode GUI framework |
| `egui_extras` | Extra widgets (tables, etc.) |
| `serde` + `bincode` | Project serialization |
| `rfd` | Native file dialogs |

## Building

### Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- A C compiler (for `capstone-sys`): `gcc`/`clang` on Linux/macOS, MSVC on Windows
- On Linux: `libgtk-3-dev` (for native file dialogs)

### Build

```bash
cargo build --release
```

### Run the GUI

```bash
cargo run -p kiln-gui --release
```

### Run the CLI

```bash
cargo run -p kiln-core --example cli -- /path/to/binary
```

Example output:
```
Loading binary: /bin/ls
============================================================
File:           ls
Format:         ELF
Architecture:   x86-64
Bits:           64
Entry Point:    0x0000000000006d30
...
```

## Usage

### GUI

1. **Open a binary**: `File → Open` or `Ctrl+O`
2. **Browse views**: Switch between Hex, Disassembly, Graph, Strings, Imports, Exports tabs
3. **Navigate**: Click addresses to follow, use `Ctrl+G` to go to a specific address, `Alt+←/→` for back/forward
4. **Search**: `Ctrl+F` to search mnemonics/operands or hex byte patterns
5. **Annotate**: Press `;` to add a comment, `N` to rename a symbol at the selected address
6. **Save project**: `Ctrl+S` to save annotations as a `.kproj` file
7. **Undo/Redo**: `Ctrl+Z` / `Ctrl+Y`
8. **Help**: Press `F1` for the keyboard shortcut cheat sheet

### CLI

The CLI harness prints binary metadata (format, architecture, sections, symbols, entry point) and a disassembly listing:

```bash
cargo run -p kiln-core --example cli -- /usr/bin/ls
```

## Running Tests

```bash
cargo test
```

## Project Status

- [x] Sprint 1: Project scaffolding & binary loading
- [x] Sprint 2: Linear disassembly engine
- [x] Sprint 3: GUI shell & hex view
- [x] Sprint 4: Disassembly listing view
- [x] Sprint 5: Control flow analysis
- [x] Sprint 6: Navigation & search
- [x] Sprint 7: Annotations & project persistence
- [x] Sprint 8: Basic CFG graph view
- [x] Sprint 9: Strings, imports/exports, multi-arch polish
- [x] Sprint 10: Polish, CI & first release

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for build instructions, code style, and how to submit changes.

## License

AGPL-3.0 — see [LICENSE](LICENSE) for details.
