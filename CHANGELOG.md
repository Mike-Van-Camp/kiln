# Kiln Development Changelog

## Sprint Status Overview

| Sprint | Description | Status |
|--------|------------|--------|
| Sprint 1 | Project Scaffolding & Binary Loading | ✅ Complete |
| Sprint 2 | Linear Disassembly Engine | ✅ Complete |
| Sprint 3 | GUI Shell & Hex View | ✅ Complete |
| Sprint 4 | Disassembly Listing View | ✅ Complete |
| Sprint 5 | Control Flow Analysis | ⏳ Not Started |
| Sprint 6 | Navigation & Search | ⏳ Not Started |
| Sprint 7 | Annotations & Project Persistence | ⏳ Not Started |
| Sprint 8 | Basic CFG Graph View | ⏳ Not Started |
| Sprint 9 | Strings, Imports/Exports, Multi-Arch Polish | ⏳ Not Started |
| Sprint 10 | Polish, CI & First Release | ⏳ Not Started |

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
- **State ownership**: `KilnApp` owns all state including `Option<BinaryImage>`, `AnalysisDatabase`, view states, and sidebar state.
- **View rendering**: Each view (`HexView`, `DisasmView`) has a `render(&mut self, ui, ...)` method called from `KilnApp::update()`.
- **Virtual scrolling**: Both views use `egui::ScrollArea::show_rows()` for efficient rendering of large data sets.
- **Navigation**: `KilnApp::navigate_to_address()` dispatches to the active view. Hex view converts virtual address → file offset via section mapping. Disasm view sets `scroll_to_address`.
- **Sidebar context**: Sidebar content changes based on `ActiveTab` — sections for hex, functions for disasm.

### What's NOT done yet (for Sprint 5+)
- No recursive descent analysis (analysis.rs is still a stub, only indexes linear disassembly)
- No basic block detection or function detection
- No cross-reference database
- No navigation history (back/forward)
- No text/byte search
- No right-click context menus
- No comment/rename annotation UI
- No undo/redo
- No CFG graph view
- No strings view
- No dark/light theme toggle
- No CI/CD pipeline

### Build & Test Commands
```bash
cargo build              # Build all crates
cargo test               # Run all tests (19 passing)
cargo clippy --all-targets  # Lint check (clean)
cargo fmt --check        # Format check (clean)
```

### Next Sprint to Work On: Sprint 5 — Control Flow Analysis
See PLAN.md for full sprint details. Key tasks:
1. Recursive descent analysis starting from entry point + known symbols
2. Basic block detection (split on branches/calls/returns)
3. Function boundary detection (call targets → function starts)
4. Cross-reference database: code xrefs (call/jump) and data xrefs
5. Display xrefs in GUI: clickable xref list at each address
6. Functions list in sidebar with detected functions (not just symbols)
