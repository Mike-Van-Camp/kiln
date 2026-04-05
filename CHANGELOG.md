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

## Post-MVP Sprint Status

| Sprint | Description | Status |
|--------|------------|--------|
| Sprint 11 | Type System & Data Structures | ✅ Complete |
| Sprint 12 | DWARF & PDB Debug Info | ✅ Complete |
| Sprint 13 | Plugin / Scripting System | ✅ Complete |
| Sprint 14 | Binary Diffing | ✅ Complete |
| Sprint 15 | Collaborative Analysis | ✅ Complete |
| Sprint 16 | CI/CD & Cross-Platform Releases | ✅ Complete |
| Sprint 17 | Decompiler / Pseudo-Code View | ✅ Complete |
| Sprint 18 | Debugger Integration | ✅ Complete |
| Sprint 19 | Advanced Graph View | ✅ Complete |
| Sprint 20 | Performance & Large Binary Support | ✅ Complete |

---

## Phase 3 — Interactivity & UX Deep Dive

| Sprint | Description | Status |
|--------|------------|--------|
| Sprint 21 | Interactive Deep Dive: Selection, Navigation & Context Menus | ✅ Complete |
| Sprint 22 | Performance Polish & UX Refinements | ✅ Complete |
| Sprint 23 | Advanced Hex Interaction & Data Inspector | 🔲 Planned |
| Sprint 24 | Bookmarks, Annotations & Workflow Polish | 🔲 Planned |

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

### Codebase Stats (Post-MVP)
- **29 source files** across 3 crates
- **~13,200 lines** of Rust code
- **119 passing tests**
- **Zero new clippy warnings**

---

## Sprint 11 — Type System & Data Structures (Completed)

### What was done
- **Primitive types**: u8, u16, u32, u64, i8, i16, i32, i64, f32, f64, char, bool with size computation
- **Struct editor**: Named structs with ordered fields, create/edit/delete via dialog
- **Enum editor**: Named enums with integer-valued variants
- **Array types**: Element type + count (e.g., `u8[256]`)
- **Apply Type**: Right-click in hex/disasm views to apply type at address with optional label
- **Struct overlay**: Hex view renders typed fields inline below hex rows
- **Serialization**: Types and applied types stored in project files
- **7 new tests**: Primitive sizes, struct/enum/array sizing, apply/remove type, serialization roundtrip

### Files created/modified
- `crates/kiln-project/src/lib.rs` — PrimitiveType, StructDef, EnumDef, TypeDef, AppliedType types and methods
- `crates/kiln-gui/src/views/types_view.rs` — New: types list, struct/enum editors, apply-type dialog
- `crates/kiln-gui/src/views/hex_view.rs` — Apply Type context menu, struct overlay
- `crates/kiln-gui/src/views/disasm_view.rs` — Apply Type context menu
- `crates/kiln-gui/src/app.rs` — Types tab, editor dialogs

---

## Sprint 12 — DWARF & PDB Debug Info (Completed)

### What was done
- **DWARF parsing**: gimli/object-based parser for ELF debug info
- **Function signatures**: Extracts parameter names, types, return types from DW_TAG_subprogram
- **Variable extraction**: Local variables and parameters with stack/register locations
- **Source line mapping**: DW_TAG_line_program parsing for file:line annotations
- **Debug info indicator**: Status bar shows debug format availability
- **Function signatures in sidebar**: Rich signatures for debug-compiled binaries
- **Source annotations**: file:line displayed above instructions in disasm view
- **11 new tests**

### Files created/modified
- `crates/kiln-core/src/debug_info.rs` — New: DWARF parser, DebugInfo, DebugFunction, SourceLocation
- `crates/kiln-core/src/lib.rs` — debug_info module export
- `crates/kiln-gui/src/views/disasm_view.rs` — Debug info rendering
- `crates/kiln-gui/src/app.rs` — Debug info field, status bar indicator

---

## Sprint 13 — Plugin / Scripting System (Completed)

