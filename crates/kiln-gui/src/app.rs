//! Main application state and frame rendering for Kiln GUI.

use eframe::egui;
use kiln_core::analysis::AnalysisDatabase;
use kiln_core::debug_info::DebugInfo;
use kiln_core::disasm::disassemble_executable_sections;
use kiln_core::model::BinaryImage;
use std::path::PathBuf;

use crate::views::collab_view::CollabView;
use crate::views::console_view::ConsoleView;
use crate::views::decompiler_view::DecompilerView;
use crate::views::diff_view::DiffView;
use crate::views::disasm_view::DisasmView;
use crate::views::exports_view::ExportsView;
use crate::views::graph_view::GraphView;
use crate::views::hex_view::HexView;
use crate::views::imports_view::ImportsView;
use crate::views::strings_view::StringsView;
use crate::views::types_view::{
    ApplyTypeDialog, EnumEditorDialog, StructEditorDialog, TypesView,
};

/// Which main view tab is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Hex,
    Disassembly,
    Graph,
    Decompiler,
    Strings,
    Imports,
    Exports,
    Types,
    Console,
    Diff,
    Collab,
}

/// State for the Go-to-Address dialog.
#[derive(Default)]
pub struct GotoDialog {
    pub open: bool,
    pub input: String,
    pub error: Option<String>,
}

/// State for the Search dialog (Sprint 6).
#[derive(Default)]
pub struct SearchDialog {
    pub open: bool,
    pub query: String,
    pub search_mode: SearchMode,
    pub results: Vec<SearchResult>,
    pub error: Option<String>,
}

/// Search mode selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchMode {
    #[default]
    Text,
    HexBytes,
}

/// A single search result.
#[derive(Clone)]
pub struct SearchResult {
    pub address: u64,
    pub context: String,
}

/// Navigation history for back/forward (Sprint 6).
#[derive(Default)]
pub struct NavigationHistory {
    stack: Vec<u64>,
    position: usize,
}

impl NavigationHistory {
    /// Push a new address onto the history, truncating any forward history.
    pub fn push(&mut self, addr: u64) {
        // Don't push duplicate of current position
        if self.position > 0
            && self.position <= self.stack.len()
            && self.stack[self.position - 1] == addr
        {
            return;
        }
        // Truncate forward history
        self.stack.truncate(self.position);
        self.stack.push(addr);
        self.position = self.stack.len();
    }

    /// Go back in history. Returns the address to navigate to.
    pub fn go_back(&mut self) -> Option<u64> {
        if self.position > 1 {
            self.position -= 1;
            Some(self.stack[self.position - 1])
        } else {
            None
        }
    }

    /// Go forward in history. Returns the address to navigate to.
    pub fn go_forward(&mut self) -> Option<u64> {
        if self.position < self.stack.len() {
            self.position += 1;
            Some(self.stack[self.position - 1])
        } else {
            None
        }
    }

    pub fn can_go_back(&self) -> bool {
        self.position > 1
    }

    pub fn can_go_forward(&self) -> bool {
        self.position < self.stack.len()
    }
}

/// State for the comment dialog.
#[derive(Default)]
pub struct CommentDialog {
    pub open: bool,
    pub address: u64,
    pub input: String,
}

/// State for the rename dialog.
#[derive(Default)]
pub struct RenameDialog {
    pub open: bool,
    pub address: u64,
    pub input: String,
}

/// State for the Help / keyboard shortcuts dialog.
#[derive(Default)]
pub struct HelpDialog {
    pub open: bool,
    pub show_about: bool,
}

/// An undoable annotation action.
#[derive(Clone)]
pub enum UndoAction {
    SetComment {
        addr: u64,
        old: Option<String>,
        new: Option<String>,
    },
    SetLabel {
        addr: u64,
        old: Option<String>,
        new: Option<String>,
    },
}

/// Undo/redo stack for annotation changes.
#[derive(Default)]
pub struct UndoStack {
    undo: Vec<UndoAction>,
    redo: Vec<UndoAction>,
}

/// Main application state.
pub struct KilnApp {
    /// The loaded binary image (None if no file is open).
    pub image: Option<BinaryImage>,
    /// Analysis database with indexed instructions.
    pub analysis: AnalysisDatabase,
    /// Currently active main view tab.
    pub active_tab: ActiveTab,
    /// Hex view state.
    pub hex_view: HexView,
    /// Disassembly view state.
    pub disasm_view: DisasmView,
    /// Graph view state.
    pub graph_view: GraphView,
    /// Strings view state.
    pub strings_view: StringsView,
    /// Imports view state.
    pub imports_view: ImportsView,
    /// Exports view state.
    pub exports_view: ExportsView,
    /// Status message displayed in the status bar.
    pub status_message: String,
    /// Currently selected section index in the sidebar.
    pub selected_section: Option<usize>,
    /// Currently selected symbol index in the sidebar.
    pub selected_symbol: Option<usize>,
    /// Go-to-address dialog state.
    pub goto_dialog: GotoDialog,
    /// Search dialog state (Sprint 6).
    pub search_dialog: SearchDialog,
    /// Navigation history (Sprint 6).
    pub nav_history: NavigationHistory,
    /// Whether to show the section sidebar (Sprint 3) or function sidebar (Sprint 4).
    pub show_sidebar: bool,
    /// Project containing annotations and metadata (Sprint 7).
    pub project: kiln_project::Project,
    /// Path where the project was last saved.
    pub project_path: Option<PathBuf>,
    /// Comment dialog state (Sprint 7).
    pub comment_dialog: CommentDialog,
    /// Rename dialog state (Sprint 7).
    pub rename_dialog: RenameDialog,
    /// Undo/redo stack for annotation changes (Sprint 7).
    pub undo_stack: UndoStack,
    /// Error dialog message (Sprint 10).
    pub error_dialog: Option<String>,
    /// Help / keyboard shortcuts dialog (Sprint 10).
    pub help_dialog: HelpDialog,
    /// Whether dark theme is active (Sprint 10).
    pub dark_mode: bool,
    /// Types view state (Sprint 11).
    pub types_view: TypesView,
    /// Struct editor dialog (Sprint 11).
    pub struct_editor_dialog: StructEditorDialog,
    /// Enum editor dialog (Sprint 11).
    pub enum_editor_dialog: EnumEditorDialog,
    /// Apply-type dialog (Sprint 11).
    pub apply_type_dialog: ApplyTypeDialog,
    /// Parsed debug info from DWARF/PDB sections (Sprint 12).
    pub debug_info: DebugInfo,
    /// Script console view (Sprint 13).
    pub console_view: ConsoleView,
    /// Binary diff view (Sprint 14).
    pub diff_view: DiffView,
    /// Collaborative analysis view (Sprint 15).
    pub collab_view: CollabView,
    /// Decompiler pseudo-code view (Sprint 17).
    pub decompiler_view: DecompilerView,
}

