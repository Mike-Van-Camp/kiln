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
                self.image = Some(image);
                self.active_tab = ActiveTab::Hex;
            }
            Err(e) => {
                self.status_message = format!("Error loading binary: {}", e);
                log::error!("Failed to load binary: {}", e);
            }
        }
    }

    /// Navigate to a specific address in the current view.
    pub fn navigate_to_address(&mut self, addr: u64) {
        match self.active_tab {
            ActiveTab::Hex => {
                if let Some(image) = &self.image {
                    // Find which section contains this address and compute file offset
                    for section in &image.sections {
                        if addr >= section.address && addr < section.address + section.size {
                            let offset_in_section = addr - section.address;
                            self.hex_view.offset =
                                section.file_offset as usize + offset_in_section as usize;
                            return;
                        }
                    }
                    // Fallback: treat address as file offset
                    self.hex_view.offset = addr as usize;
                }
            }
            ActiveTab::Disassembly => {
                self.disasm_view.scroll_to_address = Some(addr);
            }
        }
    }

    /// Render the menu bar.
    fn render_menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open...").clicked() {
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
                    if ui.button("Go to Address (Ctrl+G)").clicked() {
                        self.goto_dialog.open = true;
                        self.goto_dialog.input.clear();
                        self.goto_dialog.error = None;
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

    /// Render the function/symbol list sidebar (Sprint 4).
    fn render_function_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.heading("Functions");
        ui.separator();

        if let Some(image) = &self.image {
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

    /// Handle keyboard shortcuts.
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
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
    }
}

impl eframe::App for KilnApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_shortcuts(ctx);
        self.render_menu_bar(ctx);
        self.render_status_bar(ctx);
        self.render_tab_bar(ctx);
        self.render_goto_dialog(ctx);
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
