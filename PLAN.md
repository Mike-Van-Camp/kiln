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

### Sprint 7 — Annotations & Project Persistence ✅
**Goal:** User can annotate and save/restore analysis.

- [x] Comment system: add/edit/delete comments at any address
- [x] Rename: rename functions and labels (user-defined names override symbols)
- [x] Project file format: serialize analysis DB + annotations via serde/bincode
- [x] File → Save Project / Open Project
- [x] Undo/redo for annotation changes

**Verification:** Add comments/renames; save project; reopen → all annotations preserved; undo works.

---

### Sprint 8 — Basic CFG Graph View ✅
**Goal:** Visual control flow graph for functions.

- [x] Basic block graph layout algorithm (BFS-layered)
- [x] Render CFG in egui canvas (boxes with instructions, edges for flow)
- [x] Toggle between linear listing and graph view per function
- [x] Pan and zoom on graph canvas
- [x] Color-coded edges: green (true branch), red (false branch), blue (unconditional)

**Verification:** View CFG of a simple function; layout is readable; edges match actual control flow.

---

### Sprint 9 — Strings, Imports/Exports, Multi-Arch Polish ✅
**Goal:** Essential analysis views and cross-architecture robustness.

- [x] Strings view: extract and list printable strings with xrefs to code
- [x] Imports table view with filter
- [x] Exports table view with filter
- [x] Segment/section permissions display (R/W/X)

**Verification:** All views populate correctly for loaded binaries.

---

### Sprint 10 — Polish, CI & First Release ✅
**Goal:** Release-ready MVP.

- [x] Error handling: graceful handling of corrupt/unsupported binaries
- [x] Keyboard shortcut cheat sheet / help dialog
- [x] Dark/light theme toggle
- [x] README: features, build instructions, architecture overview
- [x] CONTRIBUTING.md
- [x] `cargo clippy` clean

**Verification:** All checks pass; download and use workflow works.

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

---

# Post-MVP SCRUM — Feature Enhancement Sprints

## TL;DR
With the MVP complete (Sprints 1-10), the next phase focuses on advanced analysis features, usability improvements, and platform polish. These sprints are independent and can be prioritized based on user demand.

---

### Sprint 11 — Type System & Data Structures
**Goal:** Allow users to define and apply data types for richer analysis.

- [ ] Define primitive types (u8, u16, u32, u64, i8, i16, i32, i64, f32, f64, char, bool)
- [ ] Struct editor: create named structs with ordered fields
- [ ] Apply types to addresses in the hex view and disassembly view
- [ ] Array type support (e.g., `u8[256]`)
- [ ] Enum type definitions
- [ ] Serialize user-defined types in the project file
- [ ] Display typed data inline in hex view (struct overlay)

**Verification:** Define a struct, apply it to an address, see formatted fields in hex view; save/reload preserves types.

---

### Sprint 12 — DWARF & PDB Debug Info
**Goal:** Parse debug information for richer symbol and type data.

- [ ] DWARF parsing for ELF binaries (function signatures, variable names, types)
- [ ] PDB parsing for PE binaries (Microsoft debug format)
- [ ] Source file / line number mapping
- [ ] Display source-level function signatures in function sidebar
- [ ] Show local variable names in disassembly operands where available
- [ ] Debug info availability indicator in status bar

**Verification:** Load a debug-compiled binary; function signatures, variable names, and source mappings are displayed.

---

### Sprint 13 — Plugin / Scripting System
**Goal:** Allow user scripts to automate analysis tasks.

- [ ] Lua or Rhai scripting engine integration
- [ ] Script API: read/write annotations, iterate instructions, query xrefs
- [ ] Script console panel (REPL)
- [ ] Script file loading (File → Run Script)
- [ ] Built-in example scripts (e.g., find crypto constants, detect obfuscation)
- [ ] Plugin directory auto-loading

**Verification:** Write a script that finds all `xor reg, reg` patterns and adds comments; execute from console.

---

### Sprint 14 — Binary Diffing
**Goal:** Compare two binaries to identify changes.

- [ ] Load two binaries side-by-side
- [ ] Function-level diffing (matched, added, removed, modified)
- [ ] Instruction-level diff within matched functions
- [ ] Color-coded diff view (green=added, red=removed, yellow=modified)
- [ ] Export diff report

**Verification:** Load two versions of a binary; diff shows changed functions and instruction-level changes.

---

### Sprint 15 — Collaborative Analysis
**Goal:** Enable team-based reverse engineering.

- [ ] Export/import annotations as JSON
- [ ] Merge annotations from multiple project files
- [ ] Conflict resolution UI for overlapping annotations
- [ ] Annotation author tracking (username per annotation)
- [ ] Project versioning (annotation history)

**Verification:** Two users annotate the same binary independently; merge their projects; conflicts are resolved.

---

### Sprint 16 — GitHub Actions CI/CD & Cross-Platform Releases
**Goal:** Automated builds and releases for all platforms.

- [ ] GitHub Actions CI: build + test on Linux, Windows, macOS
- [ ] Clippy and fmt checks in CI
- [ ] Cross-compilation matrix (x86_64-linux, x86_64-windows, x86_64-macos, aarch64-macos)
- [ ] Release workflow: tag-triggered builds with artifact upload
- [ ] Installer packaging (AppImage for Linux, .msi for Windows, .dmg for macOS)
- [ ] Release notes auto-generation from CHANGELOG

**Verification:** Push a tag → CI builds all platforms → release artifacts downloadable.

