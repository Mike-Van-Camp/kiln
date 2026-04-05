//! Strings view: extract and display printable ASCII strings from the binary.

use egui::{Color32, FontId, RichText, Ui};
use kiln_core::model::BinaryImage;

/// Minimum default length for extracted strings.
const DEFAULT_MIN_LENGTH: usize = 4;

/// A printable string extracted from binary data.
struct ExtractedString {
    file_offset: u64,
    virtual_address: u64,
    content: String,
    section_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum StringsSortColumn {
    #[default]
    Address,
    Length,
    Content,
}

/// State for the strings view panel.
pub struct StringsView {
    strings: Vec<ExtractedString>,
    filter: String,
    min_length: usize,
    cached: bool,
    sort_column: StringsSortColumn,
    sort_ascending: bool,
}

impl Default for StringsView {
    fn default() -> Self {
        Self {
            strings: Vec::new(),
            filter: String::new(),
            min_length: DEFAULT_MIN_LENGTH,
            cached: false,
            sort_column: StringsSortColumn::default(),
            sort_ascending: true,
        }
    }
}

impl StringsView {
    /// Return the number of extracted strings.
    pub fn string_count(&self) -> usize {
        self.strings.len()
    }

    /// Invalidate the cache so strings are re-extracted on next render.
    pub fn invalidate_cache(&mut self) {
        self.cached = false;
        self.strings.clear();
    }

    /// Extract printable ASCII strings from the binary data.
    fn extract_strings(&mut self, image: &BinaryImage) {
        self.strings.clear();
        let data = &image.data;
        let mut start = None;

        for (i, &byte) in data.iter().enumerate() {
            if (0x20..=0x7E).contains(&byte) {
                if start.is_none() {
                    start = Some(i);
                }
            } else {
                if let Some(s) = start {
                    let len = i - s;
                    if len >= self.min_length {
                        let content = String::from_utf8_lossy(&data[s..i]).into_owned();
                        let file_offset = s as u64;
                        let (virtual_address, section_name) = Self::map_offset(image, file_offset);
                        self.strings.push(ExtractedString {
                            file_offset,
                            virtual_address,
                            content,
                            section_name,
                        });
                    }
                }
                start = None;
            }
        }

        // Handle string at end of data
        if let Some(s) = start {
            let len = data.len() - s;
            if len >= self.min_length {
                let content = String::from_utf8_lossy(&data[s..]).into_owned();
                let file_offset = s as u64;
                let (virtual_address, section_name) = Self::map_offset(image, file_offset);
                self.strings.push(ExtractedString {
                    file_offset,
                    virtual_address,
                    content,
                    section_name,
                });
            }
        }

        self.cached = true;
    }

    /// Map a file offset to a virtual address and section name.
    fn map_offset(image: &BinaryImage, offset: u64) -> (u64, String) {
        for section in &image.sections {
            let sec_start = section.file_offset;
            let sec_end = sec_start + section.size;
            if offset >= sec_start && offset < sec_end {
                let va = section.address + (offset - sec_start);
                return (va, section.name.clone());
            }
        }
        (offset, String::new())
    }

    /// Render the strings view. Returns an optional address to navigate to.
    pub fn render(&mut self, ui: &mut Ui, image: &BinaryImage) -> Option<u64> {
        if !self.cached {
            self.extract_strings(image);
        }

        let mut navigate_to = None;

        // Filter bar and min-length control
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.text_edit_singleline(&mut self.filter);
            ui.separator();
            ui.label("Min length:");
            let old_min = self.min_length;
            ui.add(egui::DragValue::new(&mut self.min_length).range(1..=256));
            if self.min_length != old_min {
                self.cached = false;
            }
        });

        ui.separator();

