//! Types editor view: list, create, and edit user-defined data types.

use egui::{Color32, RichText, Ui};
use kiln_project::{EnumDef, EnumVariant, PrimitiveType, StructDef, StructField, TypeDef};

/// Color constants for the types view.
const COLOR_TYPE_NAME: Color32 = Color32::from_rgb(180, 220, 140);
const COLOR_FIELD_NAME: Color32 = Color32::from_rgb(180, 180, 255);
const COLOR_SIZE_INFO: Color32 = Color32::GRAY;

/// State for the struct editor dialog.
#[derive(Default)]
pub struct StructEditorDialog {
    pub open: bool,
    pub name: String,
    pub fields: Vec<(String, String)>, // (field_name, type_name)
    pub error: Option<String>,
    /// If editing an existing struct, holds the original name.
    pub editing: Option<String>,
}

/// State for the enum editor dialog.
#[derive(Default)]
pub struct EnumEditorDialog {
    pub open: bool,
    pub name: String,
    pub variants: Vec<(String, String)>, // (variant_name, value as string)
    pub error: Option<String>,
    /// If editing an existing enum, holds the original name.
    pub editing: Option<String>,
}

/// State for the types view panel.
#[derive(Default)]
pub struct TypesView {
    /// Which type is currently expanded for details.
    expanded_type: Option<String>,
}

impl TypesView {
    /// Render the types list panel. Returns an action to perform on the project.
    pub fn render(
        &mut self,
        ui: &mut Ui,
        project: &kiln_project::Project,
        struct_dialog: &mut StructEditorDialog,
        enum_dialog: &mut EnumEditorDialog,
    ) -> Option<TypesAction> {
        let mut action = None;

        ui.horizontal(|ui| {
            ui.heading("Type Definitions");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("➕ New Enum").clicked() {
                    enum_dialog.open = true;
                    enum_dialog.name.clear();
                    enum_dialog.variants = vec![("Variant0".to_string(), "0".to_string())];
                    enum_dialog.error = None;
                    enum_dialog.editing = None;
                }
                if ui.button("➕ New Struct").clicked() {
                    struct_dialog.open = true;
                    struct_dialog.name.clear();
                    struct_dialog.fields = vec![("field0".to_string(), "u8".to_string())];
                    struct_dialog.error = None;
                    struct_dialog.editing = None;
                }
            });
        });
        ui.separator();

        let user_types = project.user_type_names();

        if user_types.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    "No user-defined types yet.\nUse the buttons above to create structs or enums.",
                );
            });
            return None;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for type_name in &user_types {
                    if let Some(def) = project.get_type_def(type_name) {
                        let size_str = def
                            .size_bytes(&project.type_definitions)
                            .map(|s| format!("{} bytes", s))
                            .unwrap_or_else(|| "? bytes".to_string());

                        let is_expanded = self.expanded_type.as_ref() == Some(type_name);

                        let header = match def {
                            TypeDef::Struct(_) => format!("📦 struct {}", type_name),
                            TypeDef::Enum(_) => format!("🏷 enum {}", type_name),
                            TypeDef::Array {
                                element_type_name,
                                count,
                            } => {
                                format!("📐 {}[{}]", element_type_name, count)
                            }
                            TypeDef::Primitive(_) => continue,
                        };

                        ui.horizontal(|ui| {
                            let toggle = ui.selectable_label(
                                is_expanded,
                                RichText::new(&header).color(COLOR_TYPE_NAME),
                            );
                            if toggle.clicked() {
                                if is_expanded {
                                    self.expanded_type = None;
                                } else {
                                    self.expanded_type = Some(type_name.clone());
                                }
                            }

                            ui.label(RichText::new(&size_str).color(COLOR_SIZE_INFO).small());

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("🗑").on_hover_text("Delete type").clicked()
                                    {
                                        action = Some(TypesAction::DeleteType(type_name.clone()));
                                    }
                                    if ui.small_button("✏").on_hover_text("Edit type").clicked() {
                                        match def {
                                            TypeDef::Struct(s) => {
                                                struct_dialog.open = true;
                                                struct_dialog.name = s.name.clone();
                                                struct_dialog.fields = s
                                                    .fields
                                                    .iter()
                                                    .map(|f| (f.name.clone(), f.type_name.clone()))
                                                    .collect();
                                                struct_dialog.error = None;
                                                struct_dialog.editing = Some(type_name.clone());
                                            }
                                            TypeDef::Enum(e) => {
                                                enum_dialog.open = true;
                                                enum_dialog.name = e.name.clone();
                                                enum_dialog.variants = e
                                                    .variants
                                                    .iter()
                                                    .map(|v| (v.name.clone(), v.value.to_string()))
                                                    .collect();
                                                enum_dialog.error = None;
                                                enum_dialog.editing = Some(type_name.clone());
                                            }
                                            _ => {}
                                        }
                                    }
                                },
                            );
                        });

                        // Show details when expanded
                        if is_expanded {
                            ui.indent(type_name, |ui| match def {
                                TypeDef::Struct(s) => {
                                    for field in &s.fields {
                                        let fsize = project
                                            .get_type_def(&field.type_name)
                                            .and_then(|d| d.size_bytes(&project.type_definitions))
                                            .map(|s| format!("({} B)", s))
                                            .unwrap_or_default();
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(&field.name).color(COLOR_FIELD_NAME),
                                            );
                                            ui.label(format!(": {}", field.type_name));
                                            ui.label(
                                                RichText::new(&fsize)
                                                    .color(COLOR_SIZE_INFO)
                                                    .small(),
                                            );
                                        });
                                    }
                                }
                                TypeDef::Enum(e) => {
                                    for v in &e.variants {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(&v.name).color(COLOR_FIELD_NAME),
                                            );
                                            ui.label(format!("= {}", v.value));
                                        });
                                    }
                                }
                                _ => {}
                            });
                        }

                        ui.separator();
                    }
                }
            });

        action
    }
}

