# Kiln Development Changelog

## Sprint Status Overview

| Sprint | Description | Status |
|--------|------------|--------|
| Sprint 1 | Project Scaffolding & Binary Loading | ✅ Complete |
| Sprint 2 | Linear Disassembly Engine | ✅ Complete |
| Sprint 3 | GUI Shell & Hex View | ✅ Complete |
| Sprint 4 | Disassembly Listing View | ✅ Complete |
| Sprint 5 | Control Flow Analysis | ✅ Complete |
| Sprint 6 | Navigation & Search | ✅ Complete |
| Sprint 7 | Annotations & Project Persistence | ✅ Complete |
| Sprint 8 | Basic CFG Graph View | ✅ Complete |
| Sprint 9 | Strings, Imports/Exports, Multi-Arch Polish | ✅ Complete |
| Sprint 10 | Polish, CI & First Release | ✅ Complete |

---

## Sprint 3 — GUI Shell & Hex View (Completed)

### What was done
- **eframe application**: Created full `KilnApp` struct in `crates/kiln-gui/src/app.rs` with `eframe::App` implementation. Window launches at 1280x800 with a title, menu bar, tab bar, sidebar, status bar, and central panel.
- **File → Open dialog**: Uses `rfd::FileDialog` to open any binary. Wired to `Ctrl+O` shortcut. Calls `kiln_core::load_binary()` and populates all GUI state.
- **Hex view panel** (`crates/kiln-gui/src/views/hex_view.rs`): Virtual-scrolling hex dump using `egui::ScrollArea::show_rows()`. Displays `Offset | Hex Bytes | ASCII` with 16 bytes per row. Color-coded columns (blue addresses, gray hex, yellow ASCII).
- **Section selector sidebar**: Left panel lists all sections from the binary with name, address, size, and `[X]` marker for executable sections. Clicking a section jumps the hex view to that section's file offset.
- **Status bar**: Bottom panel shows `filename | format | architecture | size` when a binary is loaded.
- **Binary loading wired**: `KilnApp::open_binary()` loads the binary, runs disassembly via `disassemble_executable_sections()`, indexes instructions into `AnalysisDatabase`, and initializes both hex and disasm views.

### Files created/modified
- `crates/kiln-gui/src/main.rs` — Replaced stub with eframe app launch
- `crates/kiln-gui/src/app.rs` — Main app state and rendering (NEW)
- `crates/kiln-gui/src/views/mod.rs` — View module declarations (NEW)
- `crates/kiln-gui/src/views/hex_view.rs` — Hex view panel (NEW)

---

## Sprint 4 — Disassembly Listing View (Completed)

### What was done
- **Disassembly panel** (`crates/kiln-gui/src/views/disasm_view.rs`): Virtual-scrolling table using `egui::ScrollArea::show_rows()` displaying `Address | Bytes | Mnemonic | Operands` for each instruction. Uses `AnalysisDatabase.instructions` (BTreeMap) for ordered address access.
- **Syntax coloring**: Full `RichText`-based syntax highlighting:
  - Addresses: cornflower blue
  - Bytes: gray
  - Mnemonics: color-coded by instruction type (red=branches, orange=calls, green=mov/load/store, dark gray=nop, light gray=default)
  - Registers: purple (supports x86, ARM, AArch64, RISC-V register sets)
  - Immediates/hex values: light green
  - Symbol labels: yellow
- **Symbol labels inline**: When an instruction address matches a symbol (via `AnalysisDatabase::symbol_at_address()`), a yellow label like `main:` is rendered above the instruction.
- **Tab system**: Top bar with "Hex View" and "Disassembly" selectable labels. Sidebar context changes per tab (sections for hex, functions for disasm).
- **Function/symbol list sidebar**: In disassembly mode, the left panel lists all function symbols (from `BinaryImage::function_symbols()`) sorted by address. Clicking navigates to that address.
- **Go-to-address dialog**: `Ctrl+G` opens a centered dialog window. Accepts hex addresses with or without `0x` prefix. Navigates the current active view to the parsed address. Shows error for invalid input.

### Files created/modified
- `crates/kiln-gui/src/views/disasm_view.rs` — Disassembly view with syntax coloring (NEW)
- `crates/kiln-gui/src/app.rs` — Tab system, function sidebar, goto dialog, keyboard shortcuts