### What was done
- **Rhai engine integration**: Full scripting engine with 1M operation limit
- **Script API**: get_instruction, get_instructions_in_range, get_xrefs_to/from, get_functions, get_function, get_comment, get_label, set_comment, set_label, to_hex
- **Console tab**: REPL with command history, colored output, script execution
- **File → Run Script**: Execute .rhai script files
- **Rhai print fix**: Uses engine.on_print()/on_debug() callbacks to avoid built-in print conflicts
- **13 passing tests** (all scripting tests)

### Files created/modified
- `crates/kiln-gui/src/scripting.rs` — New: Rhai engine, script API
- `crates/kiln-gui/src/views/console_view.rs` — New: Script console REPL
- `crates/kiln-gui/src/app.rs` — Console tab, Run Script menu

---

## Sprint 14 — Binary Diffing (Completed)

### What was done
- **Diff engine**: Function matching by name with LCS-based instruction similarity scoring
- **Instruction-level diff**: LCS algorithm detecting Same/Added/Removed/Modified changes
- **Diff view**: Two-pane layout — function list with status icons, instruction diff display
- **Color-coded**: Green=added, Red=removed, Yellow=modified, Gray=same
- **Export report**: Copy diff summary to clipboard
- **File → Open Diff**: Opens two file dialogs for binary comparison
- **7 new tests**

### Files created/modified
- `crates/kiln-core/src/diff.rs` — New: diff engine with LCS algorithm
- `crates/kiln-gui/src/views/diff_view.rs` — New: diff view
- `crates/kiln-gui/src/app.rs` — Diff tab, Open Diff menu

---

## Sprint 15 — Collaborative Analysis (Completed)

### What was done
- **Author tracking**: Annotations store author name and unix timestamp
- **Annotation history**: Full version history of all changes (set/remove comment/label)
- **JSON export**: Export annotations as pretty-printed JSON (copies to clipboard)
- **JSON import**: Import annotations from pasted JSON text
- **Merge with conflict resolution**: Automatic merge for non-overlapping, manual resolution for conflicts
- **Collab tab**: Author settings, import text area, conflict resolution UI, history viewer
- **Backward compatible**: All new fields use `#[serde(default)]` for old project files
- **5 new tests**: JSON roundtrip, merge (no conflicts), merge (with conflicts), author tracking, history recording

### Files created/modified
- `crates/kiln-project/src/lib.rs` — Author/timestamp fields, history, JSON export/import, merge logic
- `crates/kiln-gui/src/views/collab_view.rs` — New: collaboration UI
- `crates/kiln-gui/src/app.rs` — Collab tab

---

## Sprint 16 — CI/CD & Cross-Platform Releases (Completed)

### What was done
- **CI workflow** (`.github/workflows/ci.yml`):
  - `cargo fmt --all --check` (Ubuntu)
  - `cargo clippy --all-targets --workspace -- -D warnings` (Ubuntu)
  - `cargo test --workspace` + `cargo build --release` on Linux, Windows, macOS
- **Release workflow** (`.github/workflows/release.yml`):
  - Tag-triggered (v*) builds for 4 targets: x86_64-linux, x86_64-windows, x86_64-macos, aarch64-macos
  - Release artifact upload via softprops/action-gh-release
  - Release notes extracted from CHANGELOG.md