/// Actions that the types view can request.
pub enum TypesAction {
    DeleteType(String),
}

/// Render the struct editor dialog. Returns `Some(TypeDef)` when the user confirms.
pub fn render_struct_editor(
    ctx: &egui::Context,
    dialog: &mut StructEditorDialog,
    all_type_names: &[String],
) -> Option<(String, TypeDef)> {
    if !dialog.open {
        return None;
    }

    let mut result = None;
    let mut open = dialog.open;
    let title = if dialog.editing.is_some() {
        "Edit Struct"
    } else {
        "New Struct"
    };

    egui::Window::new(title)
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(400.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut dialog.name);
            });

            ui.separator();
            ui.label("Fields:");

            let mut remove_idx = None;
            for (i, (field_name, field_type)) in dialog.fields.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("{}.", i));
                    ui.add(egui::TextEdit::singleline(field_name).desired_width(120.0));
                    egui::ComboBox::from_id_salt(format!("field_type_{}", i))
                        .selected_text(field_type.as_str())
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            for tn in all_type_names {
                                ui.selectable_value(field_type, tn.clone(), tn);
                            }
                        });
                    if ui.small_button("🗑").clicked() {
                        remove_idx = Some(i);
                    }
                });
            }

            if let Some(idx) = remove_idx {
                dialog.fields.remove(idx);
            }

            if ui.button("+ Add Field").clicked() {
                let idx = dialog.fields.len();
                dialog
                    .fields
                    .push((format!("field{}", idx), "u8".to_string()));
            }

            if let Some(err) = &dialog.error {
                ui.colored_label(Color32::RED, err);
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    let name = dialog.name.trim().to_string();
                    if name.is_empty() {
                        dialog.error = Some("Name cannot be empty".to_string());
                    } else if PrimitiveType::ALL.iter().any(|p| p.display_name() == name) {
                        dialog.error = Some("Cannot use primitive type name".to_string());
                    } else if dialog.fields.is_empty() {
                        dialog.error = Some("Struct must have at least one field".to_string());
                    } else {
                        let fields: Vec<StructField> = dialog
                            .fields
                            .iter()
                            .map(|(n, t)| StructField {
                                name: n.trim().to_string(),
                                type_name: t.clone(),
                            })
                            .collect();
                        let def = TypeDef::Struct(StructDef {
                            name: name.clone(),
                            fields,
                        });
                        // If editing, remove old name first (handled by caller)
                        result = Some((name, def));
                        dialog.open = false;
                    }
                }
                if ui.button("Cancel").clicked() {
                    dialog.open = false;
                }
            });
        });

    dialog.open = open;
    result
}