impl Default for KilnApp {
    fn default() -> Self {
        Self {
            image: None,
            analysis: AnalysisDatabase::new(),
            active_tab: ActiveTab::Hex,
            hex_view: HexView::default(),
            disasm_view: DisasmView::default(),
            graph_view: GraphView::default(),
            strings_view: StringsView::default(),
            imports_view: ImportsView::default(),
            exports_view: ExportsView::default(),
            status_message: "No file loaded".to_string(),
            selected_section: None,
            selected_symbol: None,
            goto_dialog: GotoDialog::default(),
            search_dialog: SearchDialog::default(),
            nav_history: NavigationHistory::default(),
            show_sidebar: true,
            project: kiln_project::Project::new(String::new()),
            project_path: None,
            comment_dialog: CommentDialog::default(),
            rename_dialog: RenameDialog::default(),
            undo_stack: UndoStack::default(),
            error_dialog: None,
            help_dialog: HelpDialog::default(),
            dark_mode: true,
            types_view: TypesView::default(),
            struct_editor_dialog: StructEditorDialog::default(),
            enum_editor_dialog: EnumEditorDialog::default(),
            apply_type_dialog: ApplyTypeDialog::default(),
            debug_info: DebugInfo::default(),
            console_view: ConsoleView::default(),
            diff_view: DiffView::default(),
            collab_view: CollabView::default(),
            decompiler_view: DecompilerView::default(),
        }
    }
}

impl KilnApp {
    /// Create a new KilnApp instance.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    /// Open a binary file and load it into the application state.
    pub fn open_binary(&mut self, path: &std::path::Path) {
        match kiln_core::load_binary(path) {
            Ok(image) => {
                self.status_message = format!(
                    "{} | {} | {} | {} bytes",
                    image.filename,
                    image.format,
                    image.architecture,
                    image.data.len(),
                );

                // Disassemble executable sections and index them
                match disassemble_executable_sections(&image) {
                    Ok(instructions) => {
                        self.analysis = AnalysisDatabase::new();
                        let first_addr = instructions.first().map(|i| i.address);
                        self.analysis.index_instructions(instructions);

                        // Run control flow analysis (Sprint 5)
                        self.analysis.run_analysis(&image);

                        // Rebuild disasm view cache
                        self.disasm_view.invalidate_cache();

                        // Set initial disassembly view position
                        if let Some(addr) = first_addr {
                            self.disasm_view.scroll_to_address = Some(addr);
                        }
                    }
                    Err(e) => {
                        log::warn!("Disassembly failed: {}", e);
                    }
                }

                // Set initial hex view position to start of first section
                if let Some(section) = image.sections.first() {
                    self.hex_view.offset = section.file_offset as usize;
                }

                // Parse debug info from DWARF/PDB sections (Sprint 12)
                self.debug_info = kiln_core::debug_info::parse_debug_info(&image);

                self.selected_section = None;
                self.selected_symbol = None;
                self.nav_history = NavigationHistory::default();
                self.project = kiln_project::Project::new(path.to_string_lossy().into_owned());
                self.project_path = None;
                self.undo_stack = UndoStack::default();
                self.graph_view = GraphView::default();
                self.strings_view.invalidate_cache();
                self.image = Some(image);
                self.active_tab = ActiveTab::Hex;
            }
            Err(e) => {
                let msg = format!("Failed to load binary: {}", e);
                self.status_message = format!("Error loading binary: {}", e);
                self.error_dialog = Some(msg);
                log::error!("Failed to load binary: {}", e);
            }
        }
    }

    /// Navigate to a specific address in the current view, recording history.
    pub fn navigate_to_address(&mut self, addr: u64) {
        self.nav_history.push(addr);
        self.navigate_to_address_no_history(addr);
    }

    /// Navigate without recording in history (used by back/forward).
    fn navigate_to_address_no_history(&mut self, addr: u64) {
        match self.active_tab {
            ActiveTab::Hex => {
                if let Some(image) = &self.image {
                    for section in &image.sections {
                        if addr >= section.address && addr < section.address + section.size {
                            let offset_in_section = addr - section.address;
                            self.hex_view.offset =
                                section.file_offset as usize + offset_in_section as usize;
                            return;
                        }
                    }
                    self.hex_view.offset = addr as usize;
                }
            }
            ActiveTab::Disassembly => {
                self.disasm_view.scroll_to_address = Some(addr);
            }
            ActiveTab::Graph => {
                // In graph view, navigate to the function containing this address
                for func in self.analysis.functions.values() {
                    if func
                        .blocks
                        .iter()
                        .any(|b| addr >= b.start_addr && addr < b.end_addr)
                    {
                        self.graph_view.select_function(func.entry_addr);
                        return;
                    }
                }
            }
            ActiveTab::Decompiler => {
                // In decompiler view, decompile the function containing this address
                for func in self.analysis.functions.values() {
                    if func
                        .blocks
                        .iter()
                        .any(|b| addr >= b.start_addr && addr < b.end_addr)
                    {
                        self.decompiler_view.select_function(
                            func.entry_addr,
                            &self.analysis,
                            &self.debug_info,
                        );
                        return;
                    }
                }
            }
            ActiveTab::Strings | ActiveTab::Imports | ActiveTab::Exports | ActiveTab::Types => {
                // Switch to disassembly view to show the address
                self.active_tab = ActiveTab::Disassembly;
                self.disasm_view.scroll_to_address = Some(addr);
            }
            ActiveTab::Console => {
                self.active_tab = ActiveTab::Disassembly;
                self.disasm_view.scroll_to_address = Some(addr);
            }
            ActiveTab::Diff => {
                self.active_tab = ActiveTab::Disassembly;
                self.disasm_view.scroll_to_address = Some(addr);
            }
            ActiveTab::Collab => {
                self.active_tab = ActiveTab::Disassembly;
                self.disasm_view.scroll_to_address = Some(addr);
            }
        }
    }

    /// Navigate back in history (Sprint 6).
    fn navigate_back(&mut self) {
        if let Some(addr) = self.nav_history.go_back() {
            self.navigate_to_address_no_history(addr);
        }
    }

    /// Navigate forward in history (Sprint 6).
    fn navigate_forward(&mut self) {
        if let Some(addr) = self.nav_history.go_forward() {
            self.navigate_to_address_no_history(addr);
        }
    }

