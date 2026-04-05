//! Diff view: side-by-side comparison of two binaries at function and instruction level.

use egui::{Color32, FontId, RichText, Ui};
use kiln_core::diff::{DiffResult, FunctionMatchStatus, InstructionDiffKind};

/// Color constants for diff rendering.
const COLOR_ADDED: Color32 = Color32::from_rgb(80, 200, 80);
const COLOR_REMOVED: Color32 = Color32::from_rgb(220, 80, 80);
const COLOR_MODIFIED: Color32 = Color32::from_rgb(220, 180, 50);
const COLOR_SAME: Color32 = Color32::GRAY;

/// State for the binary diff view.
#[derive(Default)]
pub struct DiffView {
    pub diff_result: Option<DiffResult>,
    pub selected_function: Option<usize>,
    pub old_filename: String,
    pub new_filename: String,
    /// Text of the last exported report for clipboard feedback.
    pub export_message: Option<String>,
}

impl DiffView {
    /// Main render entry point.
    pub fn render(&mut self, ui: &mut Ui) {
        if self.diff_result.is_none() {
            ui.centered_and_justified(|ui| {
                ui.heading(
                    "No diff loaded.\nUse File → Open Diff... to compare two binaries.",
                );
            });
            return;
        }

        self.render_header(ui);
        ui.separator();
        self.render_summary(ui);
        ui.separator();
        self.render_panes(ui);
    }

