//! Decompiler / pseudo-code view panel with syntax highlighting,
//! synchronized navigation, and copy-to-clipboard support.

use egui::{Color32, FontId, RichText, Ui};
use kiln_core::analysis::AnalysisDatabase;
use kiln_core::debug_info::DebugInfo;
use kiln_core::decompiler::{decompile_function, generate_pseudo_code, DecompiledFunction};

/// State for the decompiler pseudo-code view.
#[derive(Default)]
pub struct DecompilerView {
    /// Currently decompiled function (cached result).
    pub decompiled: Option<DecompiledFunction>,
    /// Rendered pseudo-code text (cached).
    pub pseudo_code: String,
    /// Entry address of the currently selected function.
    pub selected_function_addr: Option<u64>,
    /// Address to navigate to in the disassembly view (set on line click).
    pub pending_navigation: Option<u64>,
    /// Lines of pseudo-code (cached split for rendering).
    lines: Vec<String>,
}

impl DecompilerView {
    /// Select and decompile the function at `addr`.
    pub fn select_function(
        &mut self,
        addr: u64,
        analysis: &AnalysisDatabase,
        debug_info: &DebugInfo,
    ) {
        if self.selected_function_addr == Some(addr) && self.decompiled.is_some() {
            return; // already decompiled
        }
        self.selected_function_addr = Some(addr);

        if let Some(func) = analysis.functions.get(&addr) {
            let decomp = decompile_function(func, analysis, debug_info);
            self.pseudo_code = generate_pseudo_code(&decomp);
            self.lines = self.pseudo_code.lines().map(String::from).collect();
            self.decompiled = Some(decomp);
        } else {
            self.decompiled = None;
            self.pseudo_code = String::new();
            self.lines.clear();
        }
    }

    /// Render the decompiler view panel.
    pub fn render(&mut self, ui: &mut Ui, analysis: &AnalysisDatabase, debug_info: &DebugInfo) {
        // Function selector dropdown
        ui.horizontal(|ui| {
            ui.label("Function:");
            let current_label = self
                .decompiled
                .as_ref()
                .map(|d| d.name.clone())
                .unwrap_or_else(|| "(none)".to_string());

            egui::ComboBox::from_id_salt("decompiler_func_selector")
                .selected_text(&current_label)
                .width(300.0)
                .show_ui(ui, |ui| {
                    let mut addrs: Vec<(u64, String)> = analysis
                        .functions
                        .iter()
                        .map(|(&addr, f)| (addr, f.name.clone()))
                        .collect();
                    addrs.sort_by_key(|(a, _)| *a);
                    for (addr, name) in &addrs {
                        let label = format!("{} (0x{:x})", name, addr);
                        let selected = self.selected_function_addr == Some(*addr);
                        if ui.selectable_label(selected, &label).clicked() {
                            self.select_function(*addr, analysis, debug_info);
                        }
                    }
                });

            ui.separator();

            // Copy to clipboard button
            if ui.button("📋 Copy to Clipboard").clicked() && !self.pseudo_code.is_empty() {
                ui.ctx().copy_text(self.pseudo_code.clone());
            }
        });

        ui.separator();

        if self.decompiled.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label("Select a function to decompile");
            });
            return;
        }

        // Pseudo-code display with syntax highlighting
        let mono = FontId::monospace(13.0);
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Build address map indexed by line number
                let line_addrs: Vec<Option<u64>> = self.build_line_address_map();

                for (line_idx, line) in self.lines.iter().enumerate() {
                    let addr = line_addrs.get(line_idx).copied().flatten();

                    ui.horizontal(|ui| {
                        // Line number gutter
                        let line_num = format!("{:4} ", line_idx + 1);
                        ui.label(
                            RichText::new(line_num)
                                .font(mono.clone())
                                .color(Color32::GRAY),
                        );

                        // Syntax-highlighted line
                        let response = ui.add(
                            egui::Label::new(highlight_line(line, &mono)).sense(egui::Sense::click()),
                        );

                        // Click to navigate to disassembly
                        if response.clicked() {
                            if let Some(a) = addr {
                                self.pending_navigation = Some(a);
                            }
                        }

                        if let Some(a) = addr {
                            response.on_hover_text(format!("0x{:x}", a));
                        }
                    });
                }
            });
    }

    /// Build a mapping from pseudo-code line index to original address.
    fn build_line_address_map(&self) -> Vec<Option<u64>> {
        let decomp = match &self.decompiled {
            Some(d) => d,
            None => return vec![None; self.lines.len()],
        };

        // The address_map maps statement index → address.
        // We approximate: the pseudo-code is rendered statement by statement,
        // each statement producing one or more lines.
        // We do a simple heuristic: distribute addresses across lines.
        let mut result = vec![None; self.lines.len()];
        if self.lines.is_empty() || decomp.address_map.is_empty() {
            return result;
        }

        // Walk through statement addresses and assign to lines.
        // Line 0 is the signature line `{`; body starts at line 1.
        let sorted_stmts: Vec<(usize, u64)> = decomp.address_map.iter().map(|(&k, &v)| (k, v)).collect();

        // Simple heuristic: for each statement index, map its address to the
        // corresponding body line (offset by 1 for the signature line).
        for &(stmt_idx, addr) in &sorted_stmts {
            // +1 for the signature line at the top
            let line_idx = stmt_idx + 1;
            if line_idx < result.len() {
                result[line_idx] = Some(addr);
            }
        }

        result
    }
}

// ---------------------------------------------------------------------------
// Syntax highlighting helpers
// ---------------------------------------------------------------------------

/// Highlight a single line of pseudo-code and return a `RichText` `LayoutJob`.
fn highlight_line(line: &str, font: &FontId) -> RichText {
    // Very simple keyword-based highlighting. We colour the whole line based
    // on the dominant construct for simplicity, since egui `Label` takes a
    // single `RichText` (not a layout job with multiple spans).

    let trimmed = line.trim();

    if trimmed.starts_with("//") {
        return RichText::new(line)
            .font(font.clone())
            .color(Color32::from_rgb(106, 153, 85)); // green comment
    }

    if trimmed.starts_with("__asm(") {
        return RichText::new(line)
            .font(font.clone())
            .color(Color32::from_rgb(180, 180, 140)); // muted yellow for raw asm
    }

    // Check for keywords
    let kw_color = Color32::from_rgb(86, 156, 214); // blue
    for kw in &[
        "if ", "else ", "while ", "do ", "for ", "switch ", "return", "goto ",
    ] {
        if trimmed.starts_with(kw) || trimmed == "} else {" {
            return RichText::new(line).font(font.clone()).color(kw_color);
        }
    }

    // Return statement
    if trimmed.starts_with("return") {
        return RichText::new(line).font(font.clone()).color(kw_color);
    }

    // Braces only
    if trimmed == "{" || trimmed == "}" || trimmed == "};" {
        return RichText::new(line)
            .font(font.clone())
            .color(Color32::from_rgb(180, 180, 180));
    }

    // Signature line (first line usually)
    if trimmed.starts_with("void ") || trimmed.starts_with("int ") || trimmed.starts_with("char ") {
        return RichText::new(line)
            .font(font.clone())
            .color(Color32::from_rgb(220, 220, 170)); // light yellow for signature
    }

    // Default: light foreground
    RichText::new(line)
        .font(font.clone())
        .color(Color32::from_rgb(212, 212, 212))
}
