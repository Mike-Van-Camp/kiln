//! Hex view panel: virtual-scrolling hex dump (addr | hex bytes | ASCII).

use egui::{Color32, FontId, RichText, Ui};
use kiln_project::{Project, TypeDef};

/// Number of bytes displayed per row in the hex view.
const BYTES_PER_ROW: usize = 16;

/// Color constants for the hex view.
const COLOR_ADDRESS: Color32 = Color32::from_rgb(100, 149, 237);
const COLOR_HEX: Color32 = Color32::LIGHT_GRAY;
const COLOR_ASCII: Color32 = Color32::from_rgb(180, 180, 120);
const COLOR_SELECTED_BG: Color32 = Color32::from_rgb(50, 60, 80);
const COLOR_TYPE_OVERLAY: Color32 = Color32::from_rgb(180, 220, 140);
const COLOR_FIELD_LABEL: Color32 = Color32::from_rgb(180, 180, 255);

/// State for the hex view panel.
#[derive(Default)]
pub struct HexView {
    /// Current file offset displayed at the top of the view.
    pub offset: usize,
    /// Currently selected/clicked row offset.
    pub selected_row: Option<usize>,
    /// Currently selected byte offset (finer granularity than `selected_row`).
    pub selected_byte: Option<usize>,
    /// Number of bytes in the current selection (for future range selection).
    pub selected_byte_count: usize,
    /// Pending navigation request from context menu.
    pending_navigation: Option<u64>,
    /// Pending apply-type request from context menu.
    pending_apply_type: Option<u64>,
}

impl HexView {
    /// Returns the currently selected byte offset, if any.
    #[allow(dead_code)]
    pub fn selected_byte_offset(&self) -> Option<usize> {
        self.selected_byte
    }

    /// Take the pending navigation address (if any), clearing it.
    pub fn take_pending_navigation(&mut self) -> Option<u64> {
        self.pending_navigation.take()
    }

    /// Take the pending apply-type address (if any), clearing it.
    pub fn take_pending_apply_type(&mut self) -> Option<u64> {
        self.pending_apply_type.take()
    }

    /// Build the hex bytes string for a row.
    fn format_hex_bytes(row_data: &[u8]) -> String {
        let mut hex_str = String::with_capacity(BYTES_PER_ROW * 3 + 1);
        for (i, byte) in row_data.iter().enumerate() {
            if i == 8 {
                hex_str.push(' ');
            }
            hex_str.push_str(&format!("{:02X} ", byte));
        }
        // Pad if row is incomplete
        for i in row_data.len()..BYTES_PER_ROW {
            if i == 8 {
                hex_str.push(' ');
            }
            hex_str.push_str("   ");
        }
        hex_str
    }

