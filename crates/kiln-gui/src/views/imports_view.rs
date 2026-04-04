//! Imports view: display imported symbols from the binary.

use egui::{Color32, FontId, RichText, Ui};
use kiln_core::model::BinaryImage;

/// State for the imports view panel.
#[derive(Default)]
pub struct ImportsView {
    filter: String,
}

impl ImportsView {
    /// Render the imports view. Returns an optional address to navigate to.
    pub fn render(&mut self, ui: &mut Ui, image: &BinaryImage) -> Option<u64> {
        let mut navigate_to = None;

        let imports = image.imports();

        // Filter bar
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.text_edit_singleline(&mut self.filter);
        });
        ui.separator();

        let filter_lower = self.filter.to_lowercase();
        let filtered: Vec<_> = imports
            .iter()
            .filter(|s| filter_lower.is_empty() || s.name.to_lowercase().contains(&filter_lower))
            .collect();

        ui.label(format!(
            "{} imports ({} shown)",
            imports.len(),
            filtered.len()
        ));
        ui.separator();

        // Header
        let mono = FontId::monospace(13.0);
        ui.horizontal(|ui| {
            let header = format!("{:<12} {}", "Address", "Name");
            ui.label(
                RichText::new(header)
                    .font(mono.clone())
                    .color(Color32::GRAY),
            );
        });
        ui.separator();

        // Scrollable list
        let row_height = ui.text_style_height(&egui::TextStyle::Monospace) + 2.0;
        let total_rows = filtered.len();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(ui, row_height, total_rows, |ui, row_range| {
                for row_idx in row_range {
                    let sym = filtered[row_idx];
                    let line = format!("0x{:08X}   {}", sym.address, sym.name);
                    let response =
                        ui.selectable_label(false, RichText::new(&line).font(mono.clone()));
                    if response.clicked() {
                        navigate_to = Some(sym.address);
                    }
                    let addr = sym.address;
                    let name = sym.name.clone();
                    response.context_menu(|ui| {
                        if ui.button("Copy Address").clicked() {
                            ui.ctx().copy_text(format!("0x{:08X}", addr));
                            ui.close_menu();
                        }
                        if ui.button("Copy Name").clicked() {
                            ui.ctx().copy_text(name.clone());
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Go to Disassembly").clicked() {
                            navigate_to = Some(addr);
                            ui.close_menu();
                        }
                    });
                }
            });

        navigate_to
    }
}