---

## Sprint 5 — Control Flow Analysis (Completed)

### What was done
- **Recursive descent analysis** (`crates/kiln-core/src/analysis.rs`): Full control flow analysis starting from the binary's entry point and all function symbols. BFS-based work queue discovers all reachable code.
- **Basic block detection**: Instruction sequences are split at branches, calls, and returns. Block leaders are identified via jump targets and fall-through from branches/calls.
- **Function boundary detection**: Call targets are automatically discovered as new function entry points. Functions are named from symbols when available, otherwise auto-named as `sub_XXXX`.
- **Cross-reference database**: Efficient BTreeMap-backed xref lookup for both "xrefs to" and "xrefs from" any address. Supports Call, Jump, and Data xref types.
- **Xref display in GUI**: Disassembly view shows clickable xref annotations above instructions (e.g., `; xrefs: call:0x401000, jmp:0x401020`). Clicking navigates to the xref source.
- **Detected functions sidebar**: Function sidebar now shows all analysis-detected functions (not just symbol table entries), with count displayed in heading.
- **Status bar**: Shows function count and xref count alongside file info.
- **15 new tests**: Covers address parsing, mnemonic classification, basic block splitting, function naming, call-based function discovery, and predecessor tracking.

### Files created/modified
- `crates/kiln-core/src/analysis.rs` — Full rewrite: recursive descent, basic blocks, functions, xrefs (was stub)
- `crates/kiln-gui/src/app.rs` — Wired `run_analysis()` into binary loading, updated function sidebar
- `crates/kiln-gui/src/views/disasm_view.rs` — Xref display, symbol lookup from analysis

---

## Sprint 6 — Navigation & Search (Completed)

### What was done
- **Clickable address operands**: In the disassembly view, operands containing hex addresses (e.g., jump/call targets) are rendered in cyan and are clickable to navigate to the target address.
- **Navigation history stack**: Full back/forward navigation history with `Alt+Left` (back) and `Alt+Right` (forward). `Escape` key also navigates back (or closes open dialogs).
- **Text search**: `Ctrl+F` opens a search dialog. Text mode searches across all instruction mnemonics and operands (case-insensitive). Results are clickable and navigate to the matching instruction.
- **Hex byte pattern search**: Search dialog supports hex byte pattern matching (e.g., `90 C3` or `90C3`). Searches the raw binary data and maps file offsets to virtual addresses.
- **Keyboard shortcuts**: `Ctrl+G` (go to address), `Ctrl+O` (open file), `Ctrl+F` (search), `Alt+Left/Right` (back/forward), `Escape` (back/close dialog).
- **Right-click context menu**: Right-clicking an instruction row shows a context menu with: Copy Address, Copy Instruction, Copy Bytes, Go to target address, and xref navigation (both "xrefs from" and "xrefs to").
- **Navigate menu**: Menu bar now has Back/Forward buttons (with keyboard shortcut hints) and a Search entry.

### Files created/modified
- `crates/kiln-gui/src/app.rs` — NavigationHistory, SearchDialog, SearchMode, search logic, keyboard shortcuts, menu updates
- `crates/kiln-gui/src/views/disasm_view.rs` — Clickable operands, context menu, pending_navigation mechanism

---

## Performance Optimizations (Completed)

### What was done
- **Cached address list**: DisasmView now caches the sorted address list (`Vec<u64>`) from the BTreeMap, rebuilt only when `invalidate_cache()` is called (on binary load). Previously, addresses were collected from BTreeMap keys on every single frame.
- **Cached symbol lookup**: Symbol names are stored in a `HashMap<u64, String>` for O(1) per-address lookup, replacing the previous O(n) linear scan through `BinaryImage.symbols` on every instruction row.
- **Detected function names in cache**: Function names from the analysis database are merged into the symbol cache, so both symbol-table names and auto-detected function names are available without separate lookups.
- **Reduced per-frame allocations**: Caches are owned by `DisasmView` and reused across frames.

### Files created/modified
- `crates/kiln-gui/src/views/disasm_view.rs` — Caching infrastructure, invalidate_cache(), ensure_cache()

---

## Architecture Notes for Future Agents