    /// Render the menu bar.
    fn render_menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open...  Ctrl+O").clicked() {
                        self.open_file_dialog();
                        ui.close_menu();
                    }
                    if ui.button("Open Diff...").clicked() {
                        self.open_diff_dialog();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Save Project  Ctrl+S").clicked() {
                        self.save_project();
                        ui.close_menu();
                    }
                    if ui.button("Save Project As...").clicked() {
                        self.save_project_as();
                        ui.close_menu();
                    }
                    if ui.button("Open Project...").clicked() {
                        self.open_project_dialog();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Run Script...").clicked() {
                        self.run_script_dialog();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Edit", |ui| {
                    let can_undo = !self.undo_stack.undo.is_empty();
                    let can_redo = !self.undo_stack.redo.is_empty();
                    if ui
                        .add_enabled(can_undo, egui::Button::new("Undo  Ctrl+Z"))
                        .clicked()
                    {
                        self.perform_undo();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(can_redo, egui::Button::new("Redo  Ctrl+Y"))
                        .clicked()
                    {
                        self.perform_redo();
                        ui.close_menu();
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui
                        .checkbox(&mut self.show_sidebar, "Show Sidebar")
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    if ui.checkbox(&mut self.dark_mode, "Dark Theme").clicked() {
                        ui.close_menu();
                    }
                });
                ui.menu_button("Navigate", |ui| {
                    if ui.button("Go to Address  Ctrl+G").clicked() {
                        self.goto_dialog.open = true;
                        self.goto_dialog.input.clear();
                        self.goto_dialog.error = None;
                        ui.close_menu();
                    }
                    let back_btn = ui.add_enabled(
                        self.nav_history.can_go_back(),
                        egui::Button::new("Back  Alt+←"),
                    );
                    if back_btn.clicked() {
                        self.navigate_back();
                        ui.close_menu();
                    }
                    let fwd_btn = ui.add_enabled(
                        self.nav_history.can_go_forward(),
                        egui::Button::new("Forward  Alt+→"),
                    );
                    if fwd_btn.clicked() {
                        self.navigate_forward();
                        ui.close_menu();
                    }
                });
                ui.menu_button("Search", |ui| {
                    if ui.button("Find...  Ctrl+F").clicked() {
                        self.search_dialog.open = true;
                        self.search_dialog.error = None;
                        ui.close_menu();
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("Keyboard Shortcuts  F1").clicked() {
                        self.help_dialog.open = true;
                        self.help_dialog.show_about = false;
                        ui.close_menu();
                    }
                    if ui.button("About Kiln").clicked() {
                        self.help_dialog.open = true;
                        self.help_dialog.show_about = true;
                        ui.close_menu();
                    }
                });
            });
        });
    }

    /// Render the status bar at the bottom.
    fn render_status_bar(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_message);
                if let Some(path) = &self.project_path {
                    ui.separator();
                    if let Some(name) = path.file_name() {
                        ui.label(format!("Project: {}", name.to_string_lossy()));
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Debug info indicator (Sprint 12)
                    if self.debug_info.has_debug_info {
                        let fmt = self.debug_info.debug_format.as_deref().unwrap_or("Debug");
                        ui.label(
                            egui::RichText::new(format!("🐛 {}", fmt))
                                .color(egui::Color32::from_rgb(100, 200, 100)),
                        );
                        ui.separator();
                    } else if self.image.is_some() {
                        ui.label(
                            egui::RichText::new("No debug info")
                                .color(egui::Color32::GRAY),
                        );
                        ui.separator();
                    }
                    let func_count = self.analysis.functions.len();
                    let xref_count = self.analysis.xrefs.len();
                    let annotation_count = self.project.annotations.len();
                    if func_count > 0 {
                        ui.label(format!(
                            "{} functions | {} xrefs | {} annotations",
                            func_count, xref_count, annotation_count
                        ));
                    }
                });
            });
        });
    }

    /// Render the tab bar for switching views.
    fn render_tab_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.active_tab == ActiveTab::Hex, "Hex View")
                    .clicked()
                {
                    self.active_tab = ActiveTab::Hex;
                }
                if ui
                    .selectable_label(self.active_tab == ActiveTab::Disassembly, "Disassembly")
                    .clicked()
                {
                    self.active_tab = ActiveTab::Disassembly;
                }
                if ui
                    .selectable_label(self.active_tab == ActiveTab::Graph, "Graph")
                    .clicked()
                {
                    self.active_tab = ActiveTab::Graph;
                    // Auto-select first function if none selected
                    if self.graph_view.selected_function.is_none() {
                        if let Some(&addr) = self.analysis.functions.keys().next() {
                            self.graph_view.select_function(addr);
                        }
                    }
                }
                if ui
                    .selectable_label(self.active_tab == ActiveTab::Decompiler, "Decompiler")
                    .clicked()
                {
                    self.active_tab = ActiveTab::Decompiler;
                    // Auto-select first function if none selected
                    if self.decompiler_view.selected_function_addr.is_none() {
                        if let Some(&addr) = self.analysis.functions.keys().next() {
                            self.decompiler_view.select_function(
                                addr,
                                &self.analysis,
                                &self.debug_info,
                            );
                        }
                    }
                }
                ui.separator();
                if ui
                    .selectable_label(self.active_tab == ActiveTab::Strings, "Strings")
                    .clicked()
                {
                    self.active_tab = ActiveTab::Strings;
                }
                if ui
                    .selectable_label(self.active_tab == ActiveTab::Imports, "Imports")
                    .clicked()
                {
                    self.active_tab = ActiveTab::Imports;
                }
                if ui
                    .selectable_label(self.active_tab == ActiveTab::Exports, "Exports")
                    .clicked()
                {
                    self.active_tab = ActiveTab::Exports;
                }
                if ui
                    .selectable_label(self.active_tab == ActiveTab::Types, "Types")
                    .clicked()
                {
                    self.active_tab = ActiveTab::Types;
                }
                ui.separator();
                if ui
                    .selectable_label(self.active_tab == ActiveTab::Console, "Console")
                    .clicked()
                {
                    self.active_tab = ActiveTab::Console;
                }
                if ui
                    .selectable_label(self.active_tab == ActiveTab::Diff, "Diff")
                    .clicked()
                {
                    self.active_tab = ActiveTab::Diff;
                }
                if ui
                    .selectable_label(self.active_tab == ActiveTab::Collab, "Collab")
                    .clicked()
                {
                    self.active_tab = ActiveTab::Collab;
                }
            });
        });
    }

    /// Render the sidebar (sections for hex view, functions for disasm view).
    fn render_sidebar(&mut self, ctx: &egui::Context) {
        if !self.show_sidebar {
            return;
        }

        egui::SidePanel::left("sidebar")
            .default_width(200.0)
            .resizable(true)
            .show(ctx, |ui| match self.active_tab {
                ActiveTab::Hex => self.render_section_sidebar(ui),
                ActiveTab::Disassembly => self.render_function_sidebar(ui),
                ActiveTab::Graph => self.render_graph_function_sidebar(ui),
                ActiveTab::Decompiler => self.render_function_sidebar(ui),
                ActiveTab::Strings | ActiveTab::Imports | ActiveTab::Exports | ActiveTab::Types | ActiveTab::Console | ActiveTab::Diff | ActiveTab::Collab => {
                    self.render_info_sidebar(ui);
                }
            });
    }

    /// Render the section selector sidebar (Sprint 3).
    fn render_section_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.heading("Sections");
        ui.separator();

        if let Some(image) = &self.image {
            let sections: Vec<(String, u64, u64, u64, bool, bool, bool)> = image
                .sections
                .iter()
                .map(|s| {
                    (
                        s.name.clone(),
                        s.address,
                        s.size,
                        s.file_offset,
                        s.readable,
                        s.writable,
                        s.executable,
                    )
                })
                .collect();

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, (name, addr, size, offset, readable, writable, executable)) in
                    sections.iter().enumerate()
                {
                    let r = if *readable { 'R' } else { '-' };
                    let w = if *writable { 'W' } else { '-' };
                    let x = if *executable { 'X' } else { '-' };
                    let label =
                        format!("{} (0x{:x}, {} bytes) [{}{}{}]", name, addr, size, r, w, x);

                    let selected = self.selected_section == Some(i);
                    if ui.selectable_label(selected, &label).clicked() {
                        self.selected_section = Some(i);
                        self.hex_view.offset = *offset as usize;
                    }
                }
            });
        } else {
            ui.label("No binary loaded");
        }
    }

    /// Render the function list sidebar showing detected functions (Sprint 5).
    fn render_function_sidebar(&mut self, ui: &mut egui::Ui) {
        let func_count = self.analysis.functions.len();
        ui.heading(format!("Functions ({})", func_count));
        ui.separator();

        if func_count > 0 {
            // Collect function info; prefer debug signatures when available
            let funcs: Vec<(String, Option<String>, u64)> = self
                .analysis
                .functions
                .values()
                .map(|f| {
                    let sig = self.debug_info.signature_at(f.entry_addr).map(String::from);
                    (f.name.clone(), sig, f.entry_addr)
                })
                .collect();

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, (name, sig, addr)) in funcs.iter().enumerate() {
                    let label = if let Some(s) = sig {
                        format!("0x{:08x}  {}", addr, s)
                    } else {
                        format!("0x{:08x}  {}", addr, name)
                    };
                    let selected = self.selected_symbol == Some(i);
                    let response = ui.selectable_label(selected, &label);
                    if let Some(s) = sig {
                        response.clone().on_hover_text(s);
                    }
                    if response.clicked() {
                        self.selected_symbol = Some(i);
                        self.disasm_view.scroll_to_address = Some(*addr);
                        self.nav_history.push(*addr);
                    }
                }
            });
        } else if let Some(image) = &self.image {
            // Fallback to symbol table if no analysis yet
            let funcs: Vec<(String, u64)> = image
                .function_symbols()
                .iter()
                .map(|s| (s.name.clone(), s.address))
                .collect();

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, (name, addr)) in funcs.iter().enumerate() {
                    let label = format!("0x{:08x}  {}", addr, name);
                    let selected = self.selected_symbol == Some(i);
                    if ui.selectable_label(selected, &label).clicked() {
                        self.selected_symbol = Some(i);
                        self.disasm_view.scroll_to_address = Some(*addr);
                        self.nav_history.push(*addr);
                    }
                }
            });
        } else {
            ui.label("No binary loaded");
        }
    }

    /// Render the function list sidebar for the Graph view.
    fn render_graph_function_sidebar(&mut self, ui: &mut egui::Ui) {
        let func_count = self.analysis.functions.len();
        ui.heading(format!("Functions ({})", func_count));
        ui.separator();

        if func_count > 0 {
            let funcs: Vec<(String, u64)> = self
                .analysis
                .functions
                .values()
                .map(|f| (f.name.clone(), f.entry_addr))
                .collect();

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (name, addr) in &funcs {
                    let label = format!("0x{:08x}  {}", addr, name);
                    let selected = self.graph_view.selected_function == Some(*addr);
                    if ui.selectable_label(selected, &label).clicked() {
                        self.graph_view.select_function(*addr);
                    }
                }
            });
        } else {
            ui.label("No functions detected");
        }
    }

    /// Render a simple info sidebar for Strings/Imports/Exports views.
    fn render_info_sidebar(&self, ui: &mut egui::Ui) {
        ui.heading("Info");
        ui.separator();
        if let Some(image) = &self.image {
            ui.label(format!("File: {}", image.filename));
            ui.label(format!("Format: {}", image.format));
            ui.label(format!("Arch: {}", image.architecture));
            ui.label(format!("Size: {} bytes", image.data.len()));
            ui.separator();
            ui.label(format!("Sections: {}", image.sections.len()));
            ui.label(format!("Symbols: {}", image.symbols.len()));
            ui.label(format!("Imports: {}", image.imports().len()));
            ui.label(format!("Exports: {}", image.exports().len()));
        } else {
            ui.label("No binary loaded");
        }
    }

    /// Show file open dialog and load the selected binary.
    fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().set_title("Open Binary").pick_file() {
            self.open_binary(&path);
        }
    }

    /// Show two sequential file dialogs, load both binaries, and compute their diff (Sprint 14).
    fn open_diff_dialog(&mut self) {
        let old_path = rfd::FileDialog::new()
            .set_title("Open Old Binary (base)")
            .pick_file();
        let Some(old_path) = old_path else { return };

        let new_path = rfd::FileDialog::new()
            .set_title("Open New Binary (changed)")
            .pick_file();
        let Some(new_path) = new_path else { return };

        let old_image = match kiln_core::load_binary(&old_path) {
            Ok(img) => img,
            Err(e) => {
                self.error_dialog = Some(format!("Failed to load old binary: {}", e));
                return;
            }
        };
        let new_image = match kiln_core::load_binary(&new_path) {
            Ok(img) => img,
            Err(e) => {
                self.error_dialog = Some(format!("Failed to load new binary: {}", e));
                return;
            }
        };

        let old_insns = match kiln_core::disasm::disassemble_executable_sections(&old_image) {
            Ok(insns) => insns,
            Err(e) => {
                self.error_dialog = Some(format!("Failed to disassemble old binary: {}", e));
                return;
            }
        };
        let new_insns = match kiln_core::disasm::disassemble_executable_sections(&new_image) {
            Ok(insns) => insns,
            Err(e) => {
                self.error_dialog = Some(format!("Failed to disassemble new binary: {}", e));
                return;
            }
        };

        let mut old_analysis = kiln_core::analysis::AnalysisDatabase::new();
        old_analysis.index_instructions(old_insns);
        old_analysis.run_analysis(&old_image);

        let mut new_analysis = kiln_core::analysis::AnalysisDatabase::new();
        new_analysis.index_instructions(new_insns);
        new_analysis.run_analysis(&new_image);

        let result =
            kiln_core::diff::compute_diff(&old_analysis, &new_analysis, &old_image, &new_image);

        self.diff_view.diff_result = Some(result);
        self.diff_view.selected_function = None;
        self.diff_view.export_message = None;
        self.diff_view.old_filename = old_image.filename.clone();
        self.diff_view.new_filename = new_image.filename.clone();
        self.active_tab = ActiveTab::Diff;
        self.status_message = format!(
            "Diff: {} vs {} | {} functions compared",
            old_image.filename,
            new_image.filename,
            self.diff_view
                .diff_result
                .as_ref()
                .map_or(0, |r| r.function_matches.len())
        );
    }

    /// Show file dialog to pick and run a .rhai script file (Sprint 13).
    fn run_script_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Run Script")
            .add_filter("Rhai Script", &["rhai"])
            .pick_file()
        {
            self.console_view
                .run_file(&path, &self.analysis, &mut self.project);
            self.active_tab = ActiveTab::Console;
            self.disasm_view.invalidate_cache();
        }
    }

    /// Save the project to the current project_path, or prompt for one.
    fn save_project(&mut self) {
        if let Some(path) = self.project_path.clone() {
            match self.project.save(&path) {
                Ok(()) => {
                    log::info!("Project saved to {}", path.display());
                }
                Err(e) => {
                    log::error!("Failed to save project: {}", e);
                }
            }
        } else {
            self.save_project_as();
        }
    }

    /// Prompt for a path and save the project there.
    fn save_project_as(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Save Project")
            .add_filter("Kiln Project", &["kproj"])
            .save_file()
        {
            let path = if path.extension().is_none() {
                path.with_extension("kproj")
            } else {
                path
            };
            match self.project.save(&path) {
                Ok(()) => {
                    log::info!("Project saved to {}", path.display());
                    self.project_path = Some(path);
                }
                Err(e) => {
                    log::error!("Failed to save project: {}", e);
                }
            }
        }
    }

    /// Show open dialog for .kproj files, load project, then open the binary it references.
    fn open_project_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Open Project")
            .add_filter("Kiln Project", &["kproj"])
            .pick_file()
        {
            match kiln_project::Project::load(&path) {
                Ok(project) => {
                    let binary_path = PathBuf::from(&project.metadata.binary_path);
                    self.project = project;
                    self.project_path = Some(path);
                    self.undo_stack = UndoStack::default();
                    if binary_path.exists() {
                        self.open_binary_with_existing_project(&binary_path);
                    } else {
                        log::warn!(
                            "Binary not found at {}, open it manually",
                            binary_path.display()
                        );
                        self.status_message = format!(
                            "Project loaded, but binary not found: {}",
                            binary_path.display()
                        );
                    }
                }
                Err(e) => {
                    let msg = format!("Failed to load project: {}", e);
                    self.status_message = format!("Error loading project: {}", e);
                    self.error_dialog = Some(msg);
                    log::error!("Failed to load project: {}", e);
                }
            }
        }
    }

    /// Open a binary without resetting the project (used when loading from .kproj).
    fn open_binary_with_existing_project(&mut self, path: &std::path::Path) {
        match kiln_core::load_binary(path) {
            Ok(image) => {
                self.status_message = format!(
                    "{} | {} | {} | {} bytes",
                    image.filename,
                    image.format,
                    image.architecture,
                    image.data.len(),
                );

                match disassemble_executable_sections(&image) {
                    Ok(instructions) => {
                        self.analysis = AnalysisDatabase::new();
                        let first_addr = instructions.first().map(|i| i.address);
                        self.analysis.index_instructions(instructions);
                        self.analysis.run_analysis(&image);
                        self.disasm_view.invalidate_cache();
                        if let Some(addr) = first_addr {
                            self.disasm_view.scroll_to_address = Some(addr);
                        }
                    }
                    Err(e) => {
                        log::warn!("Disassembly failed: {}", e);
                    }
                }

                if let Some(section) = image.sections.first() {
                    self.hex_view.offset = section.file_offset as usize;
                }

                // Parse debug info (Sprint 12)
                self.debug_info = kiln_core::debug_info::parse_debug_info(&image);

                self.selected_section = None;
                self.selected_symbol = None;
                self.nav_history = NavigationHistory::default();
                self.graph_view = GraphView::default();
                self.strings_view.invalidate_cache();
                self.image = Some(image);
                self.active_tab = ActiveTab::Hex;
            }
            Err(e) => {
                let msg = format!("Failed to load binary: {}", e);
                self.status_message = format!("Error loading binary: {}", e);
                self.error_dialog = Some(msg);
                log::error!("Failed to load binary: {}", e);
            }
        }
    }

    /// Set a comment with undo tracking.
    fn set_comment_with_undo(&mut self, addr: u64, new_comment: Option<String>) {
        let old = self.project.get_comment(addr).map(|s| s.to_string());
        if old == new_comment {
            return;
        }
        self.undo_stack.redo.clear();
        self.undo_stack.undo.push(UndoAction::SetComment {
            addr,
            old,
            new: new_comment.clone(),
        });
        match new_comment {
            Some(c) => self.project.set_comment(addr, c),
            None => self.project.remove_comment(addr),
        }
    }

    /// Set a label with undo tracking.
    fn set_label_with_undo(&mut self, addr: u64, new_label: Option<String>) {
        let old = self.project.get_label(addr).map(|s| s.to_string());
        if old == new_label {
            return;
        }
        self.undo_stack.redo.clear();
        self.undo_stack.undo.push(UndoAction::SetLabel {
            addr,
            old,
            new: new_label.clone(),
        });
        match new_label {
            Some(l) => self.project.set_label(addr, l),
            None => self.project.remove_label(addr),
        }
        self.disasm_view.invalidate_cache();
    }

    /// Perform undo.
    fn perform_undo(&mut self) {
        if let Some(action) = self.undo_stack.undo.pop() {
            match &action {
                UndoAction::SetComment { addr, old, .. } => match old {
                    Some(c) => self.project.set_comment(*addr, c.clone()),
                    None => self.project.remove_comment(*addr),
                },
                UndoAction::SetLabel { addr, old, .. } => {
                    match old {
                        Some(l) => self.project.set_label(*addr, l.clone()),
                        None => self.project.remove_label(*addr),
                    }
                    self.disasm_view.invalidate_cache();
                }
            }
            self.undo_stack.redo.push(action);
        }
    }

    /// Perform redo.
    fn perform_redo(&mut self) {
        if let Some(action) = self.undo_stack.redo.pop() {
            match &action {
                UndoAction::SetComment { addr, new, .. } => match new {
                    Some(c) => self.project.set_comment(*addr, c.clone()),
                    None => self.project.remove_comment(*addr),
                },
                UndoAction::SetLabel { addr, new, .. } => {
                    match new {
                        Some(l) => self.project.set_label(*addr, l.clone()),
                        None => self.project.remove_label(*addr),
                    }
                    self.disasm_view.invalidate_cache();
                }
            }
            self.undo_stack.undo.push(action);
        }
    }

    /// Render and handle the comment dialog.
    fn render_comment_dialog(&mut self, ctx: &egui::Context) {
        if !self.comment_dialog.open {
            return;
        }

        let mut open = self.comment_dialog.open;
        egui::Window::new("Comment")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("Address: 0x{:08X}", self.comment_dialog.address));
                let response = ui.text_edit_singleline(&mut self.comment_dialog.input);

                // Focus the text edit on first frame
                if response.gained_focus() || !response.has_focus() {
                    response.request_focus();
                }

                ui.horizontal(|ui| {
                    let ok_clicked = ui.button("OK").clicked();
                    let enter_pressed =
                        response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                    if ok_clicked || enter_pressed {
                        let addr = self.comment_dialog.address;
                        let input = self.comment_dialog.input.trim().to_string();
                        if input.is_empty() {
                            self.set_comment_with_undo(addr, None);
                        } else {
                            self.set_comment_with_undo(addr, Some(input));
                        }
                        self.comment_dialog.open = false;
                    }

                    if ui.button("Delete").clicked() {
                        let addr = self.comment_dialog.address;
                        self.set_comment_with_undo(addr, None);
                        self.comment_dialog.open = false;
                    }

                    if ui.button("Cancel").clicked() {
                        self.comment_dialog.open = false;
                    }
                });
            });
        self.comment_dialog.open = open;
    }

    /// Render and handle the rename dialog.
    fn render_rename_dialog(&mut self, ctx: &egui::Context) {
        if !self.rename_dialog.open {
            return;
        }

        let mut open = self.rename_dialog.open;
        egui::Window::new("Rename")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("Address: 0x{:08X}", self.rename_dialog.address));
                let response = ui.text_edit_singleline(&mut self.rename_dialog.input);

                // Focus the text edit on first frame
                if response.gained_focus() || !response.has_focus() {
                    response.request_focus();
                }

                ui.horizontal(|ui| {
                    let ok_clicked = ui.button("OK").clicked();
                    let enter_pressed =
                        response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                    if ok_clicked || enter_pressed {
                        let addr = self.rename_dialog.address;
                        let input = self.rename_dialog.input.trim().to_string();
                        if input.is_empty() {
                            self.set_label_with_undo(addr, None);
                        } else {
                            self.set_label_with_undo(addr, Some(input));
                        }
                        self.rename_dialog.open = false;
                    }

                    if ui.button("Delete").clicked() {
                        let addr = self.rename_dialog.address;
                        self.set_label_with_undo(addr, None);
                        self.rename_dialog.open = false;
                    }

                    if ui.button("Cancel").clicked() {
                        self.rename_dialog.open = false;
                    }
                });
            });
        self.rename_dialog.open = open;
    }

    /// Render and handle the go-to-address dialog.
    fn render_goto_dialog(&mut self, ctx: &egui::Context) {
        if !self.goto_dialog.open {
            return;
        }

        let mut open = self.goto_dialog.open;
        egui::Window::new("Go to Address")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Enter address (hex, e.g. 0x401000):");
                let response = ui.text_edit_singleline(&mut self.goto_dialog.input);

                if let Some(err) = &self.goto_dialog.error {
                    ui.colored_label(egui::Color32::RED, err);
                }

                ui.horizontal(|ui| {
                    let go_clicked = ui.button("Go").clicked();
                    let enter_pressed =
                        response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                    if go_clicked || enter_pressed {
                        let input = self.goto_dialog.input.trim();
                        let input = input
                            .strip_prefix("0x")
                            .or_else(|| input.strip_prefix("0X"))
                            .unwrap_or(input);
                        match u64::from_str_radix(input, 16) {
                            Ok(addr) => {
                                self.navigate_to_address(addr);
                                self.goto_dialog.open = false;
                            }
                            Err(_) => {
                                self.goto_dialog.error = Some("Invalid hex address".to_string());
                            }
                        }
                    }

                    if ui.button("Cancel").clicked() {
                        self.goto_dialog.open = false;
                    }
                });
            });
        self.goto_dialog.open = open;
    }

    /// Render and handle the search dialog (Sprint 6).
    fn render_search_dialog(&mut self, ctx: &egui::Context) {
        if !self.search_dialog.open {
            return;
        }

        let mut open = self.search_dialog.open;
        egui::Window::new("Search")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(500.0)
            .default_height(400.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.radio_value(
                        &mut self.search_dialog.search_mode,
                        SearchMode::Text,
                        "Text",
                    );
                    ui.radio_value(
                        &mut self.search_dialog.search_mode,
                        SearchMode::HexBytes,
                        "Hex Bytes",
                    );
                });

                let hint = match self.search_dialog.search_mode {
                    SearchMode::Text => "Search mnemonics/operands...",
                    SearchMode::HexBytes => "Hex bytes (e.g. 90 C3 or 90C3)...",
                };
                ui.horizontal(|ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.search_dialog.query).hint_text(hint),
                    );
                    let search_clicked = ui.button("Search").clicked();
                    let enter_pressed =
                        response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                    if search_clicked || enter_pressed {
                        self.perform_search();
                    }
                });

                if let Some(err) = &self.search_dialog.error {
                    ui.colored_label(egui::Color32::RED, err);
                }

                ui.separator();
                ui.label(format!("{} results", self.search_dialog.results.len()));

                // Results list
                let results = self.search_dialog.results.clone();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for result in &results {
                        let label = format!("0x{:08X}  {}", result.address, result.context);
                        if ui
                            .selectable_label(false, &label)
                            .on_hover_text("Click to navigate")
                            .clicked()
                        {
                            self.active_tab = ActiveTab::Disassembly;
                            self.navigate_to_address(result.address);
                        }
                    }
                });
            });
        self.search_dialog.open = open;
    }

    /// Perform search based on current search dialog state.
    fn perform_search(&mut self) {
        let query = self.search_dialog.query.trim().to_string();
        if query.is_empty() {
            self.search_dialog.error = Some("Empty search query".to_string());
            return;
        }

        self.search_dialog.error = None;
        self.search_dialog.results.clear();

        match self.search_dialog.search_mode {
            SearchMode::Text => {
                let query_lower = query.to_lowercase();
                for insn in self.analysis.instructions.values() {
                    if insn.mnemonic.to_lowercase().contains(&query_lower)
                        || insn.operands.to_lowercase().contains(&query_lower)
                    {
                        self.search_dialog.results.push(SearchResult {
                            address: insn.address,
                            context: format!("{} {}", insn.mnemonic, insn.operands),
                        });
                        if self.search_dialog.results.len() >= 1000 {
                            break;
                        }
                    }
                }
            }
            SearchMode::HexBytes => {
                // Parse hex bytes from query (supports "90 C3" or "90C3")
                let hex_str: String = query.chars().filter(|c| !c.is_whitespace()).collect();
                if !hex_str.len().is_multiple_of(2) {
                    self.search_dialog.error =
                        Some("Hex string must have even number of digits".to_string());
                    return;
                }
                let pattern: Result<Vec<u8>, _> = (0..hex_str.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&hex_str[i..i + 2], 16))
                    .collect();
                match pattern {
                    Ok(pattern) => {
                        if let Some(image) = &self.image {
                            // Search in binary data
                            let data = &image.data;
                            let pat_len = pattern.len();
                            if pat_len > 0 && pat_len <= data.len() {
                                for i in 0..=data.len() - pat_len {
                                    if data[i..i + pat_len] == pattern[..] {
                                        // Try to find the virtual address
                                        let va = self.file_offset_to_va(i as u64);
                                        self.search_dialog.results.push(SearchResult {
                                            address: va,
                                            context: format!("offset 0x{:X}", i),
                                        });
                                        if self.search_dialog.results.len() >= 1000 {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        self.search_dialog.error = Some("Invalid hex bytes".to_string());
                    }
                }
            }
        }
    }

    /// Convert a file offset to a virtual address using section mapping.
    fn file_offset_to_va(&self, offset: u64) -> u64 {
        if let Some(image) = &self.image {
            for section in &image.sections {
                let sec_start = section.file_offset;
                let sec_end = sec_start + section.size;
                if offset >= sec_start && offset < sec_end {
                    return section.address + (offset - sec_start);
                }
            }
        }
        offset
    }

    /// Handle keyboard shortcuts.
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // Don't process single-key shortcuts if a text input has focus
        let any_dialog_open = self.goto_dialog.open
            || self.search_dialog.open
            || self.comment_dialog.open
            || self.rename_dialog.open
            || self.help_dialog.open
            || self.error_dialog.is_some()
            || self.struct_editor_dialog.open
            || self.enum_editor_dialog.open
            || self.apply_type_dialog.open;

        // F1: Help
        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.help_dialog.open = true;
            self.help_dialog.show_about = false;
        }

        // Ctrl+G: Go to address
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::G)) {
            self.goto_dialog.open = true;
            self.goto_dialog.input.clear();
            self.goto_dialog.error = None;
        }

        // Ctrl+O: Open file
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::O)) {
            self.open_file_dialog();
        }

        // Ctrl+S: Save project
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::S)) {
            self.save_project();
        }

        // Ctrl+F: Search
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F)) {
            self.search_dialog.open = true;
            self.search_dialog.error = None;
        }

        // Ctrl+Y or Ctrl+Shift+Z: Redo (check before Ctrl+Z to avoid conflict)
        if ctx.input(|i| {
            (i.modifiers.ctrl && i.key_pressed(egui::Key::Y))
                || (i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::Z))
        }) {
            self.perform_redo();
        }
        // Ctrl+Z: Undo (only without shift, so Ctrl+Shift+Z goes to redo above)
        else if ctx
            .input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Z) && !i.modifiers.shift)
        {
            self.perform_undo();
        }

        // Alt+Left: Navigate back
        if ctx.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft)) {
            self.navigate_back();
        }

        // Alt+Right: Navigate forward
        if ctx.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::ArrowRight)) {
            self.navigate_forward();
        }

        // Single-key shortcuts only when no dialog is open
        if !any_dialog_open {
            // ; (semicolon): Open comment dialog for selected address
            if ctx.input(|i| i.key_pressed(egui::Key::Semicolon)) {
                if let Some(addr) = self.get_current_selected_address() {
                    self.comment_dialog.address = addr;
                    self.comment_dialog.input =
                        self.project.get_comment(addr).unwrap_or("").to_string();
                    self.comment_dialog.open = true;
                }
            }

            // N: Open rename dialog
            if ctx.input(|i| i.key_pressed(egui::Key::N) && !i.modifiers.ctrl) {
                if let Some(addr) = self.get_current_selected_address() {
                    self.rename_dialog.address = addr;
                    self.rename_dialog.input =
                        self.project.get_label(addr).unwrap_or("").to_string();
                    self.rename_dialog.open = true;
                }
            }
        }

        // Escape: Close dialogs or navigate back
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.error_dialog.is_some() {
                self.error_dialog = None;
            } else if self.help_dialog.open {
                self.help_dialog.open = false;
            } else if self.struct_editor_dialog.open {
                self.struct_editor_dialog.open = false;
            } else if self.enum_editor_dialog.open {
                self.enum_editor_dialog.open = false;
            } else if self.apply_type_dialog.open {
                self.apply_type_dialog.open = false;
            } else if self.comment_dialog.open {
                self.comment_dialog.open = false;
            } else if self.rename_dialog.open {
                self.rename_dialog.open = false;
            } else if self.goto_dialog.open {
                self.goto_dialog.open = false;
            } else if self.search_dialog.open {
                self.search_dialog.open = false;
            } else {
                self.navigate_back();
            }
        }
    }

    /// Render the error dialog (Sprint 10).
    fn render_error_dialog(&mut self, ctx: &egui::Context) {
        if self.error_dialog.is_none() {
            return;
        }

        let mut open = true;
        let msg = self.error_dialog.clone().unwrap_or_default();
        egui::Window::new("⚠ Error")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(&msg);
                ui.add_space(8.0);
                if ui.button("OK").clicked() {
                    self.error_dialog = None;
                }
            });
        if !open {
            self.error_dialog = None;
        }
    }

    /// Render the help / keyboard shortcuts dialog (Sprint 10).
    fn render_help_dialog(&mut self, ctx: &egui::Context) {
        if !self.help_dialog.open {
            return;
        }

        let mut open = self.help_dialog.open;
        let title = if self.help_dialog.show_about {
            "About Kiln"
        } else {
            "Keyboard Shortcuts"
        };

        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(400.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                if self.help_dialog.show_about {
                    ui.heading("Kiln — Interactive Disassembler");
                    ui.add_space(4.0);
                    ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                    ui.add_space(8.0);
                    ui.label("An IDA Pro-style interactive disassembler built entirely in Rust.");
                    ui.add_space(4.0);
                    ui.label("License: AGPL-3.0");
                    ui.add_space(4.0);
                    ui.hyperlink_to("GitHub Repository", "https://github.com/Mike-Van-Camp/kiln");
                } else {
                    let shortcuts = [
                        ("Ctrl+O", "Open Binary"),
                        ("Ctrl+S", "Save Project"),
                        ("Ctrl+G", "Go to Address"),
                        ("Ctrl+F", "Find / Search"),
                        ("Ctrl+Z", "Undo"),
                        ("Ctrl+Y", "Redo"),
                        ("Alt+\u{2190}", "Navigate Back"),
                        ("Alt+\u{2192}", "Navigate Forward"),
                        (";", "Add Comment"),
                        ("N", "Rename Symbol"),
                        ("Escape", "Back / Close Dialog"),
                        ("F1", "Help"),
                    ];

                    egui::Grid::new("shortcut_grid")
                        .num_columns(2)
                        .spacing([40.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            for (key, desc) in &shortcuts {
                                ui.strong(*key);
                                ui.label(*desc);
                                ui.end_row();
                            }
                        });
                }
            });
        self.help_dialog.open = open;
    }

    /// Get the currently selected instruction address in the disassembly view.
    fn get_current_selected_address(&self) -> Option<u64> {
        if self.active_tab != ActiveTab::Disassembly {
            return None;
        }
        self.disasm_view
            .selected_address
            .or_else(|| self.disasm_view.cached_addresses.first().copied())
    }
}

