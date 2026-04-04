# Plan: Kiln — Open-Source Interactive Disassembler in Rust

## TL;DR
Build an IDA Pro-style interactive disassembler called **Kiln**, fully in Rust, using `egui` for GUI, `capstone-rs` for multi-arch disassembly, and `goblin` for binary parsing. Organized as 1-week SCRUM sprints, MVP target ~10 sprints.

## Architecture

### Workspace layout (Cargo workspace)
```
kiln/
├── Cargo.toml              # workspace root
├── crates/
│   ├── kiln-core/           # binary loading, disassembly, analysis (library)
│   ├── kiln-gui/            # egui/eframe GUI application (binary)
│   └── kiln-project/        # project save/load, annotations (library)
├── assets/                  # icons, fonts
├── LICENSE
└── README.md
```

### Key dependencies
- `goblin` — ELF, PE, Mach-O parsing (pure Rust)
- `capstone-rs` — x86/x64, ARM/AArch64, RISC-V disassembly
- `eframe` / `egui` — immediate-mode GUI (most popular Rust GUI framework)
- `egui_extras` — table widget for disassembly listing
- `serde` + `bincode` — project serialization
- `rfd` — native file dialogs
- `log` + `env_logger` — logging

### Core data model (kiln-core)
- `BinaryImage` — loaded binary with sections, segments, entry point, symbols
- `Instruction` — address, bytes, mnemonic, operands, architecture
- `Function` — name, entry address, basic blocks, xrefs
- `BasicBlock` — start/end address, instruction list, successors/predecessors
- `CrossReference` — from_addr, to_addr, xref_type (call/jump/data)
- `AnalysisDatabase` — central store for all analysis results

---

## Sprint Backlog

### Sprint 1 — Project Scaffolding & Binary Loading
**Goal:** Load any ELF/PE/Mach-O binary and extract metadata.

- [ ] Initialize Cargo workspace with `kiln-core`, `kiln-gui`, `kiln-project` crates
- [ ] Integrate `goblin` in kiln-core; implement `BinaryImage` struct
- [ ] Parse sections, segments, entry point, imported/exported symbols
- [ ] Auto-detect architecture and format from binary headers
- [ ] Write unit tests: load sample ELF, PE, Mach-O; verify parsed metadata
- [ ] Temporary CLI harness to print loaded binary info

**Verification:** `cargo test` passes; CLI prints correct header/section info for test binaries.

---

### Sprint 2 — Linear Disassembly Engine
**Goal:** Disassemble executable sections into instruction streams.

- [ ] Integrate `capstone-rs`; create `Disassembler` trait + x86/x64 implementation
- [ ] Define `Instruction` data model (address, raw bytes, mnemonic, operand string, size)
- [ ] Linear sweep disassembly of `.text` / executable sections
- [ ] Section-aware disassembly (skip non-code sections)
- [ ] Add ARM/AArch64 and RISC-V disassembler implementations behind same trait
- [ ] Unit tests with known byte sequences → expected mnemonics

**Verification:** Disassemble a small known binary; output matches `objdump -d`.

---

### Sprint 3 — GUI Shell & Hex View
**Goal:** Launchable GUI app that opens a binary and shows raw hex.

- [ ] Set up `eframe` application in kiln-gui with main window frame
- [ ] File → Open dialog using `rfd` crate
- [ ] Hex view panel: virtual-scrolling hex dump (addr | hex bytes | ASCII)
- [ ] Section selector sidebar (click section → jump hex view)
- [ ] Status bar: file name, format, architecture, size
- [ ] Wire binary loading from kiln-core into GUI state

**Verification:** Open a real binary; hex view scrolls smoothly through full file; sections are listed.

---

### Sprint 4 — Disassembly Listing View
**Goal:** Interactive disassembly listing as the primary view.

- [ ] Disassembly panel: virtual-scrolling table (Address | Bytes | Mnemonic | Operands)
- [ ] Syntax coloring: mnemonics, registers, immediates, addresses (egui RichText)
- [ ] Symbol labels shown inline (e.g., `main:`, `printf@plt:`)
- [ ] Tab system to switch between Hex view and Disassembly view
- [ ] Function/symbol list sidebar (click → navigate to address)
- [ ] Go-to-address dialog (Ctrl+G)

**Verification:** Open `/bin/ls` or similar; disassembly renders correctly; symbol navigation works.

---

### Sprint 5 — Control Flow Analysis
**Goal:** Detect functions and basic blocks; build cross-references.

- [ ] Recursive descent analysis starting from entry point + known symbols
- [ ] Basic block detection (split on branches/calls/returns)
- [ ] Function boundary detection (call targets → function starts)
- [ ] Cross-reference database: code xrefs (call/jump) and data xrefs
- [ ] Display xrefs in GUI: clickable xref list at each address
- [ ] Functions list in sidebar with detected functions (not just symbols)

**Verification:** Analysis detects functions matching symbol table; xrefs are bidirectional and correct.

