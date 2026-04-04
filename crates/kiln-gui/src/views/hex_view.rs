//! Hex view panel: virtual-scrolling hex dump (addr | hex bytes | ASCII).

use egui::{Color32, FontId, RichText, Ui};

/// Number of bytes displayed per row in the hex view.
const BYTES_PER_ROW: usize = 16;

/// Color constants for the hex view.
const COLOR_ADDRESS: Color32 = Color32::from_rgb(100, 149, 237);
const COLOR_HEX: Color32 = Color32::LIGHT_GRAY;
const COLOR_ASCII: Color32 = Color32::from_rgb(180, 180, 120);
const COLOR_SELECTED_BG: Color32 = Color32::from_rgb(50, 60, 80);

/// State for the hex view panel.
#[derive(Default)]
pub struct HexView {
    /// Current file offset displayed at the top of the view.
    pub offset: usize,
    /// Currently selected/clicked row offset.
    pub selected_row: Option<usize>,
    /// Pending navigation request from context menu.
    pending_navigation: Option<u64>,
}

impl HexView {
    /// Take the pending navigation address (if any), clearing it.
    pub fn take_pending_navigation(&mut self) -> Option<u64> {
        self.pending_navigation.take()
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

    /// Render the hex view for the given binary data.
    pub fn render(&mut self, ui: &mut Ui, data: &[u8]) {
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
                    });
                }
            });
    }
}
