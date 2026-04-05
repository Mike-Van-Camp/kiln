//! Debugger view panel with breakpoint management, register display, call stack,
//! and memory watch (Sprint 18).

use egui::{Color32, RichText, Ui};
use kiln_core::debugger::{DebugSession, DebuggerState};

/// Color constants for debugger state indicators.
const COLOR_DISCONNECTED: Color32 = Color32::from_rgb(200, 60, 60);
const COLOR_RUNNING: Color32 = Color32::from_rgb(60, 200, 60);
const COLOR_PAUSED: Color32 = Color32::from_rgb(220, 200, 50);
const COLOR_EXITED: Color32 = Color32::from_rgb(150, 150, 150);

/// State for the debugger view.
#[derive(Default)]
pub struct DebuggerView {
    /// The debug session holding all debugger state.
    pub session: DebugSession,
    /// Input field for adding a breakpoint address.
    pub bp_address_input: String,
    /// Input field for watch address.
    pub watch_address: String,
    /// Input field for watch size.
    pub watch_size: String,
    /// Input field for watch label.
    pub watch_label: String,
    /// Pending navigation request (address to jump to in disassembly).
    pub pending_navigation: Option<u64>,
}

impl DebuggerView {
    /// Render the full debugger panel.
    pub fn render(&mut self, ui: &mut Ui) {
        self.render_state_bar(ui);
        ui.separator();
        self.render_toolbar(ui);
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            self.render_breakpoints_section(ui);
            ui.add_space(4.0);
            self.render_registers_section(ui);
            ui.add_space(4.0);
            self.render_call_stack_section(ui);
            ui.add_space(4.0);
            self.render_memory_watch_section(ui);
            ui.add_space(4.0);
            self.render_output_log(ui);
        });
    }

    fn render_state_bar(&self, ui: &mut Ui) {
        let (color, label) = match self.session.state {
            DebuggerState::Disconnected => (COLOR_DISCONNECTED, "⏻ Disconnected"),
            DebuggerState::Running => (COLOR_RUNNING, "▶ Running"),
            DebuggerState::Paused => (COLOR_PAUSED, "⏸ Paused"),
            DebuggerState::Exited => (COLOR_EXITED, "⏹ Exited"),
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).color(color).strong().size(16.0));
            if let Some(addr) = self.session.current_address {
                ui.separator();
                ui.monospace(format!("PC: 0x{addr:016x}"));
            }
        });
    }

    fn render_toolbar(&mut self, ui: &mut Ui) {
        let is_paused = self.session.state == DebuggerState::Paused;
        let is_disconnected = self.session.state == DebuggerState::Disconnected;
        let is_running = self.session.state == DebuggerState::Running;

        ui.horizontal(|ui| {
            let connect_label = if is_disconnected {
                "🔌 Connect"
            } else {
                "🔌 Disconnect"
            };
            if ui.button(connect_label).clicked() {
                if is_disconnected {
                    self.session.state = DebuggerState::Paused;
                    self.session
                        .output_log
                        .push("Debugger connected (stub)".into());
                } else {
                    self.session.state = DebuggerState::Disconnected;
                    self.session.output_log.push("Debugger disconnected".into());
                }
            }

            ui.separator();

            let can_continue = is_paused;
            if ui
                .add_enabled(can_continue, egui::Button::new("▶ Continue"))
                .clicked()
            {
                self.session.state = DebuggerState::Running;
                self.session.output_log.push("Continuing execution".into());
            }

            if ui
                .add_enabled(can_continue, egui::Button::new("⤵ Step Over"))
                .clicked()
            {
                self.session.output_log.push("Step over (stub)".into());
            }

            if ui
                .add_enabled(can_continue, egui::Button::new("↓ Step Into"))
                .clicked()
            {
                self.session.output_log.push("Step into (stub)".into());
            }

            if ui
                .add_enabled(is_running || is_paused, egui::Button::new("⏹ Stop"))
                .clicked()
            {
                self.session.state = DebuggerState::Exited;
                self.session.output_log.push("Execution stopped".into());
            }

            if ui
                .add_enabled(is_running, egui::Button::new("⏸ Break"))
                .clicked()
            {
                self.session.state = DebuggerState::Paused;
                self.session.output_log.push("Execution paused".into());
            }
        });
    }

    fn render_breakpoints_section(&mut self, ui: &mut Ui) {
        egui::CollapsingHeader::new(
            RichText::new(format!("Breakpoints ({})", self.session.breakpoints.len())).strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            // Add breakpoint row
            ui.horizontal(|ui| {
                ui.label("Address:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.bp_address_input)
                        .desired_width(120.0)
                        .font(egui::TextStyle::Monospace)
                        .hint_text("0x..."),
                );
                if ui.button("Add").clicked() && !self.bp_address_input.trim().is_empty() {
                    let input = self.bp_address_input.trim().trim_start_matches("0x");
                    if let Ok(addr) = u64::from_str_radix(input, 16) {
                        self.session.add_breakpoint(addr);
                        self.bp_address_input.clear();
                    } else {
                        self.session
                            .output_log
                            .push(format!("Invalid address: {}", self.bp_address_input));
                    }
                }
                if ui.button("Clear All").clicked() {
                    self.session.clear_breakpoints();
                }
            });

            if self.session.breakpoints.is_empty() {
                ui.colored_label(Color32::GRAY, "No breakpoints set");
            } else {
                let mut toggle_id = None;
                let mut remove_id = None;
                let mut nav_addr = None;

                for bp in &self.session.breakpoints {
                    ui.horizontal(|ui| {
                        let enabled_text = if bp.enabled { "✓" } else { "○" };
                        if ui.button(enabled_text).clicked() {
                            toggle_id = Some(bp.id);
                        }

                        let addr_text = format!("0x{:x}", bp.address);
                        let label = if bp.enabled {
                            RichText::new(&addr_text).monospace()
                        } else {
                            RichText::new(&addr_text)
                                .monospace()
                                .strikethrough()
                                .color(Color32::GRAY)
                        };
                        if ui.link(label).clicked() {
                            nav_addr = Some(bp.address);
                        }

                        if bp.hit_count > 0 {
                            ui.colored_label(
                                Color32::LIGHT_GRAY,
                                format!("hits: {}", bp.hit_count),
                            );
                        }

                        if let Some(cond) = &bp.condition {
                            ui.colored_label(Color32::LIGHT_BLUE, format!("if {cond}"));
                        }

                        if ui.small_button("✕").clicked() {
                            remove_id = Some(bp.id);
                        }
                    });
                }

                if let Some(id) = toggle_id {
                    self.session.toggle_breakpoint(id);
                }
                if let Some(id) = remove_id {
                    self.session.remove_breakpoint(id);
                }
                if let Some(addr) = nav_addr {
                    self.pending_navigation = Some(addr);
                }
            }
        });
    }

    fn render_registers_section(&self, ui: &mut Ui) {
        egui::CollapsingHeader::new(
            RichText::new(format!("Registers ({})", self.session.registers.len())).strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            if self.session.registers.is_empty() {
                ui.colored_label(Color32::GRAY, "No register data available");
            } else {
                egui::Grid::new("register_grid")
                    .striped(true)
                    .min_col_width(80.0)
                    .show(ui, |ui| {
                        ui.label(RichText::new("Register").strong());
                        ui.label(RichText::new("Value").strong());
                        ui.end_row();

                        for reg in &self.session.registers {
                            ui.monospace(&reg.name);
                            let fmt = match reg.size {
                                4 => format!("0x{:08x}", reg.value as u32),
                                _ => format!("0x{:016x}", reg.value),
                            };
                            ui.monospace(fmt);
                            ui.end_row();
                        }
                    });
            }
        });
    }

    fn render_call_stack_section(&mut self, ui: &mut Ui) {
        egui::CollapsingHeader::new(
            RichText::new(format!("Call Stack ({})", self.session.call_stack.len())).strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            if self.session.call_stack.is_empty() {
                ui.colored_label(Color32::GRAY, "No call stack available");
            } else {
                for frame in &self.session.call_stack {
                    ui.horizontal(|ui| {
                        ui.monospace(format!("#{}", frame.index));
                        let addr_label = format!("0x{:x}", frame.address);
                        if ui.link(RichText::new(&addr_label).monospace()).clicked() {
                            self.pending_navigation = Some(frame.address);
                        }
                        if let Some(name) = &frame.function_name {
                            ui.label(name);
                        }
                        if let Some(file) = &frame.file {
                            let loc = if let Some(line) = frame.line {
                                format!("{file}:{line}")
                            } else {
                                file.clone()
                            };
                            ui.colored_label(Color32::GRAY, loc);
                        }
                    });
                }
            }
        });
    }

    fn render_memory_watch_section(&mut self, ui: &mut Ui) {
        egui::CollapsingHeader::new(
            RichText::new(format!("Memory Watch ({})", self.session.watches.len())).strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            // Add watch row
            ui.horizontal(|ui| {
                ui.label("Addr:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.watch_address)
                        .desired_width(100.0)
                        .font(egui::TextStyle::Monospace)
                        .hint_text("0x..."),
                );
                ui.label("Size:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.watch_size)
                        .desired_width(40.0)
                        .hint_text("16"),
                );
                ui.label("Label:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.watch_label)
                        .desired_width(80.0)
                        .hint_text("name"),
                );
                if ui.button("Add").clicked() {
                    let addr_str = self.watch_address.trim().trim_start_matches("0x");
                    let size_str = self.watch_size.trim();
                    if let Ok(addr) = u64::from_str_radix(addr_str, 16) {
                        let size = size_str.parse::<usize>().unwrap_or(16);
                        let label = if self.watch_label.trim().is_empty() {
                            format!("0x{addr:x}")
                        } else {
                            self.watch_label.trim().to_string()
                        };
                        self.session.add_watch(addr, size, label);
                        self.watch_address.clear();
                        self.watch_size.clear();
                        self.watch_label.clear();
                    } else {
                        self.session
                            .output_log
                            .push(format!("Invalid watch address: {}", self.watch_address));
                    }
                }
            });

            let mut remove_idx = None;
            for (i, watch) in self.session.watches.iter().enumerate() {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.strong(&watch.label);
                        ui.monospace(format!("0x{:x} ({} bytes)", watch.address, watch.size));
                        if ui.small_button("✕").clicked() {
                            remove_idx = Some(i);
                        }
                    });
                    // Render hex data
                    let hex: String = watch
                        .data
                        .iter()
                        .map(|b| format!("{b:02X}"))
                        .collect::<Vec<_>>()
                        .chunks(16)
                        .map(|chunk| chunk.join(" "))
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !hex.is_empty() {
                        ui.monospace(&hex);
                    }
                });
            }
            if let Some(idx) = remove_idx {
                self.session.remove_watch(idx);
            }
        });
    }

    fn render_output_log(&mut self, ui: &mut Ui) {
        egui::CollapsingHeader::new(
            RichText::new(format!("Output Log ({})", self.session.output_log.len())).strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Clear Log").clicked() {
                    self.session.output_log.clear();
                }
            });

            let available = (ui.available_height() - 8.0).max(100.0);
            egui::ScrollArea::vertical()
                .max_height(available)
                .stick_to_bottom(true)
                .id_salt("debugger_log_scroll")
                .show(ui, |ui| {
                    if self.session.output_log.is_empty() {
                        ui.colored_label(Color32::GRAY, "Debugger output will appear here.");
                    }
                    for line in &self.session.output_log {
                        ui.monospace(line);
                    }
                });
        });
    }
}