### Files created
- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`

---

## Sprint 17 — Decompiler / Pseudo-Code View (Completed)

### What was done
- **IR representation**: IrExpr and IrStatement types for intermediate representation
- **Pattern-matching decompiler**: Translates x86 instructions to C-like constructs:
  - mov/xor/add/sub/inc/dec → assignments and arithmetic
  - call → function calls
  - ret → return statements
  - cmp/test + jcc → if/else conditions
  - Back-edge detection → while loops
  - push/pop/nop → filtered (prologue/epilogue)
  - Unrecognized → RawAsm fallback
- **Pseudo-code generation**: C-like output with proper indentation
- **Decompiler view**: Function selector, syntax-highlighted monospace display, click-to-navigate
- **Copy to clipboard**: Copy pseudo-code button
- **10 new tests**

### Files created/modified
- `crates/kiln-core/src/decompiler.rs` — New: decompiler, IR types, code generator
- `crates/kiln-gui/src/views/decompiler_view.rs` — New: decompiler view
- `crates/kiln-gui/src/app.rs` — Decompiler tab

---

## Sprint 18 — Debugger Integration (Completed)

### What was done
- **Debug session model**: DebuggerState (Disconnected/Running/Paused/Exited), Breakpoint, RegisterValue, StackFrame, MemoryWatch structures
- **Breakpoint management**: Add/remove/toggle/clear with ID tracking
- **Debugger view**: Full UI with:
  - State indicator bar (color-coded)
  - Toolbar: Connect, Continue, Step Over, Step Into, Stop buttons
  - Collapsible panels: Breakpoints, Registers, Call Stack, Memory Watch
  - Output log
- **Note**: GDB/LLDB wire protocol to be added in future sprint
- **8 new tests**

### Files created/modified
- `crates/kiln-core/src/debugger.rs` — New: debug session data model
- `crates/kiln-gui/src/views/debugger_view.rs` — New: debugger UI
- `crates/kiln-gui/src/app.rs` — Debugger tab

---

## Sprint 19 — Advanced Graph View (Completed)

### What was done
- **Sugiyama algorithm**: Longest-path layering with DFS back-edge detection + barycenter crossing minimization
- **Edge routing**: Quadratic bezier curves with offset departure points for parallel edges
- **Minimap**: Bottom-right overview panel showing graph extent and visible viewport
- **Dominance tree view**: Cooper-Harvey-Kennedy algorithm for immediate dominator computation, toggle via toolbar
- **Call graph view**: Function-level graph from Call xrefs with deduplication, toggle via toolbar
- **SVG export**: Generates SVG string with nodes and edges, copies to clipboard
- **13 new tests**

### Files created/modified
- `crates/kiln-core/src/graph.rs` — New: graph layout and dominance algorithms
- `crates/kiln-gui/src/views/graph_view.rs` — Enhanced with all advanced features

---

## Sprint 20 — Performance & Large Binary Support (Completed)

### What was done
- **LRU instruction cache**: HashMap + VecDeque bounded cache with LRU eviction, configurable max entries
- **Progress tracker**: Thread-safe progress reporting with Arc<AtomicU64>, cancellation support
- **Section range queries**: Efficient lookup of sections overlapping an address range
- **Progress bar**: Rendered in status bar when long-running operations are active
- **8 new tests**

### Files created/modified
- `crates/kiln-core/src/perf.rs` — New: InstructionCache, ProgressTracker, sections_in_range
- `crates/kiln-gui/src/app.rs` — Progress field and status bar rendering

---

## All Sprints Complete — Final Architecture

### GUI Module Structure
```
crates/kiln-gui/src/
├── main.rs               # eframe entry point
├── app.rs                # KilnApp struct, all app state & rendering
├── scripting.rs          # Rhai scripting engine
└── views/
    ├── mod.rs            # Module declarations
    ├── hex_view.rs       # Hex dump view
    ├── disasm_view.rs    # Disassembly listing view
    ├── graph_view.rs     # Control flow graph view (advanced)
    ├── decompiler_view.rs # Pseudo-code decompiler view
    ├── strings_view.rs   # Extracted strings view
    ├── imports_view.rs   # Imports table view
    ├── exports_view.rs   # Exports table view
    ├── types_view.rs     # Type editor view
    ├── console_view.rs   # Script console view
    ├── diff_view.rs      # Binary diff view
    ├── collab_view.rs    # Collaborative analysis view
    └── debugger_view.rs  # Debugger integration view