---

### Sprint 17 — Decompiler / Pseudo-Code View
**Goal:** Show C-like pseudo-code for analyzed functions.

- [ ] SSA (Static Single Assignment) intermediate representation
- [ ] Pattern matching for common C constructs (if/else, while, for, switch)
- [ ] Pseudo-code generation from SSA
- [ ] Pseudo-code view panel with syntax highlighting
- [ ] Synchronized navigation between disassembly and pseudo-code
- [ ] Copy pseudo-code to clipboard

**Verification:** View pseudo-code for a compiled C function; output is readable and matches source structure.

---

### Sprint 18 — Debugger Integration
**Goal:** Attach to running processes and debug with disassembly context.

- [ ] GDB/LLDB remote protocol client
- [ ] Attach to process / launch with debugger
- [ ] Breakpoint management (set/clear/enable/disable)
- [ ] Step over / step into / continue / run to cursor
- [ ] Register view panel
- [ ] Memory watch panel
- [ ] Call stack view

**Verification:** Launch a binary under debugger; set breakpoint; step through code; inspect registers and memory.

---

### Sprint 19 — Advanced Graph View
**Goal:** Enhanced CFG visualization and analysis.

- [ ] Sugiyama algorithm for proper layered graph layout
- [ ] Edge routing with splines (avoid node overlaps)
- [ ] Minimap overview panel
- [ ] Dominance tree view
- [ ] Call graph view (function-level graph)
- [ ] Graph export (SVG, PNG)

**Verification:** Complex functions render without overlapping edges; call graph shows full program structure.

---

### Sprint 20 — Performance & Large Binary Support
**Goal:** Handle very large binaries (100MB+) efficiently.

- [ ] Lazy disassembly (disassemble on demand, not all upfront)
- [ ] Background analysis thread (non-blocking UI)
- [ ] Memory-mapped file access instead of loading entire file
- [ ] Progress bar for long-running operations
- [ ] Instruction cache with LRU eviction
- [ ] Benchmark suite for performance regression testing

**Verification:** Open a 200MB binary; UI remains responsive; analysis runs in background with progress indicator.

---

# Phase 3 — Interactivity & UX Deep Dive

## TL;DR
With all foundational features in place (Sprints 1-20), the next phase focuses on making every view truly interactive and polished. The goal is to close the interactivity gap with IDA Pro — everything should be clickable, navigable, and responsive. No more display-only text.

---

### Sprint 21 — Interactive Deep Dive: Selection, Navigation & Context Menus
**Goal:** Make every view element interactive — clickable, selectable, and context-menu-aware.

- [ ] Hex view: byte-level selection with highlighted cursor (click individual bytes)
- [ ] Graph view: click on a node to navigate to that block's address in disassembly
- [ ] Graph view: hover highlighting on nodes (subtle outline change)
- [ ] Graph view: right-click context menu on nodes (Copy address, Go to disassembly, Decompile function)
- [ ] Diff view: selectable instruction rows with context menus (Copy old/new instruction, Copy address)
- [ ] Sidebar: function filter/search textbox to quickly find functions by name
- [ ] Strings view: sortable columns (click header to sort by address, length, or content)
- [ ] Imports/Exports view: sortable columns (click header to sort by address or name)

**Verification:** Every clickable element responds visually; context menus offer relevant actions; function filter narrows the list in real-time.

---

### Sprint 22 — Performance Polish & UX Refinements
**Goal:** Polish every rough edge, improve performance feedback, and clean up the codebase.

- [ ] Console: command history navigation via up/down arrow keys
- [ ] Tab bar: item count badges (e.g., "Strings (142)", "Imports (38)")
- [ ] Hex view: track disassembly selection (highlight corresponding bytes when an instruction is selected)
- [ ] Better empty state messages across all views (actionable hints instead of bare text)
- [ ] Status bar: make function/xref/annotation counts clickable to navigate
- [ ] Remove all unnecessary Sprint marker comments from the entire codebase
- [ ] Clean up redundant/obvious doc comments across all source files

**Verification:** Up/down arrows cycle console history; tab badges update dynamically; empty states guide users; codebase has zero Sprint marker comments.

---

### Sprint 23 — Advanced Hex Interaction & Data Inspector
**Goal:** Bring hex view to parity with dedicated hex editors.

- [ ] Byte-range selection with shift-click and drag
- [ ] Data inspector panel: show selected bytes as u8/u16/u32/u64/i8/i16/i32/i64/f32/f64 (both endiannesses)
- [ ] Hex view: highlight cross-references from disassembly (color data xref targets)
- [ ] Hex view: inline editing of byte values
- [ ] Copy selection as hex, C array, Python bytes, or raw
- [ ] Hex view: highlight search results inline

**Verification:** Select a range of bytes; data inspector shows all interpretations; edit a byte and see the change reflected.

---

### Sprint 24 — Bookmarks, Annotations & Workflow Polish
**Goal:** Let users organize and track their analysis workflow.

- [ ] Bookmark system: add/remove/list bookmarks at any address with optional notes
- [ ] Bookmark sidebar panel (visible across all tabs)
- [ ] Quick-jump to next/previous bookmark (Ctrl+B / Ctrl+Shift+B)
- [ ] Annotation summary view: table of all comments and labels with filters
- [ ] Recent files list in File menu
- [ ] Session state persistence (remember open tab, scroll position, sidebar state)

**Verification:** Add bookmarks; cycle through them; reopen app and find session restored.