---

### Sprint 6 — Navigation & Search
**Goal:** IDA-like navigation UX.

- [ ] Click on address operand → navigate to target
- [ ] Navigation history stack (back/forward, Alt+Left/Right)
- [ ] Search: text search in mnemonics/operands
- [ ] Search: hex byte pattern search
- [ ] Keyboard shortcuts: G (goto), N (rename), ; (comment), Esc (back)
- [ ] Context menu (right-click) on addresses

**Verification:** Navigate through a binary fluidly using mouse and keyboard; search finds known strings/patterns.

---

### Sprint 7 — Annotations & Project Persistence
**Goal:** User can annotate and save/restore analysis.

- [ ] Comment system: add/edit/delete comments at any address
- [ ] Rename: rename functions and labels (user-defined names override symbols)
- [ ] Project file format: serialize analysis DB + annotations via serde/bincode
- [ ] File → Save Project / Open Project
- [ ] Recent projects list (persisted in app config)
- [ ] Undo/redo for annotation changes

**Verification:** Add comments/renames; save project; reopen → all annotations preserved; undo works.

---

### Sprint 8 — Basic CFG Graph View
**Goal:** Visual control flow graph for functions.

- [ ] Basic block graph layout algorithm (layered/Sugiyama or simple topological)
- [ ] Render CFG in egui canvas (boxes with instructions, edges for flow)
- [ ] Toggle between linear listing and graph view per function
- [ ] Pan and zoom on graph canvas
- [ ] Color-coded edges: green (true branch), red (false branch), blue (unconditional)

**Verification:** View CFG of a simple function; layout is readable; edges match actual control flow.

---

### Sprint 9 — Strings, Imports/Exports, Multi-Arch Polish
**Goal:** Essential analysis views and cross-architecture robustness.

- [ ] Strings view: extract and list printable strings with xrefs to code
- [ ] Imports table view (with library grouping)
- [ ] Exports table view
- [ ] Test and fix ARM/AArch64 binary loading + disassembly end-to-end
- [ ] Test and fix RISC-V binary loading + disassembly end-to-end
- [ ] Segment/section permissions display (R/W/X)

**Verification:** All views populate correctly for ELF-ARM, ELF-RISC-V, PE-x64, Mach-O-x64 binaries.

---

### Sprint 10 — Polish, CI & First Release
**Goal:** Release-ready MVP.

- [ ] Error handling: graceful handling of corrupt/unsupported binaries
- [ ] Keyboard shortcut cheat sheet / help dialog
- [ ] Dark/light theme toggle
- [ ] GitHub Actions CI: build (Linux, Windows, macOS), test, clippy, fmt
- [ ] README: features, screenshots, build instructions, architecture overview
- [ ] CONTRIBUTING.md and issue templates
- [ ] Release binaries via GitHub Releases (cross-compile or per-platform)
- [ ] `cargo clippy` clean, no unsafe code without justification

**Verification:** CI green on all platforms; download release binary → open binary → full workflow works.

---

## Relevant Files (to be created)
- `Cargo.toml` — workspace root with members
- `crates/kiln-core/src/lib.rs` — binary loading, disassembly, analysis APIs
- `crates/kiln-core/src/loader.rs` — goblin-based binary parser
- `crates/kiln-core/src/disasm.rs` — capstone-based disassembly trait + impls
- `crates/kiln-core/src/analysis.rs` — CFG, function detection, xrefs
- `crates/kiln-core/src/model.rs` — core data types (Instruction, Function, BasicBlock, etc.)
- `crates/kiln-gui/src/main.rs` — eframe app entry point
- `crates/kiln-gui/src/app.rs` — main app state and frame rendering
- `crates/kiln-gui/src/views/` — hex_view, disasm_view, graph_view, strings_view, etc.
- `crates/kiln-project/src/lib.rs` — project save/load, annotation storage

## Decisions
- **GUI framework:** `egui` via `eframe` — most popular, best suited for developer tool UIs, immediate-mode, cross-platform, fast iteration
- **Disassembly backend:** `capstone-rs` — single library covers all 3 arch families (x86, ARM, RISC-V); can swap to pure-Rust backends later
- **Binary parsing:** `goblin` — mature, pure Rust, handles ELF/PE/Mach-O in one crate
- **MVP scope:** Linear disassembly + recursive descent analysis + basic CFG + annotations. No decompiler, no scripting, no debugging — those are post-MVP.
- **Sprint cadence:** 1-week sprints, prompted individually
- **Security:** No `unsafe` without justification; all file parsing through battle-tested crates; input validation on all binary data; no arbitrary code execution

## Excluded from MVP (post-MVP backlog)
- Decompiler / pseudo-code view
- Scripting / plugin system
- Debugger integration
- Type system / struct editor
- DWARF/PDB debug info parsing
- Diffing / binary comparison
- Collaborative analysis
- WASM architecture support