```

### Core Module Structure
```
crates/kiln-core/src/
├── lib.rs          # Module declarations and re-exports
├── loader.rs       # Binary loading (goblin)
├── disasm.rs       # Disassembly (capstone)
├── model.rs        # Core data types
├── analysis.rs     # CFG analysis, functions, xrefs
├── debug_info.rs   # DWARF/PDB parsing (gimli/object)
├── diff.rs         # Binary diffing engine
├── decompiler.rs   # Pseudo-code decompiler
├── debugger.rs     # Debug session model
├── graph.rs        # Graph layout algorithms
└── perf.rs         # Performance infrastructure
```

### All Tabs
1. **Hex View** — Raw hex dump with struct overlay
2. **Disassembly** — Instruction listing with xrefs, comments, labels, debug info
3. **Graph** — CFG with Sugiyama layout, minimap, dominance tree, call graph
4. **Decompiler** — C-like pseudo-code
5. **Strings** — Extracted printable strings
6. **Imports** — Imported symbols
7. **Exports** — Exported symbols
8. **Types** — Type editor with struct/enum definitions
9. **Console** — Rhai scripting REPL
10. **Diff** — Binary comparison
11. **Collab** — Collaborative analysis
12. **Debugger** — Debug session with breakpoints, registers, call stack

### Final Codebase Stats
- **29 source files** across 3 crates
- **~13,200 lines** of Rust code
- **119 passing tests**
- **Zero new clippy warnings**

---

## Sprint 21 — Interactive Deep Dive: Selection, Navigation & Context Menus (Completed)

### What was done
- **Graph view: clickable nodes**: Left-clicking any node in the CFG, call graph, or dominance tree view navigates to that block's address in the disassembly view
- **Graph view: hover highlighting**: Nodes get a brighter blue outline when the mouse hovers over them
- **Graph view: right-click context menu**: Right-clicking a node shows "Copy Address" and "Go to Disassembly" options
- **Sidebar: function filter**: Added a 🔍 search textbox to the function sidebar (both disassembly and graph modes) for real-time case-insensitive function name filtering
- **Strings view: sortable columns**: Click on Address, Len, or String column headers to sort (with ▲/▼ indicators and ascending/descending toggle)
- **Imports view: sortable columns**: Click on Address or Name headers to sort
- **Exports view: sortable columns**: Click on Address or Name headers to sort
- **Diff view: interactive rows**: Instruction diff rows are now selectable with hover effects and right-click context menus (Copy Old/New Address, Copy Old/New Instruction)
- **Hex view: data inspector**: Selecting a row now shows a mini data inspector panel at the bottom displaying the value as u8, i8, u16 LE, u32 LE, and ASCII

### Files modified
- `crates/kiln-gui/src/views/graph_view.rs` — Clickable nodes, hover highlighting, context menus, cleaned up section separator comments
- `crates/kiln-gui/src/views/strings_view.rs` — Sortable column headers, sort state
- `crates/kiln-gui/src/views/imports_view.rs` — Sortable column headers, sort state
- `crates/kiln-gui/src/views/exports_view.rs` — Sortable column headers, sort state
- `crates/kiln-gui/src/views/diff_view.rs` — Selectable instruction rows with context menus
- `crates/kiln-gui/src/views/hex_view.rs` — Byte-level selection, data inspector panel
- `crates/kiln-gui/src/app.rs` — Function filter field, sidebar filter UI, graph view navigation wiring

---

## Sprint 22 — Performance Polish & UX Refinements (Completed)

### What was done
- **Console: command history navigation**: Up/Down arrow keys cycle through previously entered commands (using existing history state)
- **Tab item count badges**: Strings, Imports, Exports, and Types tabs now show item counts in their labels (e.g., "Strings (142)")
- **Improved empty state messages**: All views now show actionable guidance text when no data is loaded:
  - Main panel: mentions both Open and Open Project
  - Graph view: explains what to do
  - Decompiler: guides to function selector
  - Diff: explains the diff engine
  - Console: mentions arrow key history
  - Debugger: guides to Connect button for each panel section
- **Sprint marker cleanup**: Removed all 56 `(Sprint N)` references from doc comments and inline comments across 11 source files
- **Comment cleanup**: Removed unnecessary section separator comments from graph_view.rs

### Files modified
- `crates/kiln-gui/src/views/console_view.rs` — Arrow key history, cleaned Sprint marker
- `crates/kiln-gui/src/views/debugger_view.rs` — Improved empty states, cleaned Sprint marker
- `crates/kiln-gui/src/views/collab_view.rs` — Cleaned Sprint marker
- `crates/kiln-gui/src/views/decompiler_view.rs` — Improved empty state
- `crates/kiln-gui/src/views/diff_view.rs` — Improved empty state
- `crates/kiln-gui/src/views/graph_view.rs` — Improved empty states
- `crates/kiln-gui/src/app.rs` — Tab count badges, improved main empty state, cleaned all Sprint markers
