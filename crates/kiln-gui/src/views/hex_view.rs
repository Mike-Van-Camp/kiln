//! Hex view panel: virtual-scrolling hex dump (addr | hex bytes | ASCII).

use std::collections::BTreeSet;

use egui::{Color32, FontId, RichText, Ui};
use kiln_core::analysis::AnalysisDatabase;
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
const COLOR_XREF_BG: Color32 = Color32::from_rgb(60, 40, 50);
const COLOR_SEARCH_HIT_BG: Color32 = Color32::from_rgb(80, 70, 30);

/// State for the hex view panel.
#[derive(Default)]
pub struct HexView {
    /// Current file offset displayed at the top of the view.
    pub offset: usize,
    /// Start of byte-range selection (anchor).
    pub selection_anchor: Option<usize>,
    /// End of byte-range selection (extends as user shift-clicks/drags).
    pub selection_end: Option<usize>,
    /// Currently selected/clicked row offset (kept for backward compat).
    pub selected_row: Option<usize>,
    /// Currently selected byte offset (finer granularity than `selected_row`).
    pub selected_byte: Option<usize>,
    /// Number of bytes in the current selection.
    pub selected_byte_count: usize,
    /// Pending navigation request from context menu.
    pending_navigation: Option<u64>,
    /// Pending apply-type request from context menu.
    pending_apply_type: Option<u64>,
    /// Inline edit mode: byte offset being edited.
    editing_byte: Option<usize>,
    /// Inline edit buffer (accumulates nibbles).
    edit_nibble_buf: Option<u8>,
    /// Pending byte edits: (offset, new_value).
    pub pending_edits: Vec<(usize, u8)>,
    /// Cached set of data xref target addresses for highlight.
    xref_targets: BTreeSet<u64>,
    /// Search hit offsets to highlight inline.
    pub search_hits: Vec<usize>,
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

    /// Take pending byte edits, clearing the buffer.
    pub fn take_pending_edits(&mut self) -> Vec<(usize, u8)> {
        std::mem::take(&mut self.pending_edits)
    }

    /// Returns the selected byte range as (start, count). Empty if no selection.
    fn selection_range(&self) -> Option<(usize, usize)> {
        match (self.selection_anchor, self.selection_end) {
            (Some(a), Some(b)) => {
                let lo = a.min(b);
                let hi = a.max(b);
                Some((lo, hi - lo + 1))
            }
            _ => self.selected_byte.map(|b| (b, self.selected_byte_count.max(1))),
        }
    }

    /// Update the xref target cache from the analysis database.
    pub fn update_xref_targets(&mut self, analysis: &AnalysisDatabase) {
        self.xref_targets.clear();
        for xref in &analysis.xrefs {
            if xref.xref_type == kiln_core::model::XrefType::Data {
                self.xref_targets.insert(xref.to_addr);
            }
        }
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
            ui.centered_and_justified(|ui| {
                ui.label("No data to display. Open a binary file (File → Open or Ctrl+O).");
            });
            return;
        }

        let mono_font = FontId::monospace(13.0);
        let total_rows = data.len().div_ceil(BYTES_PER_ROW);

        // Clamp offset to valid range
        if self.offset >= data.len() {
            self.offset = data.len().saturating_sub(1);
        }