### GUI Module Structure
```
crates/kiln-gui/src/
├── main.rs          # eframe entry point, NativeOptions config
├── app.rs           # KilnApp struct (implements eframe::App), all app state
└── views/
    ├── mod.rs       # Module declarations
    ├── hex_view.rs  # HexView struct with render() method
    └── disasm_view.rs  # DisasmView struct with render() method
```

### Key Patterns
- **State ownership**: `KilnApp` owns all state including `Option<BinaryImage>`, `AnalysisDatabase`, view states, sidebar state, navigation history, and search dialog state.
- **View rendering**: Each view (`HexView`, `DisasmView`) has a `render(&mut self, ui, ...)` method called from `KilnApp::update()`.
- **Virtual scrolling**: Both views use `egui::ScrollArea::show_rows()` for efficient rendering of large data sets.
- **Navigation**: `KilnApp::navigate_to_address()` records history then dispatches to the active view. Hex view converts virtual address → file offset via section mapping. Disasm view sets `scroll_to_address`.
- **Navigation history**: `NavigationHistory` struct with back/forward stack. `Alt+Left/Right` and `Escape` for navigation.
- **Pending navigation**: `DisasmView` sets `pending_navigation` when the user clicks an address operand or xref. `KilnApp::update()` checks and processes this each frame.
- **Sidebar context**: Sidebar content changes based on `ActiveTab` — sections for hex, detected functions for disasm.
- **Performance caching**: `DisasmView` caches address list and symbol lookup HashMap, invalidated only on new binary load.
- **Control flow analysis**: `AnalysisDatabase::run_analysis()` is called after disassembly to detect functions, basic blocks, and cross-references.

### Build & Test Commands
```bash
cargo build              # Build all crates
cargo test               # Run all tests (37 passing)
cargo clippy --all-targets  # Lint check (clean)
cargo fmt --check        # Format check (clean)
```

---

## Sprint 7 — Annotations & Project Persistence (Completed)

### What was done
- **Comment system**: Add/edit/delete comments at any address. `;` shortcut opens comment dialog. Comments displayed in green in the disassembly view.
- **Rename system**: `N` shortcut opens rename dialog. User-defined labels override symbol table names. Project labels take priority in the symbol cache.
- **Project integration**: `KilnApp` owns a `kiln_project::Project` instance. New project created on binary load. Annotations stored per-address.
- **File → Save/Open Project**: `Ctrl+S` to save, Save As for new path, Open Project loads `.kproj` files and re-opens the referenced binary.
- **Undo/redo**: Full undo/redo stack for comment and label changes. `Ctrl+Z` undo, `Ctrl+Y`/`Ctrl+Shift+Z` redo. Edit menu with Undo/Redo items.
- **Context menu**: "Add Comment" and "Rename" items added to right-click context menu in disassembly view.
- **Status bar**: Shows project path and annotation count.

### Files created/modified
- `crates/kiln-project/src/lib.rs` — Added `remove_comment()`, `remove_label()` methods
- `crates/kiln-gui/src/app.rs` — CommentDialog, RenameDialog, UndoAction, UndoStack, project integration, Save/Open Project, Edit menu
- `crates/kiln-gui/src/views/disasm_view.rs` — Comment display, pending comment/rename actions, project-aware symbol cache

---

## Sprint 8 — Basic CFG Graph View (Completed)

### What was done
- **Graph view** (`crates/kiln-gui/src/views/graph_view.rs`): Full control flow graph visualization for functions.
- **BFS-layered layout**: Assigns layers via BFS from entry block, centers nodes per layer. Nodes sized based on instruction count.
- **Node rendering**: Rounded rectangles with address header, separator line, and monospace instruction listing.
- **Edge rendering**: Lines between block bottoms and tops with arrowheads. Color-coded:
  - Green: true branch (conditional taken)
  - Red: false branch (fallthrough from conditional)
  - Blue: unconditional jump / fallthrough
- **Pan and zoom**: Mouse drag to pan, scroll wheel to zoom.
- **Function selection**: Function sidebar works in Graph tab; clicking a function shows its CFG.
- **Tab integration**: "Graph" tab added to tab bar. Auto-selects first function when switching.

### Files created/modified
- `crates/kiln-gui/src/views/graph_view.rs` — New: 508 lines, full CFG graph view
- `crates/kiln-gui/src/views/mod.rs` — Added `graph_view` module
- `crates/kiln-gui/src/app.rs` — Graph tab, GraphView field, function sidebar for graph

