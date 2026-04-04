//! Disassembly listing view: virtual-scrolling table with syntax coloring.

use egui::{Color32, FontId, RichText, Ui};
use kiln_core::analysis::AnalysisDatabase;
use kiln_core::model::{BinaryImage, Instruction};

/// State for the disassembly view panel.
#[derive(Default)]
pub struct DisasmView {
    /// If set, scroll to this address on the next frame.
    pub scroll_to_address: Option<u64>,
}

/// Color palette for syntax highlighting.
struct SyntaxColors;

impl SyntaxColors {
    const ADDRESS: Color32 = Color32::from_rgb(100, 149, 237); // Cornflower blue
    const BYTES: Color32 = Color32::from_rgb(128, 128, 128); // Gray
    const MNEMONIC_BRANCH: Color32 = Color32::from_rgb(220, 120, 120); // Red-ish for branches
    const MNEMONIC_CALL: Color32 = Color32::from_rgb(255, 180, 80); // Orange for calls
    const MNEMONIC_RET: Color32 = Color32::from_rgb(255, 100, 100); // Red for returns
    const MNEMONIC_MOV: Color32 = Color32::from_rgb(130, 200, 130); // Green for data movement
    const MNEMONIC_DEFAULT: Color32 = Color32::from_rgb(200, 200, 200); // Light gray default
    const REGISTER: Color32 = Color32::from_rgb(180, 140, 255); // Purple for registers
    const IMMEDIATE: Color32 = Color32::from_rgb(180, 220, 140); // Light green for immediates
    const SYMBOL_LABEL: Color32 = Color32::from_rgb(255, 220, 100); // Yellow for labels
    const NOP: Color32 = Color32::from_rgb(100, 100, 100); // Dark gray for nops
}

/// Classify a mnemonic for syntax coloring.
fn mnemonic_color(mnemonic: &str) -> Color32 {
    let m = mnemonic.to_lowercase();
    if m == "nop" || m == "fnop" {
        SyntaxColors::NOP
    } else if m == "ret" || m == "retn" || m == "retf" || m == "iret" || m == "sysret" {
        SyntaxColors::MNEMONIC_RET
    } else if m == "call" || m == "bl" || m == "blr" || m == "blx" || m.starts_with("call") {
        SyntaxColors::MNEMONIC_CALL
    } else if m.starts_with('j')
        || m.starts_with('b')
            && (m == "b"
                || m.starts_with("b.")
                || m.starts_with("br")
                || m.starts_with("bne")
                || m.starts_with("beq")
                || m.starts_with("blt")
                || m.starts_with("bgt")
                || m.starts_with("bge")
                || m.starts_with("ble")
                || m.starts_with("bx"))
        || m == "loop"
        || m == "loope"
        || m == "loopne"
    {
        SyntaxColors::MNEMONIC_BRANCH
    } else if m.starts_with("mov")
        || m.starts_with("lea")
        || m.starts_with("ldr")
        || m.starts_with("str")
        || m.starts_with("push")
        || m.starts_with("pop")
        || m == "xchg"
    {
        SyntaxColors::MNEMONIC_MOV
    } else {
        SyntaxColors::MNEMONIC_DEFAULT
    }
}

