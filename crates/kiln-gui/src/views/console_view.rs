//! Script console panel with REPL and output history (Sprint 13).

use crate::scripting::{self, OutputKind, OutputLine, ScriptResult};
use eframe::egui;
use kiln_core::analysis::AnalysisDatabase;

/// State for the script console view.
#[derive(Default)]
pub struct ConsoleView {
    /// Command input line.
    pub input: String,
    /// Output history.
    pub output: Vec<OutputLine>,
    /// Command history for up/down recall.
    pub history: Vec<String>,
    /// Current position in command history.
    history_pos: Option<usize>,
}

impl ConsoleView {
    /// Render the console panel.
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        analysis: &AnalysisDatabase,
        project: &mut kiln_project::Project,
    ) {
        // Toolbar
        ui.horizontal(|ui| {
            ui.heading("Script Console");
            ui.separator();
            if ui.button("Clear").clicked() {
                self.output.clear();
            }
        });
        ui.separator();

        // Output area
        let available = ui.available_height() - 32.0;
        egui::ScrollArea::vertical()
            .max_height(available)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if self.output.is_empty() {
                    ui.colored_label(
                        egui::Color32::GRAY,
                        "Rhai scripting console. Type expressions below and press Enter.",
                    );
                }
                for line in &self.output {
                    let color = match line.kind {
                        OutputKind::Normal => egui::Color32::LIGHT_GRAY,
                        OutputKind::Error => egui::Color32::from_rgb(255, 100, 100),
                        OutputKind::Info => egui::Color32::from_rgb(100, 200, 255),
                    };
                    ui.colored_label(color, &line.text);
                }
            });

        // Input line
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(">");
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.input)
                    .desired_width(ui.available_width() - 60.0)
                    .font(egui::TextStyle::Monospace)
                    .hint_text("Enter Rhai script..."),
            );

            if response.has_focus()
                && ui.input(|i| i.key_pressed(egui::Key::ArrowUp))
                && !self.history.is_empty()
            {
                let new_pos = match self.history_pos {
                    None => self.history.len() - 1,
                    Some(p) => p.saturating_sub(1),
                };
                self.history_pos = Some(new_pos);
                self.input = self.history[new_pos].clone();
            }
            if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                if let Some(pos) = self.history_pos {
                    if pos + 1 < self.history.len() {
                        self.history_pos = Some(pos + 1);
                        self.input = self.history[pos + 1].clone();
                    } else {
                        self.history_pos = None;
                        self.input.clear();
                    }
                }
            }

            let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let run_clicked = ui.button("Run").clicked();

            if (enter || run_clicked) && !self.input.trim().is_empty() {
                let code = self.input.clone();
                self.output.push(OutputLine {
                    text: format!("> {code}"),
                    kind: OutputKind::Info,
                });
                self.history.push(code.clone());
                self.history_pos = None;

                let result = scripting::execute_script(&code, analysis, project);
                self.apply_result(result);
                self.input.clear();
                response.request_focus();
            }
        });
    }

    /// Execute a script file and append results.
    pub fn run_file(
        &mut self,
        path: &std::path::Path,
        analysis: &AnalysisDatabase,
        project: &mut kiln_project::Project,
    ) {
        self.output.push(OutputLine {
            text: format!("Running script: {}", path.display()),
            kind: OutputKind::Info,
        });
        let result = scripting::execute_file(path, analysis, project);
        self.apply_result(result);
    }

    fn apply_result(&mut self, result: ScriptResult) {
        self.output.extend(result.output);
        if result.error.is_none() {
            self.output.push(OutputLine {
                text: "OK".to_string(),
                kind: OutputKind::Info,
            });
        }
    }
}