---

## Sprint 9 — Strings, Imports/Exports, Multi-Arch Polish (Completed)

### What was done
- **Strings view** (`crates/kiln-gui/src/views/strings_view.rs`): Extracts printable ASCII strings (≥4 chars) from binary data. Virtual-scrolling table with Address | Offset | Length | Section | String columns. Filter textbox, configurable minimum length, click-to-navigate.
- **Imports view** (`crates/kiln-gui/src/views/imports_view.rs`): Displays imported symbols with Address | Name columns. Filter textbox, click-to-navigate.
- **Exports view** (`crates/kiln-gui/src/views/exports_view.rs`): Displays exported symbols with Address | Size | Name columns. Filter textbox, click-to-navigate.
- **Section permissions**: Section sidebar now shows `[R-X]`, `[RW-]`, etc. permission flags.
- **Tab integration**: "Strings", "Imports", "Exports" tabs added to tab bar.
- **3 new unit tests**: String extraction tests for basic, minimum length, and empty data.

### Files created/modified
- `crates/kiln-gui/src/views/strings_view.rs` — New: strings extraction and display
- `crates/kiln-gui/src/views/imports_view.rs` — New: imports table
- `crates/kiln-gui/src/views/exports_view.rs` — New: exports table
- `crates/kiln-gui/src/views/mod.rs` — Added new view modules
- `crates/kiln-gui/src/app.rs` — New tabs, view fields, sidebar routing, section permissions

---

## Sprint 10 — Polish, CI & First Release (Completed)

### What was done
- **Error handling**: User-friendly error dialog for corrupt/unsupported binaries and project files. Dismissible error window.
- **Help dialog**: F1 shortcut opens keyboard shortcut cheat sheet. 12 shortcuts in a table layout. "About Kiln" dialog with version, license, and project info.
- **Dark/light theme**: Toggle in View menu. Applies `egui::Visuals::dark()` or `egui::Visuals::light()` each frame.
- **README update**: Full feature list covering all 10 sprints, architecture overview, build instructions, usage docs.
- **CONTRIBUTING.md**: Created with build, test, code style, PR submission, and architecture documentation.
- **Code quality**: All `cargo clippy` and `cargo fmt` checks pass clean.

### Files created/modified
- `crates/kiln-gui/src/app.rs` — Error dialog, help dialog, theme toggle, F1 shortcut
- `README.md` — Comprehensive update
- `CONTRIBUTING.md` — New file

---

## MVP Complete — Architecture Summary

### GUI Module Structure (Final)
```
crates/kiln-gui/src/
├── main.rs              # eframe entry point
├── app.rs               # KilnApp struct, all app state & rendering
└── views/
    ├── mod.rs           # Module declarations
    ├── hex_view.rs      # Hex dump view
    ├── disasm_view.rs   # Disassembly listing view
    ├── graph_view.rs    # Control flow graph view
    ├── strings_view.rs  # Extracted strings view
    ├── imports_view.rs  # Imports table view
    └── exports_view.rs  # Exports table view
```

### All Tabs
1. **Hex View** — Raw hex dump with address/bytes/ASCII
2. **Disassembly** — Instruction listing with syntax coloring, xrefs, comments, labels
3. **Graph** — Control flow graph for selected function
4. **Strings** — Extracted printable strings
5. **Imports** — Imported symbols
6. **Exports** — Exported symbols

### All Keyboard Shortcuts
| Shortcut | Action |
|----------|--------|
| Ctrl+O | Open binary |
| Ctrl+S | Save project |
| Ctrl+G | Go to address |
| Ctrl+F | Find/Search |
| Ctrl+Z | Undo |
| Ctrl+Y | Redo |
| Alt+← | Navigate back |
| Alt+→ | Navigate forward |
| ; | Add comment |
| N | Rename symbol |
| Escape | Back / close dialog |
| F1 | Help / shortcuts |

### Codebase Stats
- **16 source files** across 3 crates
- **~5,400 lines** of Rust code
- **37 passing tests**
- **Zero clippy warnings**

### Next Steps (Post-MVP)
See PLAN.md "Excluded from MVP" section for the post-MVP backlog including decompiler, scripting, debugger integration, type system, DWARF/PDB parsing, diffing, and collaborative analysis.
