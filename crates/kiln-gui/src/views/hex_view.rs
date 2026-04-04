//! Hex view panel: virtual-scrolling hex dump (addr | hex bytes | ASCII).

use egui::{Color32, FontId, RichText, Ui};

/// Number of bytes displayed per row in the hex view.
const BYTES_PER_ROW: usize = 16;

/// State for the hex view panel.
#[derive(Default)]
pub struct HexView {
    /// Current file offset displayed at the top of the view.
    pub offset: usize,
}

impl HexView {
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

                    ui.horizontal(|ui| {
                        // Address column
                        ui.label(
                            RichText::new(format!("{:08X}  ", row_offset))
                                .font(mono_font.clone())
                                .color(Color32::from_rgb(100, 149, 237)),
                        );

                        // Hex bytes
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

                        ui.label(
                            RichText::new(&hex_str)
                                .font(mono_font.clone())
                                .color(Color32::LIGHT_GRAY),
                        );

                        // ASCII column
                        ui.label(RichText::new(" ").font(mono_font.clone()));
                        let ascii: String = row_data
                            .iter()
                            .map(|&b| {
                                if b.is_ascii_graphic() || b == b' ' {
                                    b as char
                                } else {
                                    '.'
                                }
                            })
                            .collect();
                        ui.label(
                            RichText::new(ascii)
                                .font(mono_font.clone())
                                .color(Color32::from_rgb(180, 180, 120)),
                        );
                    });
                }
            });

        // Allow scrolling to a specific row when offset changes
        // The ScrollArea handles virtual scrolling automatically
    }
}