    /// Render header with filenames and export button.
    fn render_header(&mut self, ui: &mut Ui) {
        let result = self.diff_result.as_ref().unwrap();
        let old_filename = &result.old_filename;
        let new_filename = &result.new_filename;

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("Old: {}", old_filename))
                    .color(COLOR_REMOVED)
                    .strong(),
            );
            ui.separator();
            ui.label(
                RichText::new(format!("New: {}", new_filename))
                    .color(COLOR_ADDED)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("📋 Export Report").clicked() {
                    let report =
                        kiln_core::diff::export_diff_report(self.diff_result.as_ref().unwrap());
                    ui.ctx().copy_text(report);
                    self.export_message = Some("Report copied to clipboard!".to_string());
                }
                if let Some(msg) = &self.export_message {
                    ui.label(RichText::new(msg).color(Color32::from_rgb(100, 200, 100)));
                }
            });
        });
    }

    /// Render summary counts bar.
    fn render_summary(&self, ui: &mut Ui) {
        let result = self.diff_result.as_ref().unwrap();
        let matches = &result.function_matches;

        let matched = matches.iter().filter(|m| m.status == FunctionMatchStatus::Matched).count();
        let modified = matches.iter().filter(|m| m.status == FunctionMatchStatus::Modified).count();
        let added = matches.iter().filter(|m| m.status == FunctionMatchStatus::Added).count();
        let removed = matches.iter().filter(|m| m.status == FunctionMatchStatus::Removed).count();

        ui.horizontal(|ui| {
            ui.label(format!("{} functions total", matches.len()));
            ui.separator();
            ui.label(RichText::new(format!("✓ {matched}")).color(COLOR_SAME));
            ui.label(RichText::new(format!("≠ {modified}")).color(COLOR_MODIFIED));
            ui.label(RichText::new(format!("+ {added}")).color(COLOR_ADDED));
            ui.label(RichText::new(format!("− {removed}")).color(COLOR_REMOVED));
        });
    }

    /// Render the two-pane layout: function list + instruction diff.
    fn render_panes(&mut self, ui: &mut Ui) {
        let available = ui.available_size();
        let left_width = (available.x * 0.35).clamp(200.0, 400.0);

        // Capture the click from the function list (index of clicked item).
        let mut clicked_index: Option<usize> = None;

        let result = self.diff_result.as_ref().unwrap();
        let function_matches = &result.function_matches;
        let selected = self.selected_function;

        ui.horizontal(|ui| {
            // Left pane – function list
            ui.vertical(|ui| {
                ui.set_width(left_width);
                ui.heading("Functions");
                ui.separator();

                let mono = FontId::monospace(13.0);
                let row_height = ui.text_style_height(&egui::TextStyle::Monospace) + 2.0;
                let total_rows = function_matches.len();

                egui::ScrollArea::vertical()
                    .id_salt("diff_func_list")
                    .auto_shrink([false, false])
                    .show_rows(ui, row_height, total_rows, |ui, row_range| {
                        for idx in row_range {
                            let m = &function_matches[idx];
                            let (icon, color) = match m.status {
                                FunctionMatchStatus::Matched => ("✓", COLOR_SAME),
                                FunctionMatchStatus::Added => ("+", COLOR_ADDED),
                                FunctionMatchStatus::Removed => ("−", COLOR_REMOVED),
                                FunctionMatchStatus::Modified => ("≠", COLOR_MODIFIED),
                            };
                            let name = m
                                .old_function
                                .as_deref()
                                .or(m.new_function.as_deref())
                                .unwrap_or("<unknown>");
                            let label = format!(
                                "{} {} ({:.0}%)",
                                icon,
                                name,
                                m.similarity * 100.0
                            );
                            let is_selected = selected == Some(idx);
                            let response = ui.selectable_label(
                                is_selected,
                                RichText::new(&label).font(mono.clone()).color(color),
                            );
                            if response.clicked() {
                                clicked_index = Some(idx);
                            }
                        }
                    });
            });

            ui.separator();

            // Right pane – instruction diff
            ui.vertical(|ui| {
                if let Some(sel) = selected {
                    if sel < function_matches.len() {
                        let m = &function_matches[sel];
                        let name = m
                            .old_function
                            .as_deref()
                            .or(m.new_function.as_deref())
                            .unwrap_or("<unknown>");
                        ui.heading(format!("Instructions: {}", name));
                        ui.separator();

                        if let Some(diffs) = result.instruction_diffs.get(name) {
                            Self::render_instruction_diffs(ui, diffs);
                        } else {
                            match m.status {
                                FunctionMatchStatus::Added => {
                                    ui.label(
                                        RichText::new("Function only exists in the new binary.")
                                            .color(COLOR_ADDED),
                                    );
                                }
                                FunctionMatchStatus::Removed => {
                                    ui.label(
                                        RichText::new("Function only exists in the old binary.")
                                            .color(COLOR_REMOVED),
                                    );
                                }
                                _ => {
                                    ui.label("No instruction diff available.");
                                }
                            }
                        }
                    }
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label("Select a function to view instruction-level diff.");
                    });
                }
            });
        });

        // Apply selection change after the immutable borrow of diff_result ends.
        if let Some(idx) = clicked_index {
            self.selected_function = Some(idx);
        }
    }

    /// Render the instruction-level diff table.
    fn render_instruction_diffs(
        ui: &mut Ui,
        diffs: &[kiln_core::diff::InstructionDiff],
    ) {
        let mono = FontId::monospace(13.0);

        // Column header
        ui.horizontal(|ui| {
            let header = format!(
                "{:<3} {:<18} {:<24} │ {:<18} {:<24}",
                "", "Old Address", "Old Instruction", "New Address", "New Instruction"
            );
            ui.label(
                RichText::new(header)
                    .font(mono.clone())
                    .color(Color32::GRAY),
            );
        });
        ui.separator();

        let row_height = ui.text_style_height(&egui::TextStyle::Monospace) + 2.0;
        let total_rows = diffs.len();

        egui::ScrollArea::vertical()
            .id_salt("diff_insn_list")
            .auto_shrink([false, false])
            .show_rows(ui, row_height, total_rows, |ui, row_range| {
                for idx in row_range {
                    let d = &diffs[idx];
                    let (prefix, color) = match d.kind {
                        InstructionDiffKind::Same => (" ", COLOR_SAME),
                        InstructionDiffKind::Added => ("+", COLOR_ADDED),
                        InstructionDiffKind::Removed => ("-", COLOR_REMOVED),
                        InstructionDiffKind::Modified => ("~", COLOR_MODIFIED),
                    };

                    let old_part = if let Some(old) = &d.old_instruction {
                        format!(
                            "0x{:08x}  {:<8} {}",
                            old.address, old.mnemonic, old.operands
                        )
                    } else {
                        " ".repeat(30)
                    };
                    let new_part = if let Some(new) = &d.new_instruction {
                        format!(
                            "0x{:08x}  {:<8} {}",
                            new.address, new.mnemonic, new.operands
                        )
                    } else {
                        String::new()
                    };

                    let line = format!("{:<3} {:<30} │ {}", prefix, old_part, new_part);
                    let response = ui.selectable_label(
                        false,
                        RichText::new(&line)
                            .font(mono.clone())
                            .color(color),
                    );

                    // Hover tooltip with full instruction details
                    let mut hover_text = String::new();
                    if let Some(old) = &d.old_instruction {
                        hover_text.push_str(&format!(
                            "Old: 0x{:08x}  {} {}\n",
                            old.address, old.mnemonic, old.operands
                        ));
                    }
                    if let Some(new) = &d.new_instruction {
                        hover_text.push_str(&format!(
                            "New: 0x{:08x}  {} {}",
                            new.address, new.mnemonic, new.operands
                        ));
                    }
                    let response = if !hover_text.is_empty() {
                        response.on_hover_text(hover_text)
                    } else {
                        response
                    };

                    // Right-click context menu
                    response.context_menu(|ui| {
                        if let Some(old) = &d.old_instruction {
                            if ui.button("Copy Old Address").clicked() {
                                ui.ctx().copy_text(format!("0x{:08x}", old.address));
                                ui.close_menu();
                            }
                        }
                        if let Some(new) = &d.new_instruction {
                            if ui.button("Copy New Address").clicked() {
                                ui.ctx().copy_text(format!("0x{:08x}", new.address));
                                ui.close_menu();
                            }
                        }
                        if let Some(old) = &d.old_instruction {
                            if ui.button("Copy Old Instruction").clicked() {
                                ui.ctx().copy_text(format!(
                                    "{} {}",
                                    old.mnemonic, old.operands
                                ));
                                ui.close_menu();
                            }
                        }
                        if let Some(new) = &d.new_instruction {
                            if ui.button("Copy New Instruction").clicked() {
                                ui.ctx().copy_text(format!(
                                    "{} {}",
                                    new.mnemonic, new.operands
                                ));
                                ui.close_menu();
                            }
                        }
                    });
                }
            });
    }
}