        // Build filtered list of indices
        let filter_lower = self.filter.to_lowercase();
        let filtered: Vec<usize> = self
            .strings
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                filter_lower.is_empty() || s.content.to_lowercase().contains(&filter_lower)
            })
            .map(|(i, _)| i)
            .collect();

        ui.label(format!(
            "{} strings ({} shown)",
            self.strings.len(),
            filtered.len()
        ));
        ui.separator();

        // Sort filtered indices
        let mut filtered = filtered;
        {
            let strings = &self.strings;
            let col = self.sort_column;
            let asc = self.sort_ascending;
            filtered.sort_by(|&a, &b| {
                let cmp = match col {
                    StringsSortColumn::Address => {
                        strings[a].virtual_address.cmp(&strings[b].virtual_address)
                    }
                    StringsSortColumn::Length => strings[a]
                        .content
                        .len()
                        .cmp(&strings[b].content.len())
                        .then_with(|| strings[a].virtual_address.cmp(&strings[b].virtual_address)),
                    StringsSortColumn::Content => strings[a]
                        .content
                        .cmp(&strings[b].content)
                        .then_with(|| strings[a].virtual_address.cmp(&strings[b].virtual_address)),
                };
                if asc {
                    cmp
                } else {
                    cmp.reverse()
                }
            });
        }

        // Header
        let mono = FontId::monospace(13.0);
        let arrow_str = if self.sort_ascending { " ▲" } else { " ▼" };
        let addr_arrow = if self.sort_column == StringsSortColumn::Address {
            arrow_str
        } else {
            ""
        };
        let len_arrow = if self.sort_column == StringsSortColumn::Length {
            arrow_str
        } else {
            ""
        };
        let content_arrow = if self.sort_column == StringsSortColumn::Content {
            arrow_str
        } else {
            ""
        };
        ui.horizontal(|ui| {
            let addr_label = format!("Address     {}", addr_arrow);
            if ui
                .selectable_label(
                    self.sort_column == StringsSortColumn::Address,
                    RichText::new(addr_label)
                        .font(mono.clone())
                        .color(Color32::GRAY),
                )
                .clicked()
            {
                if self.sort_column == StringsSortColumn::Address {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.sort_column = StringsSortColumn::Address;
                    self.sort_ascending = true;
                }
            }
            ui.label(
                RichText::new("Offset       ")
                    .font(mono.clone())
                    .color(Color32::GRAY),
            );
            let len_label = format!("Len   {}", len_arrow);
            if ui
                .selectable_label(
                    self.sort_column == StringsSortColumn::Length,
                    RichText::new(len_label)
                        .font(mono.clone())
                        .color(Color32::GRAY),
                )
                .clicked()
            {
                if self.sort_column == StringsSortColumn::Length {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.sort_column = StringsSortColumn::Length;
                    self.sort_ascending = true;
                }
            }
            ui.label(
                RichText::new("Section          ")
                    .font(mono.clone())
                    .color(Color32::GRAY),
            );
            let str_label = format!("String{}", content_arrow);
            if ui
                .selectable_label(
                    self.sort_column == StringsSortColumn::Content,
                    RichText::new(str_label)
                        .font(mono.clone())
                        .color(Color32::GRAY),
                )
                .clicked()
            {
                if self.sort_column == StringsSortColumn::Content {
                    self.sort_ascending = !self.sort_ascending;
                } else {
                    self.sort_column = StringsSortColumn::Content;
                    self.sort_ascending = true;
                }
            }
        });
        ui.separator();

        // Virtual-scrolling table
        let row_height = ui.text_style_height(&egui::TextStyle::Monospace) + 2.0;
        let total_rows = filtered.len();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(ui, row_height, total_rows, |ui, row_range| {
                for row_idx in row_range {
                    let s = &self.strings[filtered[row_idx]];
                    let line = format!(
                        "0x{:08X}   0x{:08X}   {:<4}  {:<16} {}",
                        s.virtual_address,
                        s.file_offset,
                        s.content.len(),
                        s.section_name,
                        truncate_string(&s.content, 120),
                    );
                    let response =
                        ui.selectable_label(false, RichText::new(&line).font(mono.clone()));
                    if response.clicked() {
                        navigate_to = Some(s.virtual_address);
                    }
                    let va = s.virtual_address;
                    let offset = s.file_offset;
                    let content_clone = s.content.clone();
                    response.on_hover_text(&s.content).context_menu(|ui| {
                        if ui.button("Copy Address").clicked() {
                            ui.ctx().copy_text(format!("0x{:08X}", va));
                            ui.close_menu();
                        }
                        if ui.button("Copy String").clicked() {
                            ui.ctx().copy_text(content_clone.clone());
                            ui.close_menu();
                        }
                        if ui.button("Copy Offset").clicked() {
                            ui.ctx().copy_text(format!("0x{:08X}", offset));
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Go to Disassembly").clicked() {
                            navigate_to = Some(va);
                            ui.close_menu();
                        }
                    });
                }
            });

        navigate_to
    }
}

/// Truncate a string to a maximum display length, adding "..." if truncated.
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_core::model::*;

    fn make_image(data: Vec<u8>) -> BinaryImage {
        BinaryImage {
            filename: "test".to_string(),
            format: BinaryFormat::Elf,
            architecture: Architecture::X86_64,
            entry_point: 0x1000,
            bits: 64,
            is_little_endian: true,
            sections: vec![Section {
                name: ".text".to_string(),
                address: 0x1000,
                size: data.len() as u64,
                file_offset: 0,
                kind: SectionKind::Code,
                readable: true,
                writable: false,
                executable: true,
            }],
            segments: Vec::new(),
            symbols: Vec::new(),
            data,
        }
    }

    #[test]
    fn extract_strings_basic() {
        let mut data = Vec::new();
        data.extend_from_slice(b"Hello\0World\0");
        data.push(0x01); // non-printable separator
        data.extend_from_slice(b"ab\0"); // too short (< 4)
        data.extend_from_slice(b"Test1234\0");

        let image = make_image(data);
        let mut view = StringsView::default();
        view.extract_strings(&image);

        let contents: Vec<&str> = view.strings.iter().map(|s| s.content.as_str()).collect();
        assert!(contents.contains(&"Hello"));
        assert!(contents.contains(&"World"));
        assert!(contents.contains(&"Test1234"));
        // "ab" is only 2 chars, should not be included
        assert!(!contents.contains(&"ab"));
    }

    #[test]
    fn extract_strings_maps_va() {
        let data = b"\0\0\0\0Hello\0".to_vec();
        let mut image = make_image(data);
        image.sections[0].file_offset = 0;
        image.sections[0].address = 0x4000;

        let mut view = StringsView::default();
        view.extract_strings(&image);

        assert_eq!(view.strings.len(), 1);
        assert_eq!(view.strings[0].content, "Hello");
        assert_eq!(view.strings[0].file_offset, 4);
        assert_eq!(view.strings[0].virtual_address, 0x4004);
        assert_eq!(view.strings[0].section_name, ".text");
    }

    #[test]
    fn extract_strings_min_length() {
        let data = b"ab\0abcd\0".to_vec();
        let image = make_image(data);

        let mut view = StringsView {
            min_length: 4,
            ..StringsView::default()
        };
        view.extract_strings(&image);

        assert_eq!(view.strings.len(), 1);
        assert_eq!(view.strings[0].content, "abcd");
    }
}
