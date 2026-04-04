//! Collaborative analysis view — export/import, merge, author tracking, history (Sprint 15).

use kiln_project::{AnnotationChange, MergeConflict, MergeResolution, Project};

/// State for the collaborative analysis panel.
#[derive(Default)]
pub struct CollabView {
    /// Current author name.
    pub author_name: String,
    /// Active merge conflicts awaiting resolution.
    pub merge_conflicts: Vec<MergeConflict>,
    /// Per-conflict resolution choice (None = not yet resolved).
    pub merge_resolutions: Vec<Option<MergeResolution>>,
    /// Status message displayed to user.
    pub status_message: Option<String>,
    /// Whether the history panel is expanded.
    pub show_history: bool,
    /// JSON text area for import.
    pub import_text: String,
}

impl CollabView {
    /// Render the collaborative analysis panel.
    pub fn render(&mut self, ui: &mut egui::Ui, project: &mut Project) {
        // Author field
        ui.horizontal(|ui| {
            ui.label("Author:");
            if ui.text_edit_singleline(&mut self.author_name).changed() {
                project.author = self.author_name.clone();
            }
        });
        ui.separator();

        // Export/Import toolbar
        ui.horizontal(|ui| {
            if ui.button("📤 Export JSON").clicked() {
                match project.export_annotations_json() {
                    Ok(json) => {
                        ui.ctx().copy_text(json);
                        self.status_message =
                            Some("Annotations exported to clipboard".to_string());
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Export error: {e}"));
                    }
                }
            }
            if ui.button("📥 Import & Merge").clicked() {
                if self.import_text.is_empty() {
                    self.status_message =
                        Some("Paste JSON into the text area below first".to_string());
                } else {
                    match Project::import_annotations_json(&self.import_text) {
                        Ok(remote) => {
                            let (merged, conflicts) =
                                Project::merge_annotations(&project.annotations, &remote);
                            if conflicts.is_empty() {
                                project.annotations = merged;
                                self.status_message = Some(format!(
                                    "Merged {} annotations (no conflicts)",
                                    remote.len()
                                ));
                            } else {
                                let n = conflicts.len();
                                self.merge_resolutions =
                                    vec![None; n];
                                self.merge_conflicts = conflicts;
                                // Apply non-conflicting part
                                project.annotations = merged;
                                self.status_message = Some(format!(
                                    "Merged with {n} conflict(s) — resolve below"
                                ));
                            }
                        }
                        Err(e) => {
                            self.status_message = Some(format!("Import error: {e}"));
                        }
                    }
                }
            }
        });

        // Status
        if let Some(msg) = &self.status_message {
            ui.label(msg.as_str());
        }
        ui.separator();

        // Import text area
        ui.label("Paste JSON to import:");
        egui::ScrollArea::vertical()
            .id_salt("import_json")
            .max_height(120.0)
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.import_text)
                        .desired_width(f32::INFINITY)
                        .desired_rows(5)
                        .font(egui::TextStyle::Monospace),
                );
            });
        ui.separator();

        // Merge conflict resolution
        if !self.merge_conflicts.is_empty() {
            ui.heading(format!("Merge Conflicts ({})", self.merge_conflicts.len()));

            let mut all_resolved = true;
            for i in 0..self.merge_conflicts.len() {
                let conflict = &self.merge_conflicts[i];
                ui.group(|ui| {
                    ui.label(format!("Address: 0x{:x}", conflict.address));
                    ui.horizontal(|ui| {
                        ui.label("Local:");
                        if let Some(c) = &conflict.local.comment {
                            ui.label(format!("comment=\"{c}\""));
                        }
                        if let Some(l) = &conflict.local.label {
                            ui.label(format!("label=\"{l}\""));
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Remote:");
                        if let Some(c) = &conflict.remote.comment {
                            ui.label(format!("comment=\"{c}\""));
                        }
                        if let Some(l) = &conflict.remote.label {
                            ui.label(format!("label=\"{l}\""));
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Keep Mine").clicked() {
                            self.merge_resolutions[i] = Some(MergeResolution::KeepLocal);
                        }
                        if ui.button("Keep Theirs").clicked() {
                            self.merge_resolutions[i] = Some(MergeResolution::KeepRemote);
                        }
                        if let Some(res) = &self.merge_resolutions[i] {
                            let lbl = match res {
                                MergeResolution::KeepLocal => "✓ Keeping local",
                                MergeResolution::KeepRemote => "✓ Keeping remote",
                            };
                            ui.label(lbl);
                        } else {
                            all_resolved = false;
                        }
                    });
                });
            }

            if all_resolved && !self.merge_conflicts.is_empty()
                && ui.button("Apply All Resolutions").clicked()
            {
                for (i, conflict) in self.merge_conflicts.iter().enumerate() {
                    if let Some(res) = self.merge_resolutions[i] {
                        Project::resolve_conflict(
                            &mut project.annotations,
                            conflict,
                            res,
                        );
                    }
                }
                self.merge_conflicts.clear();
                self.merge_resolutions.clear();
                self.status_message = Some("All conflicts resolved".to_string());
            }
        }

        ui.separator();

        // History panel
        let history_label = if self.show_history {
            "▼ History"
        } else {
            "▶ History"
        };
        if ui.button(history_label).clicked() {
            self.show_history = !self.show_history;
        }
        if self.show_history {
            egui::ScrollArea::vertical()
                .id_salt("history_panel")
                .max_height(300.0)
                .show(ui, |ui| {
                    for entry in project.history.iter().rev().take(100) {
                        let change_desc = match &entry.change {
                            AnnotationChange::SetComment(c) => {
                                format!("comment = \"{c}\"")
                            }
                            AnnotationChange::RemoveComment => "removed comment".to_string(),
                            AnnotationChange::SetLabel(l) => format!("label = \"{l}\""),
                            AnnotationChange::RemoveLabel => "removed label".to_string(),
                        };
                        ui.label(format!(
                            "0x{:x} — {} — {}",
                            entry.address, entry.author, change_desc
                        ));
                    }
                    if project.history.is_empty() {
                        ui.label("No history yet.");
                    }
                });
        }
    }
}