        // Handle inline editing keyboard input
        if self.editing_byte.is_some() {
            self.handle_edit_input(ui, data);
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

        let sel_range = self.selection_range();

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

                    // Determine if any byte in this row is in the selection range
                    let row_selected = sel_range.is_some_and(|(sel_start, sel_count)| {
                        let sel_end = sel_start + sel_count;
                        row_offset < sel_end && end > sel_start
                    });

                    // Check for xref targets in this row
                    let has_xref = (row_offset..end)
                        .any(|off| self.xref_targets.contains(&(off as u64)));

                    // Check for search hits in this row
                    let has_search_hit = self
                        .search_hits
                        .iter()
                        .any(|&hit| hit >= row_offset && hit < end);

                    // Check for applied type overlays at this row's offset
                    let applied_info =
                        self.find_applied_type(row_offset as u64, BYTES_PER_ROW as u64, project);

                    // Paint background highlights
                    let rect = ui.available_rect_before_wrap();
                    let row_rect = egui::Rect::from_min_size(
                        rect.min,
                        egui::vec2(rect.width(), row_height),
                    );
                    if row_selected {
                        ui.painter().rect_filled(row_rect, 0.0, COLOR_SELECTED_BG);
                    } else if has_search_hit {
                        ui.painter()
                            .rect_filled(row_rect, 0.0, COLOR_SEARCH_HIT_BG);
                    } else if has_xref {
                        ui.painter().rect_filled(row_rect, 0.0, COLOR_XREF_BG);
                    }

                    // Check if we're editing a byte in this row
                    let editing_in_row = self
                        .editing_byte
                        .is_some_and(|eb| eb >= row_offset && eb < end);

                    let response = ui.horizontal(|ui| {
                        // Address column
                        ui.label(
                            RichText::new(format!("{:08X}  ", row_offset))
                                .font(mono_font.clone())
                                .color(COLOR_ADDRESS),
                        );

                        if editing_in_row {
                            // Show hex bytes with the editing byte highlighted
                            let edit_offset = self.editing_byte.unwrap();
                            let edit_idx = edit_offset - row_offset;
                            let display =
                                Self::format_hex_bytes_with_edit(row_data, edit_idx, self.edit_nibble_buf);
                            ui.label(
                                RichText::new(display)
                                    .font(mono_font.clone())
                                    .color(Color32::from_rgb(255, 200, 100)),
                            );
                        } else {
                            // Hex bytes (colorize xref targets within the row)
                            if has_xref {
                                let colored = Self::format_hex_bytes_colored(
                                    row_data,
                                    row_offset,
                                    &self.xref_targets,
                                );
                                for (text, color) in &colored {
                                    ui.label(
                                        RichText::new(text)
                                            .font(mono_font.clone())
                                            .color(*color),
                                    );
                                }
                            } else {
                                ui.label(
                                    RichText::new(&hex_str)
                                        .font(mono_font.clone())
                                        .color(COLOR_HEX),
                                );
                            }
                        }

                        // ASCII column
                        ui.label(RichText::new(" ").font(mono_font.clone()));
                        ui.label(
                            RichText::new(&ascii)
                                .font(mono_font.clone())
                                .color(COLOR_ASCII),
                        );
                    });

                    // Track selection on click (with shift for range selection)
                    if response.response.clicked() {
                        let shift = ui.input(|i| i.modifiers.shift);
                        if shift {
                            // Extend selection from anchor to this row
                            if self.selection_anchor.is_some() {
                                self.selection_end = Some(row_offset);
                                let (lo, count) = self.selection_range().unwrap_or((row_offset, 1));
                                self.selected_byte = Some(lo);
                                self.selected_byte_count = count;
                                self.selected_row = Some(row_offset);
                            }
                        } else {
                            // New single selection
                            self.selection_anchor = Some(row_offset);
                            self.selection_end = Some(row_offset);
                            self.selected_row = Some(row_offset);
                            self.selected_byte = Some(row_offset);
                            self.selected_byte_count = 1;
                            self.editing_byte = None;
                            self.edit_nibble_buf = None;
                        }
                    }

                    // Shift+drag extends selection
                    if response.response.dragged() && self.selection_anchor.is_some() {
                        self.selection_end = Some(row_offset);
                        let (lo, count) = self.selection_range().unwrap_or((row_offset, 1));
                        self.selected_byte = Some(lo);
                        self.selected_byte_count = count;
                        self.selected_row = Some(row_offset);
                    }

                    // Right-click context menu
                    response.response.context_menu(|ui| {
                        if ui.button("Copy Address").clicked() {
                            ui.ctx().copy_text(format!("0x{:08X}", row_offset));
                            ui.close_menu();
                        }
                        if ui.button("Copy Hex Bytes").clicked() {
                            ui.ctx().copy_text(hex_str.trim_end().to_string());
                            ui.close_menu();
                        }
                        if ui.button("Copy ASCII").clicked() {
                            ui.ctx().copy_text(ascii.to_string());
                            ui.close_menu();
                        }
                        ui.separator();

                        // Extended copy formats for selections
                        if let Some((sel_start, sel_count)) = self.selection_range() {
                            let sel_end = (sel_start + sel_count).min(data.len());
                            let sel_data = &data[sel_start..sel_end];

                            if ui.button("Copy as Hex String").clicked() {
                                let hex: String = sel_data
                                    .iter()
                                    .map(|b| format!("{:02X}", b))
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                ui.ctx().copy_text(hex);
                                ui.close_menu();
                            }
                            if ui.button("Copy as C Array").clicked() {
                                let items: String = sel_data
                                    .iter()
                                    .map(|b| format!("0x{:02X}", b))
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                ui.ctx()
                                    .copy_text(format!("unsigned char data[] = {{ {} }};", items));
                                ui.close_menu();
                            }
                            if ui.button("Copy as Python Bytes").clicked() {
                                let items: String = sel_data
                                    .iter()
                                    .map(|b| format!("\\x{:02x}", b))
                                    .collect::<Vec<_>>()
                                    .join("");
                                ui.ctx().copy_text(format!("b\"{}\"", items));
                                ui.close_menu();
                            }
                            if ui.button("Copy as Raw Bytes").clicked() {
                                let raw: String = sel_data
                                    .iter()
                                    .map(|b| format!("{:02x}", b))
                                    .collect();
                                ui.ctx().copy_text(raw);
                                ui.close_menu();
                            }
                            ui.separator();
                        }

                        if ui.button("Edit Byte").clicked() {
                            self.editing_byte = Some(row_offset);
                            self.edit_nibble_buf = None;
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

        // Data inspector panel: show details when bytes are selected
        if let Some((sel_start, sel_count)) = self.selection_range() {
            if sel_start < data.len() {
                ui.separator();
                Self::render_data_inspector(ui, data, sel_start, sel_count, &mono_font);
            }
        }
    }

    /// Render a data inspector panel showing the selected bytes in multiple formats.
    fn render_data_inspector(
        ui: &mut Ui,
        data: &[u8],
        offset: usize,
        sel_count: usize,
        mono_font: &FontId,
    ) {
        let remaining = data.len() - offset;
        let byte_val = data[offset];

        ui.label(
            RichText::new(format!(
                "Data Inspector — 0x{:08X} ({} byte{} selected)",
                offset,
                sel_count,
                if sel_count == 1 { "" } else { "s" }
            ))
            .font(mono_font.clone())
            .color(COLOR_ADDRESS),
        );

        ui.horizontal_wrapped(|ui| {
            // u8 / i8
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

            // u16 / i16
            if remaining >= 2 {
                let le = u16::from_le_bytes([data[offset], data[offset + 1]]);
                let be = u16::from_be_bytes([data[offset], data[offset + 1]]);
                ui.label(
                    RichText::new(format!("u16 LE: {}  BE: {}", le, be))
                        .font(mono_font.clone())
                        .color(COLOR_HEX),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!("i16 LE: {}  BE: {}", le as i16, be as i16))
                        .font(mono_font.clone())
                        .color(COLOR_HEX),
                );
                ui.separator();
            }

            // u32 / i32 / f32
            if remaining >= 4 {
                let le = u32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                let be = u32::from_be_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                ui.label(
                    RichText::new(format!("u32 LE: {}  BE: {}", le, be))
                        .font(mono_font.clone())
                        .color(COLOR_HEX),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!("i32 LE: {}  BE: {}", le as i32, be as i32))
                        .font(mono_font.clone())
                        .color(COLOR_HEX),
                );
                ui.separator();
                let f_le = f32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                let f_be = f32::from_be_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                ui.label(
                    RichText::new(format!("f32 LE: {:.6}  BE: {:.6}", f_le, f_be))
                        .font(mono_font.clone())
                        .color(COLOR_HEX),
                );
                ui.separator();
            }

            // u64 / i64 / f64
            if remaining >= 8 {
                let mut le_bytes = [0u8; 8];
                let mut be_bytes = [0u8; 8];
                le_bytes.copy_from_slice(&data[offset..offset + 8]);
                be_bytes.copy_from_slice(&data[offset..offset + 8]);
                let le64 = u64::from_le_bytes(le_bytes);
                let be64 = u64::from_be_bytes(be_bytes);
                ui.label(
                    RichText::new(format!("u64 LE: {}  BE: {}", le64, be64))
                        .font(mono_font.clone())
                        .color(COLOR_HEX),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!("i64 LE: {}  BE: {}", le64 as i64, be64 as i64))
                        .font(mono_font.clone())
                        .color(COLOR_HEX),
                );
                ui.separator();
                let f_le = f64::from_le_bytes(le_bytes);
                let f_be = f64::from_be_bytes(be_bytes);
                ui.label(
                    RichText::new(format!("f64 LE: {:.6}  BE: {:.6}", f_le, f_be))
                        .font(mono_font.clone())
                        .color(COLOR_HEX),
                );
                ui.separator();
            }

            // ASCII char
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

    /// Handle keyboard input for inline byte editing.
    fn handle_edit_input(&mut self, ui: &mut Ui, data: &[u8]) {
        let edit_offset = match self.editing_byte {
            Some(o) if o < data.len() => o,
            _ => {
                self.editing_byte = None;
                return;
            }
        };

        // Read keys pressed this frame
        let events: Vec<egui::Event> = ui.input(|i| i.events.clone());
        for event in &events {
            if let egui::Event::Key {
                key: egui::Key::Escape,
                pressed: true,
                ..
            } = event
            {
                self.editing_byte = None;
                self.edit_nibble_buf = None;
                return;
            }
            if let egui::Event::Text(text) = event {
                for ch in text.chars() {
                    if let Some(nibble) = ch.to_digit(16) {
                        let nibble = nibble as u8;
                        if let Some(high) = self.edit_nibble_buf.take() {
                            // Second nibble: commit the byte
                            let new_val = (high << 4) | nibble;
                            self.pending_edits.push((edit_offset, new_val));
                            // Move to next byte
                            if edit_offset + 1 < data.len() {
                                self.editing_byte = Some(edit_offset + 1);
                            } else {
                                self.editing_byte = None;
                            }
                        } else {
                            // First nibble
                            self.edit_nibble_buf = Some(nibble);
                        }
                    }
                }
            }
        }
    }

    /// Format hex bytes with the editing byte highlighted.
    fn format_hex_bytes_with_edit(row_data: &[u8], edit_idx: usize, nibble_buf: Option<u8>) -> String {
        let mut hex_str = String::with_capacity(BYTES_PER_ROW * 3 + 4);
        for (i, byte) in row_data.iter().enumerate() {
            if i == 8 {
                hex_str.push(' ');
            }
            if i == edit_idx {
                if let Some(high) = nibble_buf {
                    hex_str.push_str(&format!("{:X}_ ", high));
                } else {
                    hex_str.push_str(&format!("[{:02X}]", byte));
                }
            } else {
                hex_str.push_str(&format!("{:02X} ", byte));
            }
        }
        for i in row_data.len()..BYTES_PER_ROW {
            if i == 8 {
                hex_str.push(' ');
            }
            hex_str.push_str("   ");
        }
        hex_str
    }

    /// Format hex bytes with xref target bytes colored differently.
    fn format_hex_bytes_colored(
        row_data: &[u8],
        row_offset: usize,
        xref_targets: &BTreeSet<u64>,
    ) -> Vec<(String, Color32)> {
        let mut segments: Vec<(String, Color32)> = Vec::new();
        let mut current_text = String::new();
        let mut current_is_xref = false;

        for (i, byte) in row_data.iter().enumerate() {
            let is_xref = xref_targets.contains(&((row_offset + i) as u64));
            if i == 0 {
                current_is_xref = is_xref;
            }

            if is_xref != current_is_xref {
                // Flush current segment
                let color = if current_is_xref {
                    Color32::from_rgb(255, 140, 140)
                } else {
                    COLOR_HEX
                };
                segments.push((std::mem::take(&mut current_text), color));
                current_is_xref = is_xref;
            }

            if i == 8 {
                current_text.push(' ');
            }
            current_text.push_str(&format!("{:02X} ", byte));
        }
        // Pad if row is incomplete
        for i in row_data.len()..BYTES_PER_ROW {
            if i == 8 {
                current_text.push(' ');
            }
            current_text.push_str("   ");
        }
        // Flush last segment
        let color = if current_is_xref {
            Color32::from_rgb(255, 140, 140)
        } else {
            COLOR_HEX
        };
        segments.push((current_text, color));

        segments
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