    /// Build the ASCII string for a row.
    fn format_ascii(row_data: &[u8]) -> String {
        row_data
            .iter()
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                }
            })
            .collect()
    }

    /// Render the hex view for the given binary data with optional type overlays.
    pub fn render(&mut self, ui: &mut Ui, data: &[u8], project: &Project) {
        if data.is_empty() {
            ui.label("No data to display");
            return;
        }

        let mono_font = FontId::monospace(13.0);
        let total_rows = data.len().div_ceil(BYTES_PER_ROW);

        // Clamp offset to valid range
        if self.offset >= data.len() {
            self.offset = data.len().saturating_sub(1);
        }
        // Render header
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("  Offset   ")
                    .font(mono_font.clone())
                    .color(Color32::GRAY),
            );
            let mut header = String::new();
            for i in 0..BYTES_PER_ROW {
                if i == 8 {
                    header.push(' ');
                }
                header.push_str(&format!("{:02X} ", i));
            }
            header.push_str("  ASCII");
            ui.label(
                RichText::new(header)
                    .font(mono_font.clone())
                    .color(Color32::GRAY),
            );
        });
        ui.separator();

        // Virtual scrolling hex dump
        let row_height = ui.text_style_height(&egui::TextStyle::Monospace) + 2.0;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(ui, row_height, total_rows, |ui, row_range| {
                for row_idx in row_range {
                    let row_offset = row_idx * BYTES_PER_ROW;
                    if row_offset >= data.len() {
                        break;
                    }

                    let end = (row_offset + BYTES_PER_ROW).min(data.len());
                    let row_data = &data[row_offset..end];

                    let hex_str = Self::format_hex_bytes(row_data);
                    let ascii = Self::format_ascii(row_data);
                    let is_selected = self.selected_row == Some(row_offset);

                    // Check for applied type overlays at this row's offset
                    let applied_info = self.find_applied_type(
                        row_offset as u64,
                        BYTES_PER_ROW as u64,
                        project,
                    );

                    // Paint selection highlight behind the row
                    if is_selected {
                        let rect = ui.available_rect_before_wrap();
                        let row_rect = egui::Rect::from_min_size(
                            rect.min,
                            egui::vec2(rect.width(), row_height),
                        );
                        ui.painter()
                            .rect_filled(row_rect, 0.0, COLOR_SELECTED_BG);
                    }

                    let response = ui.horizontal(|ui| {
                        // Address column
                        ui.label(
                            RichText::new(format!("{:08X}  ", row_offset))
                                .font(mono_font.clone())
                                .color(COLOR_ADDRESS),
                        );

                        // Hex bytes
                        ui.label(
                            RichText::new(&hex_str)
                                .font(mono_font.clone())
                                .color(COLOR_HEX),
                        );

                        // ASCII column
                        ui.label(RichText::new(" ").font(mono_font.clone()));
                        ui.label(
                            RichText::new(&ascii)
                                .font(mono_font.clone())
                                .color(COLOR_ASCII),
                        );
                    });

                    // Track selection on click
                    if response.response.clicked() {
                        self.selected_row = Some(row_offset);
                        self.selected_byte = Some(row_offset);
                        self.selected_byte_count = 1;
                    }

                    // Right-click context menu
                    response.response.context_menu(|ui| {
                        if ui.button("Copy Address").clicked() {
                            ui.ctx().copy_text(format!("0x{:08X}", row_offset));
                            ui.close_menu();
                        }
                        if ui.button("Copy Hex Bytes").clicked() {
                            ui.ctx()
                                .copy_text(hex_str.trim_end().to_string());
                            ui.close_menu();
                        }
                        if ui.button("Copy ASCII").clicked() {
                            ui.ctx().copy_text(ascii.to_string());
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Go to Address").clicked() {
                            self.pending_navigation = Some(row_offset as u64);
                            ui.close_menu();
                        }
                        if ui.button("Apply Type...").clicked() {
                            self.pending_apply_type = Some(row_offset as u64);
                            ui.close_menu();
                        }
                        // Show option to remove applied type if one exists
                        if project.get_applied_type(row_offset as u64).is_some()
                            && ui.button("Remove Applied Type").clicked()
                        {
                            self.pending_apply_type = None;
                            ui.close_menu();
                        }
                    });

                    // Render type overlay below the row if applicable
                    if let Some((overlay_label, fields)) = applied_info {
                        ui.horizontal(|ui| {
                            ui.add_space(20.0);
                            ui.label(
                                RichText::new(format!("  ── {} ──", overlay_label))
                                    .font(mono_font.clone())
                                    .color(COLOR_TYPE_OVERLAY)
                                    .small(),
                            );
                        });
                        for (fname, fval) in &fields {
                            ui.horizontal(|ui| {
                                ui.add_space(30.0);
                                ui.label(
                                    RichText::new(fname)
                                        .font(mono_font.clone())
                                        .color(COLOR_FIELD_LABEL)
                                        .small(),
                                );
                                ui.label(
                                    RichText::new(fval)
                                        .font(mono_font.clone())
                                        .color(COLOR_HEX)
                                        .small(),
                                );
                            });
                        }
                    }
                }
            });

        // Data inspector panel: show details when a row is selected
        if let Some(sel_offset) = self.selected_byte {
            if sel_offset < data.len() {
                ui.separator();
                Self::render_data_inspector(ui, data, sel_offset, &mono_font);
            }
        }
    }

    /// Render a data inspector panel showing the selected byte in multiple formats.
    fn render_data_inspector(ui: &mut Ui, data: &[u8], offset: usize, mono_font: &FontId) {
        let remaining = data.len() - offset;
        let byte_val = data[offset];

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("Offset: 0x{:08X}", offset))
                    .font(mono_font.clone())
                    .color(COLOR_ADDRESS),
            );
            ui.separator();
            ui.label(
                RichText::new(format!("u8: {}", byte_val))
                    .font(mono_font.clone())
                    .color(COLOR_HEX),
            );
            ui.separator();
            ui.label(
                RichText::new(format!("i8: {}", byte_val as i8))
                    .font(mono_font.clone())
                    .color(COLOR_HEX),
            );
            ui.separator();
            if remaining >= 2 {
                let val = u16::from_le_bytes([data[offset], data[offset + 1]]);
                ui.label(
                    RichText::new(format!("u16 LE: {}", val))
                        .font(mono_font.clone())
                        .color(COLOR_HEX),
                );
                ui.separator();
            }
            if remaining >= 4 {
                let val = u32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                ui.label(
                    RichText::new(format!("u32 LE: {}", val))
                        .font(mono_font.clone())
                        .color(COLOR_HEX),
                );
                ui.separator();
            }
            let ch = if byte_val.is_ascii_graphic() || byte_val == b' ' {
                byte_val as char
            } else {
                '.'
            };
            ui.label(
                RichText::new(format!("ASCII: '{}'", ch))
                    .font(mono_font.clone())
                    .color(COLOR_ASCII),
            );
        });
    }

    /// Check if there's an applied type at the given offset and produce overlay info.
    /// Returns `Some((label, vec of (field_name, formatted_value)))` if applicable.
    fn find_applied_type(
        &self,
        row_offset: u64,
        _row_size: u64,
        project: &Project,
    ) -> Option<(String, Vec<(String, String)>)> {
        let applied = project.get_applied_type(row_offset)?;
        let def = project.get_type_def(&applied.type_name)?;

        let label = if let Some(ref lbl) = applied.label {
            format!("{}: {}", lbl, applied.type_name)
        } else {
            applied.type_name.clone()
        };

        let mut fields = Vec::new();
        match def {
            TypeDef::Primitive(p) => {
                fields.push((
                    applied.type_name.clone(),
                    format!("({} bytes)", p.size_bytes()),
                ));
            }
            TypeDef::Struct(s) => {
                let mut offset = 0usize;
                for field in &s.fields {
                    let fsize = project
                        .get_type_def(&field.type_name)
                        .and_then(|d| d.size_bytes(&project.type_definitions))
                        .unwrap_or(0);
                    fields.push((
                        format!("  .{}: {}", field.name, field.type_name),
                        format!("(+{:#x}, {} B)", offset, fsize),
                    ));
                    offset += fsize;
                }
            }
            TypeDef::Enum(e) => {
                for v in &e.variants {
                    fields.push((format!("  {} = {}", v.name, v.value), String::new()));
                }
            }
            TypeDef::Array {
                element_type_name,
                count,
            } => {
                let elem_size = project
                    .get_type_def(element_type_name)
                    .and_then(|d| d.size_bytes(&project.type_definitions))
                    .unwrap_or(0);
                fields.push((
                    format!("  {}[{}]", element_type_name, count),
                    format!("(total {} B)", elem_size * count),
                ));
            }
        }

        Some((label, fields))
    }
}