impl eframe::App for KilnApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme
        if self.dark_mode {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }

        // Check if hex view has a pending navigation request
        if let Some(addr) = self.hex_view.take_pending_navigation() {
            self.active_tab = ActiveTab::Disassembly;
            self.navigate_to_address(addr);
        }

        // Check if disasm view has a pending navigation request
        let pending_nav = self.disasm_view.take_pending_navigation();
        if let Some(addr) = pending_nav {
            self.active_tab = ActiveTab::Disassembly;
            self.navigate_to_address(addr);
        }

        // Check if disasm view has a pending comment request
        if let Some(addr) = self.disasm_view.take_pending_comment() {
            self.comment_dialog.address = addr;
            self.comment_dialog.input = self.project.get_comment(addr).unwrap_or("").to_string();
            self.comment_dialog.open = true;
        }

        // Check if disasm view has a pending rename request
        if let Some(addr) = self.disasm_view.take_pending_rename() {
            self.rename_dialog.address = addr;
            self.rename_dialog.input = self.project.get_label(addr).unwrap_or("").to_string();
            self.rename_dialog.open = true;
        }

        // Check if hex view has a pending apply-type request
        if let Some(addr) = self.hex_view.take_pending_apply_type() {
            self.apply_type_dialog.open = true;
            self.apply_type_dialog.address = addr;
            self.apply_type_dialog.selected_type = "u8".to_string();
            self.apply_type_dialog.label.clear();
        }

        // Check if disasm view has a pending apply-type request
        if let Some(addr) = self.disasm_view.take_pending_apply_type() {
            self.apply_type_dialog.open = true;
            self.apply_type_dialog.address = addr;
            self.apply_type_dialog.selected_type = "u8".to_string();
            self.apply_type_dialog.label.clear();
        }

        self.handle_shortcuts(ctx);
        self.render_menu_bar(ctx);
        self.render_status_bar(ctx);
        self.render_tab_bar(ctx);
        self.render_goto_dialog(ctx);
        self.render_search_dialog(ctx);
        self.render_comment_dialog(ctx);
        self.render_rename_dialog(ctx);
        self.render_error_dialog(ctx);
        self.render_help_dialog(ctx);
        self.render_sidebar(ctx);

        // Render type system dialogs (Sprint 11)
        let all_type_names = self.project.all_type_names();
        if let Some((name, def)) = crate::views::types_view::render_struct_editor(
            ctx,
            &mut self.struct_editor_dialog,
            &all_type_names,
        ) {
            // If editing, remove the old name first
            if let Some(old_name) = self.struct_editor_dialog.editing.take() {
                if old_name != name {
                    self.project.remove_type_def(&old_name);
                }
            }
            self.project.add_type_def(name, def);
        }
        if let Some((name, def)) =
            crate::views::types_view::render_enum_editor(ctx, &mut self.enum_editor_dialog)
        {
            if let Some(old_name) = self.enum_editor_dialog.editing.take() {
                if old_name != name {
                    self.project.remove_type_def(&old_name);
                }
            }
            self.project.add_type_def(name, def);
        }
        if let Some((addr, applied)) = crate::views::types_view::render_apply_type_dialog(
            ctx,
            &mut self.apply_type_dialog,
            &all_type_names,
        ) {
            self.project.apply_type_at(addr, applied);
        }

        // Main central panel
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.image.is_none() && self.active_tab != ActiveTab::Types && self.active_tab != ActiveTab::Console && self.active_tab != ActiveTab::Diff && self.active_tab != ActiveTab::Collab && self.active_tab != ActiveTab::Decompiler {
                ui.centered_and_justified(|ui| {
                    ui.heading("Open a binary file to get started\n(File → Open or Ctrl+O)");
                });
                return;
            }

            match self.active_tab {
                ActiveTab::Hex => {
                    if let Some(image) = &self.image {
                        self.hex_view
                            .render(ui, &image.data, &self.project);
                    }
                }
                ActiveTab::Disassembly => {
                    let image_ref = self.image.as_ref();
                    self.disasm_view
                        .render(ui, &self.analysis, image_ref, &self.project, &self.debug_info);
                }
                ActiveTab::Graph => {
                    self.graph_view.render(ui, &self.analysis);
                }
                ActiveTab::Decompiler => {
                    self.decompiler_view
                        .render(ui, &self.analysis, &self.debug_info);
                    // Handle pending navigation from decompiler
                    if let Some(addr) = self.decompiler_view.pending_navigation.take() {
                        self.active_tab = ActiveTab::Disassembly;
                        self.navigate_to_address(addr);
                    }
                }
                ActiveTab::Strings => {
                    if let Some(image) = self.image.clone() {
                        if let Some(addr) = self.strings_view.render(ui, &image) {
                            self.active_tab = ActiveTab::Disassembly;
                            self.navigate_to_address(addr);
                        }
                    }
                }
                ActiveTab::Imports => {
                    if let Some(image) = self.image.clone() {
                        if let Some(addr) = self.imports_view.render(ui, &image) {
                            self.active_tab = ActiveTab::Disassembly;
                            self.navigate_to_address(addr);
                        }
                    }
                }
                ActiveTab::Exports => {
                    if let Some(image) = self.image.clone() {
                        if let Some(addr) = self.exports_view.render(ui, &image) {
                            self.active_tab = ActiveTab::Disassembly;
                            self.navigate_to_address(addr);
                        }
                    }
                }
                ActiveTab::Types => {
                    if let Some(types_action) = self.types_view.render(
                        ui,
                        &self.project,
                        &mut self.struct_editor_dialog,
                        &mut self.enum_editor_dialog,
                    ) {
                        match types_action {
                            crate::views::types_view::TypesAction::DeleteType(name) => {
                                self.project.remove_type_def(&name);
                            }
                        }
                    }
                }
                ActiveTab::Console => {
                    self.console_view.render(ui, &self.analysis, &mut self.project);
                }
                ActiveTab::Diff => {
                    self.diff_view.render(ui);
                }
                ActiveTab::Collab => {
                    self.collab_view.render(ui, &mut self.project);
                }
            }
        });
    }
}