/// Colorize operand text. Returns a list of (text, color) segments.
fn colorize_operands(operands: &str) -> Vec<(String, Color32)> {
    if operands.is_empty() {
        return vec![];
    }

    let mut segments = Vec::new();
    let mut current = String::new();
    let mut chars = operands.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch == '0' {
            // Check for hex immediate like 0x...
            current.clear();
            current.push(ch);
            chars.next();
            if let Some(&next) = chars.peek() {
                if next == 'x' || next == 'X' {
                    current.push(next);
                    chars.next();
                    while let Some(&hc) = chars.peek() {
                        if hc.is_ascii_hexdigit() {
                            current.push(hc);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    segments.push((current.clone(), SyntaxColors::IMMEDIATE));
                    current.clear();
                    continue;
                }
            }
            // Not hex, just a digit
            while let Some(&dc) = chars.peek() {
                if dc.is_ascii_digit() {
                    current.push(dc);
                    chars.next();
                } else {
                    break;
                }
            }
            segments.push((current.clone(), SyntaxColors::IMMEDIATE));
            current.clear();
        } else if ch.is_ascii_digit() {
            // Decimal immediate
            current.clear();
            while let Some(&dc) = chars.peek() {
                if dc.is_ascii_hexdigit() || dc == 'x' || dc == 'X' {
                    current.push(dc);
                    chars.next();
                } else {
                    break;
                }
            }
            segments.push((current.clone(), SyntaxColors::IMMEDIATE));
            current.clear();
        } else if ch.is_ascii_alphabetic() {
            // Could be a register name
            current.clear();
            while let Some(&ac) = chars.peek() {
                if ac.is_ascii_alphanumeric() || ac == '_' {
                    current.push(ac);
                    chars.next();
                } else {
                    break;
                }
            }
            // Check if it looks like a register
            let is_register = is_register_name(&current);
            if is_register {
                segments.push((current.clone(), SyntaxColors::REGISTER));
            } else {
                segments.push((current.clone(), SyntaxColors::MNEMONIC_DEFAULT));
            }
            current.clear();
        } else {
            // Punctuation/operators: [, ], +, -, *, :, etc.
            segments.push((ch.to_string(), SyntaxColors::MNEMONIC_DEFAULT));
            chars.next();
        }
    }

    segments
}

/// Check if a token looks like a CPU register name.
fn is_register_name(name: &str) -> bool {
    let n = name.to_lowercase();
    // x86 registers
    if matches!(
        n.as_str(),
        "eax"
            | "ebx"
            | "ecx"
            | "edx"
            | "esi"
            | "edi"
            | "ebp"
            | "esp"
            | "rax"
            | "rbx"
            | "rcx"
            | "rdx"
            | "rsi"
            | "rdi"
            | "rbp"
            | "rsp"
            | "r8"
            | "r9"
            | "r10"
            | "r11"
            | "r12"
            | "r13"
            | "r14"
            | "r15"
            | "r8d"
            | "r9d"
            | "r10d"
            | "r11d"
            | "r12d"
            | "r13d"
            | "r14d"
            | "r15d"
            | "r8w"
            | "r9w"
            | "r10w"
            | "r11w"
            | "r12w"
            | "r13w"
            | "r14w"
            | "r15w"
            | "r8b"
            | "r9b"
            | "r10b"
            | "r11b"
            | "r12b"
            | "r13b"
            | "r14b"
            | "r15b"
            | "al"
            | "ah"
            | "bl"
            | "bh"
            | "cl"
            | "ch"
            | "dl"
            | "dh"
            | "ax"
            | "bx"
            | "cx"
            | "dx"
            | "si"
            | "di"
            | "bp"
            | "sp"
            | "sil"
            | "dil"
            | "bpl"
            | "spl"
            | "rip"
            | "eip"
            | "ip"
            | "cs"
            | "ds"
            | "es"
            | "fs"
            | "gs"
            | "ss"
            | "rflags"
            | "eflags"
            | "flags"
            | "cr0"
            | "cr2"
            | "cr3"
            | "cr4"
            | "cr8"
            | "xmm0"
            | "xmm1"
            | "xmm2"
            | "xmm3"
            | "xmm4"
            | "xmm5"
            | "xmm6"
            | "xmm7"
            | "xmm8"
            | "xmm9"
            | "xmm10"
            | "xmm11"
            | "xmm12"
            | "xmm13"
            | "xmm14"
            | "xmm15"
            | "ymm0"
            | "ymm1"
            | "ymm2"
            | "ymm3"
            | "ymm4"
            | "ymm5"
            | "ymm6"
            | "ymm7"
    ) {
        return true;
    }
    // ARM registers
    if n.starts_with('x')
        || n.starts_with('w')
        || n.starts_with('q')
        || n.starts_with('d')
        || n.starts_with('s')
    {
        if let Some(rest) = n.get(1..) {
            if rest.parse::<u32>().is_ok() {
                return true;
            }
        }
    }
    if matches!(
        n.as_str(),
        "sp" | "lr" | "pc" | "fp" | "cpsr" | "spsr" | "wzr" | "xzr"
    ) {
        return true;
    }
    // RISC-V registers
    if matches!(
        n.as_str(),
        "zero"
            | "ra"
            | "sp"
            | "gp"
            | "tp"
            | "fp"
            | "t0"
            | "t1"
            | "t2"
            | "t3"
            | "t4"
            | "t5"
            | "t6"
            | "s0"
            | "s1"
            | "s2"
            | "s3"
            | "s4"
            | "s5"
            | "s6"
            | "s7"
            | "s8"
            | "s9"
            | "s10"
            | "s11"
            | "a0"
            | "a1"
            | "a2"
            | "a3"
            | "a4"
            | "a5"
            | "a6"
            | "a7"
    ) {
        return true;
    }
    false
}

impl DisasmView {
    /// Render the disassembly listing view.
    pub fn render(
        &mut self,
        ui: &mut Ui,
        analysis: &AnalysisDatabase,
        image: Option<&BinaryImage>,
    ) {
        let mono_font = FontId::monospace(13.0);

        if analysis.instructions.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("No disassembly available");
            });
            return;
        }

        // Collect instruction addresses for indexing
        let addresses: Vec<u64> = analysis.instructions.keys().copied().collect();
        let total_rows = addresses.len();

        // Render header
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "{:<12} {:<24} {:<10} {}",
                    "Address", "Bytes", "Mnemonic", "Operands"
                ))
                .font(mono_font.clone())
                .color(Color32::GRAY),
            );
        });
        ui.separator();

        let row_height = ui.text_style_height(&egui::TextStyle::Monospace) + 2.0;

        // Handle scroll-to-address
        let mut scroll_to_row = None;
        if let Some(target_addr) = self.scroll_to_address.take() {
            // Binary search for the closest address
            match addresses.binary_search(&target_addr) {
                Ok(idx) => scroll_to_row = Some(idx),
                Err(idx) => {
                    if idx < addresses.len() {
                        scroll_to_row = Some(idx);
                    }
                }
            }
        }

        let mut scroll_area = egui::ScrollArea::vertical().auto_shrink([false, false]);

        if let Some(row) = scroll_to_row {
            let offset = row as f32 * row_height;
            scroll_area = scroll_area.vertical_scroll_offset(offset);
        }

        scroll_area.show_rows(ui, row_height, total_rows, |ui, row_range| {
            for row_idx in row_range {
                if row_idx >= addresses.len() {
                    break;
                }
                let addr = addresses[row_idx];
                if let Some(insn) = analysis.get_instruction(addr) {
                    self.render_instruction_row(ui, insn, &mono_font, analysis, image);
                }
            }
        });
    }

    /// Render a single instruction row with syntax coloring.
    fn render_instruction_row(
        &self,
        ui: &mut Ui,
        insn: &Instruction,
        font: &FontId,
        analysis: &AnalysisDatabase,
        image: Option<&BinaryImage>,
    ) {
        // Check for symbol label at this address
        if let Some(image) = image {
            if let Some(name) = analysis.symbol_at_address(image, insn.address) {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{}:", name))
                            .font(font.clone())
                            .color(SyntaxColors::SYMBOL_LABEL),
                    );
                });
            }
        }

        ui.horizontal(|ui| {
            // Address
            ui.label(
                RichText::new(format!("0x{:08X}  ", insn.address))
                    .font(font.clone())
                    .color(SyntaxColors::ADDRESS),
            );

            // Bytes (max 8 bytes displayed, truncated with ..)
            let bytes_str = if insn.bytes.len() <= 8 {
                let hex: String = insn.bytes.iter().map(|b| format!("{:02X} ", b)).collect();
                format!("{:<24}", hex.trim_end())
            } else {
                let hex: String = insn.bytes[..8]
                    .iter()
                    .map(|b| format!("{:02X} ", b))
                    .collect();
                format!("{:<22}..", hex.trim_end())
            };
            ui.label(
                RichText::new(&bytes_str)
                    .font(font.clone())
                    .color(SyntaxColors::BYTES),
            );

            // Mnemonic with syntax coloring
            let mnem_color = mnemonic_color(&insn.mnemonic);
            ui.label(
                RichText::new(format!("{:<10}", &insn.mnemonic))
                    .font(font.clone())
                    .color(mnem_color),
            );

            // Operands with syntax coloring
            let operand_segments = colorize_operands(&insn.operands);
            for (text, color) in operand_segments {
                ui.label(RichText::new(&text).font(font.clone()).color(color));
            }
        });
    }
}
