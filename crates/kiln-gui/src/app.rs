//! Main application state and frame rendering for Kiln GUI.

use eframe::egui;
use kiln_core::analysis::AnalysisDatabase;
use kiln_core::disasm::disassemble_executable_sections;
use kiln_core::model::BinaryImage;

use crate::views::disasm_view::DisasmView;
use crate::views::hex_view::HexView;

/// Which main view tab is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Hex,
    Disassembly,
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
}

impl Default for KilnApp {
    fn default() -> Self {
        Self {
            image: None,
            analysis: AnalysisDatabase::new(),
            active_tab: ActiveTab::Hex,
            hex_view: HexView::default(),
            disasm_view: DisasmView::default(),
            status_message: "No file loaded".to_string(),
            selected_section: None,
            selected_symbol: None,
            goto_dialog: GotoDialog::default(),
            search_dialog: SearchDialog::default(),
            nav_history: NavigationHistory::default(),
            show_sidebar: true,
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

                self.selected_section = None;
                self.selected_symbol = None;
                self.nav_history = NavigationHistory::default();
                self.image = Some(image);
                self.active_tab = ActiveTab::Hex;
            }
            Err(e) => {
                self.status_message = format!("Error loading binary: {}", e);
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
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui
                        .checkbox(&mut self.show_sidebar, "Show Sidebar")
                        .clicked()
                    {
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
            });
        });
    }

    /// Render the status bar at the bottom.
    fn render_status_bar(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_message);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let func_count = self.analysis.functions.len();
                    let xref_count = self.analysis.xrefs.len();
                    if func_count > 0 {
                        ui.label(format!("{} functions | {} xrefs", func_count, xref_count));
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
            });
    }

    /// Render the section selector sidebar (Sprint 3).
    fn render_section_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.heading("Sections");
        ui.separator();

        if let Some(image) = &self.image {
            let sections: Vec<(String, u64, u64, u64, bool)> = image
                .sections
                .iter()
                .map(|s| {
                    (
                        s.name.clone(),
                        s.address,
                        s.size,
                        s.file_offset,
                        s.executable,
                    )
                })
                .collect();

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, (name, addr, size, offset, executable)) in sections.iter().enumerate() {
                    let label = if *executable {
                        format!("{} (0x{:x}, {} bytes) [X]", name, addr, size)
                    } else {
                        format!("{} (0x{:x}, {} bytes)", name, addr, size)
                    };

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
            // Show detected functions from analysis (sorted by address via BTreeMap)
            let funcs: Vec<(String, u64)> = self
                .analysis
                .functions
                .values()
                .map(|f| (f.name.clone(), f.entry_addr))
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

    /// Show file open dialog and load the selected binary.
    fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().set_title("Open Binary").pick_file() {
            self.open_binary(&path);
        }
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
        // Ctrl+G or G (without text focus): Go to address
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::G)) {
            self.goto_dialog.open = true;
            self.goto_dialog.input.clear();
            self.goto_dialog.error = None;
        }

        // Ctrl+O: Open file
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::O)) {
            self.open_file_dialog();
        }

        // Ctrl+F: Search
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F)) {
            self.search_dialog.open = true;
            self.search_dialog.error = None;
        }

        // Alt+Left: Navigate back
        if ctx.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft)) {
            self.navigate_back();
        }

        // Alt+Right: Navigate forward
        if ctx.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::ArrowRight)) {
            self.navigate_forward();
        }

        // Escape: Navigate back (or close dialogs)
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.goto_dialog.open {
                self.goto_dialog.open = false;
            } else if self.search_dialog.open {
                self.search_dialog.open = false;
            } else {
                self.navigate_back();
            }
        }
    }
}

impl eframe::App for KilnApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check if disasm view has a pending navigation request
        let pending_nav = self.disasm_view.take_pending_navigation();
        if let Some(addr) = pending_nav {
            self.active_tab = ActiveTab::Disassembly;
            self.navigate_to_address(addr);
        }

        self.handle_shortcuts(ctx);
        self.render_menu_bar(ctx);
        self.render_status_bar(ctx);
        self.render_tab_bar(ctx);
        self.render_goto_dialog(ctx);
        self.render_search_dialog(ctx);
        self.render_sidebar(ctx);

        // Main central panel
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.image.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.heading("Open a binary file to get started\n(File → Open or Ctrl+O)");
                });
                return;
            }

            match self.active_tab {
                ActiveTab::Hex => {
                    if let Some(image) = &self.image {
                        self.hex_view.render(ui, &image.data);
                    }
                }
                ActiveTab::Disassembly => {
                    let image_ref = self.image.as_ref();
                    self.disasm_view.render(ui, &self.analysis, image_ref);
                }
            }
        });
    }
}
