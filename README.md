# Kiln — Open-Source Interactive Disassembler

An IDA Pro-style interactive disassembler built entirely in Rust.

## Features (MVP)

- **Binary Loading**: ELF, PE, and Mach-O format support via `goblin`
- **Multi-Architecture Disassembly**: x86, x86-64, ARM, AArch64, RISC-V via `capstone-rs`
- **Linear Sweep Disassembly**: Disassemble all executable sections
- **Project Persistence**: Save/load annotations (comments, labels) via `serde`/`bincode`
- **CLI Harness**: Inspect binary metadata and disassembly from the command line

## Architecture

```
kiln/
├── Cargo.toml              # workspace root
├── crates/
│   ├── kiln-core/           # binary loading, disassembly, analysis (library)
│   ├── kiln-gui/            # egui/eframe GUI application (binary, WIP)
│   └── kiln-project/        # project save/load, annotations (library)
├── assets/                  # icons, fonts
├── LICENSE
└── README.md
```

### Key Dependencies

| Crate | Purpose |
|-------|---------|
| `goblin` | ELF, PE, Mach-O binary parsing (pure Rust) |
| `capstone` | Multi-arch disassembly (x86, ARM, RISC-V) |
| `eframe`/`egui` | Immediate-mode GUI framework |
| `serde` + `bincode` | Project serialization |
| `rfd` | Native file dialogs |

## Building

```bash
cargo build
```

## Running the CLI

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

## Running Tests

```bash
cargo test
```

## Project Status

- [x] Sprint 1: Project scaffolding & binary loading
- [x] Sprint 2: Linear disassembly engine
- [ ] Sprint 3: GUI shell & hex view
- [ ] Sprint 4: Disassembly listing view
- [ ] Sprint 5: Control flow analysis
- [ ] Sprint 6: Navigation & search
- [ ] Sprint 7: Annotations & project persistence
- [ ] Sprint 8: Basic CFG graph view
- [ ] Sprint 9: Strings, imports/exports, multi-arch polish
- [ ] Sprint 10: Polish, CI & first release

## License

MIT
