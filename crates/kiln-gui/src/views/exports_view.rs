//! Exports view: display exported symbols from the binary.

use egui::{Color32, FontId, RichText, Ui};
use kiln_core::model::BinaryImage;

/// State for the exports view panel.
pub struct ExportsView {
    filter: String,
    sort_by_name: bool,
    sort_ascending: bool,
}

impl Default for ExportsView {
    fn default() -> Self {
        Self {
            filter: String::new(),
            sort_by_name: false,
            sort_ascending: true,
        }
    }
}

impl ExportsView {
    /// Render the exports view. Returns an optional address to navigate to.
    pub fn render(&mut self, ui: &mut Ui, image: &BinaryImage) -> Option<u64> {
        let mut navigate_to = None;

        let exports = image.exports();

        // Filter bar
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.text_edit_singleline(&mut self.filter);
        });
        ui.separator();

        let filter_lower = self.filter.to_lowercase();
        let mut filtered: Vec<_> = exports
            .iter()
            .filter(|s| filter_lower.is_empty() || s.name.to_lowercase().contains(&filter_lower))
            .collect();

        // Sort filtered list
        {
            let by_name = self.sort_by_name;
            let asc = self.sort_ascending;
            filtered.sort_by(|a, b| {
                let cmp = if by_name {
                    a.name.cmp(&b.name).then_with(|| a.address.cmp(&b.address))
                } else {
                    a.address.cmp(&b.address)
                };
                if asc {
                    cmp
                } else {
                    cmp.reverse()
                }
            });
        }

        ui.label(format!(
            "{} exports ({} shown)",
            exports.len(),
            filtered.len()
        ));
        ui.separator();

        // Header
        let mono = FontId::monospace(13.0);
        ui.horizontal(|ui| {
            let addr_arrow = if !self.sort_by_name {
                if self.sort_ascending { " ▲" } else { " ▼" }
            } else {
                ""
            };
            let name_arrow = if self.sort_by_name {
                if self.sort_ascending { " ▲" } else { " ▼" }
            } else {
                ""
            };
            let addr_label = format!("Address     {}", addr_arrow);
            if ui
                .selectable_label(
                    !self.sort_by_name,
                    RichText::new(addr_label).font(mono.clone()).color(Color32::GRAY),
                )
                .clicked()
            {
                if !self.sort_by_name {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.sort_by_name = false;
                    self.sort_ascending = true;
                }
            }
            ui.label(
                RichText::new("Size     ")
                    .font(mono.clone())
                    .color(Color32::GRAY),
            );
            let name_label = format!("Name{}", name_arrow);
            if ui
                .selectable_label(
                    self.sort_by_name,
                    RichText::new(name_label).font(mono.clone()).color(Color32::GRAY),
                )
                .clicked()
            {
                if self.sort_by_name {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.sort_by_name = true;
                    self.sort_ascending = true;
                }
            }
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
                    let line = format!("0x{:08X}   {:<6}   {}", sym.address, sym.size, sym.name);
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