/// Render the enum editor dialog. Returns `Some(TypeDef)` when the user confirms.
pub fn render_enum_editor(
    ctx: &egui::Context,
    dialog: &mut EnumEditorDialog,
) -> Option<(String, TypeDef)> {
    if !dialog.open {
        return None;
    }

    let mut result = None;
    let mut open = dialog.open;
    let title = if dialog.editing.is_some() {
        "Edit Enum"
    } else {
        "New Enum"
    };

    egui::Window::new(title)
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(400.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut dialog.name);
            });

            ui.separator();
            ui.label("Variants:");

            let mut remove_idx = None;
            for (i, (var_name, var_value)) in dialog.variants.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("{}.", i));
                    ui.add(egui::TextEdit::singleline(var_name).desired_width(150.0));
                    ui.label("=");
                    ui.add(egui::TextEdit::singleline(var_value).desired_width(80.0));
                    if ui.small_button("🗑").clicked() {
                        remove_idx = Some(i);
                    }
                });
            }

            if let Some(idx) = remove_idx {
                dialog.variants.remove(idx);
            }

            if ui.button("+ Add Variant").clicked() {
                let idx = dialog.variants.len() as i64;
                dialog
                    .variants
                    .push((format!("Variant{}", idx), idx.to_string()));
            }

            if let Some(err) = &dialog.error {
                ui.colored_label(Color32::RED, err);
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    let name = dialog.name.trim().to_string();
                    if name.is_empty() {
                        dialog.error = Some("Name cannot be empty".to_string());
                    } else if PrimitiveType::ALL.iter().any(|p| p.display_name() == name) {
                        dialog.error = Some("Cannot use primitive type name".to_string());
                    } else if dialog.variants.is_empty() {
                        dialog.error = Some("Enum must have at least one variant".to_string());
                    } else {
                        let mut variants = Vec::new();
                        let mut parse_err = false;
                        for (vn, vv) in &dialog.variants {
                            match vv.trim().parse::<i64>() {
                                Ok(val) => variants.push(EnumVariant {
                                    name: vn.trim().to_string(),
                                    value: val,
                                }),
                                Err(_) => {
                                    dialog.error =
                                        Some(format!("Invalid integer value for '{}'", vn));
                                    parse_err = true;
                                    break;
                                }
                            }
                        }
                        if !parse_err {
                            let def = TypeDef::Enum(EnumDef {
                                name: name.clone(),
                                variants,
                            });
                            result = Some((name, def));
                            dialog.open = false;
                        }
                    }
                }
                if ui.button("Cancel").clicked() {
                    dialog.open = false;
                }
            });
        });

    dialog.open = open;
    result
}

/// Render the "Apply Type" dialog used from context menus.
#[derive(Default)]
pub struct ApplyTypeDialog {
    pub open: bool,
    pub address: u64,
    pub selected_type: String,
    pub label: String,
}

pub fn render_apply_type_dialog(
    ctx: &egui::Context,
    dialog: &mut ApplyTypeDialog,
    all_type_names: &[String],
) -> Option<(u64, kiln_project::AppliedType)> {
    if !dialog.open {
        return None;
    }

    let mut result = None;
    let mut open = dialog.open;

    egui::Window::new("Apply Type")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(format!("Address: 0x{:08X}", dialog.address));
            ui.horizontal(|ui| {
                ui.label("Type:");
                egui::ComboBox::from_id_salt("apply_type_combo")
                    .selected_text(&dialog.selected_type)
                    .width(180.0)
                    .show_ui(ui, |ui| {
                        for tn in all_type_names {
                            ui.selectable_value(&mut dialog.selected_type, tn.clone(), tn);
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Label (optional):");
                ui.text_edit_singleline(&mut dialog.label);
            });

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Apply").clicked() && !dialog.selected_type.is_empty() {
                    let label = if dialog.label.trim().is_empty() {
                        None
                    } else {
                        Some(dialog.label.trim().to_string())
                    };
                    result = Some((
                        dialog.address,
                        kiln_project::AppliedType {
                            type_name: dialog.selected_type.clone(),
                            label,
                        },
                    ));
                    dialog.open = false;
                }
                if ui.button("Cancel").clicked() {
                    dialog.open = false;
                }
            });
        });

    dialog.open = open;
    result
}
