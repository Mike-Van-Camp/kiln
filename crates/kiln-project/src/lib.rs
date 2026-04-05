//! Project save/load and annotation storage for Kiln.
//!
//! This crate handles serializing and deserializing the analysis state,
//! user annotations (comments, renames), type definitions, and project metadata.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialize(#[from] bincode::Error),
}

/// A user annotation at a specific address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub comment: Option<String>,
    pub label: Option<String>,
    /// Who made this annotation (Sprint 15).
    #[serde(default)]
    pub author: Option<String>,
    /// Unix timestamp when this annotation was last modified (Sprint 15).
    #[serde(default)]
    pub timestamp: Option<u64>,
}

/// A change recorded in the project history (Sprint 15).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnnotationChange {
    SetComment(String),
    RemoveComment,
    SetLabel(String),
    RemoveLabel,
}

/// A single history entry recording an annotation change (Sprint 15).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationHistoryEntry {
    pub address: u64,
    pub author: String,
    pub timestamp: u64,
    pub change: AnnotationChange,
}

/// A conflict between local and remote annotations during merge (Sprint 15).
#[derive(Debug, Clone)]
pub struct MergeConflict {
    pub address: u64,
    pub local: Annotation,
    pub remote: Annotation,
}

/// Resolution strategy for a merge conflict (Sprint 15).
#[derive(Debug, Clone, Copy)]
pub enum MergeResolution {
    KeepLocal,
    KeepRemote,
}

/// Project metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub binary_path: String,
    pub binary_hash: String,
    pub created_at: String,
    pub modified_at: String,
}

// ---------------------------------------------------------------------------
// Type system (Sprint 11)
// ---------------------------------------------------------------------------

/// Primitive data types supported by the type system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrimitiveType {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Char,
    Bool,
}

impl PrimitiveType {
    /// Size in bytes of this primitive type.
    pub fn size_bytes(&self) -> usize {
        match self {
            PrimitiveType::U8 | PrimitiveType::I8 | PrimitiveType::Char | PrimitiveType::Bool => 1,
            PrimitiveType::U16 | PrimitiveType::I16 => 2,
            PrimitiveType::U32 | PrimitiveType::I32 | PrimitiveType::F32 => 4,
            PrimitiveType::U64 | PrimitiveType::I64 | PrimitiveType::F64 => 8,
        }
    }

    /// Human-readable display name.
    pub fn display_name(&self) -> &'static str {
        match self {
            PrimitiveType::U8 => "u8",
            PrimitiveType::U16 => "u16",
            PrimitiveType::U32 => "u32",
            PrimitiveType::U64 => "u64",
            PrimitiveType::I8 => "i8",
            PrimitiveType::I16 => "i16",
            PrimitiveType::I32 => "i32",
            PrimitiveType::I64 => "i64",
            PrimitiveType::F32 => "f32",
            PrimitiveType::F64 => "f64",
            PrimitiveType::Char => "char",
            PrimitiveType::Bool => "bool",
        }
    }

    /// All primitive types, in display order.
    pub const ALL: &'static [PrimitiveType] = &[
        PrimitiveType::U8,
        PrimitiveType::U16,
        PrimitiveType::U32,
        PrimitiveType::U64,
        PrimitiveType::I8,
        PrimitiveType::I16,
        PrimitiveType::I32,
        PrimitiveType::I64,
        PrimitiveType::F32,
        PrimitiveType::F64,
        PrimitiveType::Char,
        PrimitiveType::Bool,
    ];
}

/// A single field inside a struct definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructField {
    pub name: String,
    pub type_name: String,
}

/// A struct definition with ordered fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<StructField>,
}

/// A single variant inside an enum definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumVariant {
    pub name: String,
    pub value: i64,
}

/// An enum definition with named integer variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<EnumVariant>,
}

/// A user-defined (or primitive) type definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypeDef {
    Primitive(PrimitiveType),
    /// Array of `element_type_name` with a fixed `count`.
    Array {
        element_type_name: String,
        count: usize,
    },
    Struct(StructDef),
    Enum(EnumDef),
}

impl TypeDef {
    /// Compute the total size in bytes, resolving nested types via `defs`.
    /// Returns `None` if any referenced type is not found.
    pub fn size_bytes(&self, defs: &HashMap<String, TypeDef>) -> Option<usize> {
        match self {
            TypeDef::Primitive(p) => Some(p.size_bytes()),
            TypeDef::Array {
                element_type_name,
                count,
            } => {
                let elem = defs.get(element_type_name)?;
                Some(elem.size_bytes(defs)? * count)
            }
            TypeDef::Struct(s) => {
                let mut total = 0usize;
                for field in &s.fields {
                    let ft = defs.get(&field.type_name)?;
                    total += ft.size_bytes(defs)?;
                }
                Some(total)
            }
            TypeDef::Enum(_) => Some(4), // enums are stored as i32
        }
    }

    /// Human-readable display name for this type definition.
    pub fn display_name(&self) -> String {
        match self {
            TypeDef::Primitive(p) => p.display_name().to_string(),
            TypeDef::Array {
                element_type_name,
                count,
            } => format!("{}[{}]", element_type_name, count),
            TypeDef::Struct(s) => s.name.clone(),
            TypeDef::Enum(e) => e.name.clone(),
        }
    }
}

/// A type applied at a specific address in the binary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedType {
    pub type_name: String,
    pub label: Option<String>,
}

/// A saved project containing annotations, type definitions, and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub metadata: ProjectMetadata,
    pub annotations: BTreeMap<u64, Annotation>,
    /// User-defined type definitions (name → definition).
    #[serde(default)]
    pub type_definitions: HashMap<String, TypeDef>,
    /// Types applied at specific addresses (address → applied type).
    #[serde(default)]
    pub applied_types: BTreeMap<u64, AppliedType>,
    /// Current author name for annotations (Sprint 15).
    #[serde(default)]
    pub author: String,
    /// History of annotation changes (Sprint 15).
    #[serde(default)]
    pub history: Vec<AnnotationHistoryEntry>,
}

impl Project {
    /// Create a new empty project for the given binary.
    pub fn new(binary_path: String) -> Self {
        let now = chrono_placeholder();
        let mut type_definitions = HashMap::new();
        // Register all primitive types so they can be referenced by name.
        for p in PrimitiveType::ALL {
            type_definitions.insert(p.display_name().to_string(), TypeDef::Primitive(*p));
        }
        Self {
            metadata: ProjectMetadata {
                binary_path,
                binary_hash: String::new(),
                created_at: now.clone(),
                modified_at: now,
            },
            annotations: BTreeMap::new(),
            type_definitions,
            applied_types: BTreeMap::new(),
            author: String::new(),
            history: Vec::new(),
        }
    }

    /// Add or update a comment at an address.
    pub fn set_comment(&mut self, addr: u64, comment: String) {
        let ts = current_timestamp();
        let author = self.author.clone();
        let entry = self.annotations.entry(addr).or_insert_with(|| Annotation {
            comment: None,
            label: None,
            author: None,
            timestamp: None,
        });
        entry.comment = Some(comment.clone());
        entry.author = Some(author.clone());
        entry.timestamp = Some(ts);
        self.history.push(AnnotationHistoryEntry {
            address: addr,
            author,
            timestamp: ts,
            change: AnnotationChange::SetComment(comment),
        });
    }

    /// Add or update a label (rename) at an address.
    pub fn set_label(&mut self, addr: u64, label: String) {
        let ts = current_timestamp();
        let author = self.author.clone();
        let entry = self.annotations.entry(addr).or_insert_with(|| Annotation {
            comment: None,
            label: None,
            author: None,
            timestamp: None,
        });
        entry.label = Some(label.clone());
        entry.author = Some(author.clone());
        entry.timestamp = Some(ts);
        self.history.push(AnnotationHistoryEntry {
            address: addr,
            author,
            timestamp: ts,
            change: AnnotationChange::SetLabel(label),
        });
    }

    /// Remove the comment at an address.
    pub fn remove_comment(&mut self, addr: u64) {
        if let Some(entry) = self.annotations.get_mut(&addr) {
            entry.comment = None;
            let ts = current_timestamp();
            let author = self.author.clone();
            self.history.push(AnnotationHistoryEntry {
                address: addr,
                author,
                timestamp: ts,
                change: AnnotationChange::RemoveComment,
            });
            if entry.label.is_none() {
                self.annotations.remove(&addr);
            }
        }
    }

    /// Remove the label at an address.
    pub fn remove_label(&mut self, addr: u64) {
        if let Some(entry) = self.annotations.get_mut(&addr) {
            entry.label = None;
            let ts = current_timestamp();
            let author = self.author.clone();
            self.history.push(AnnotationHistoryEntry {
                address: addr,
                author,
                timestamp: ts,
                change: AnnotationChange::RemoveLabel,
            });
            if entry.comment.is_none() {
                self.annotations.remove(&addr);
            }
        }
    }

    /// Get the comment at an address.
    pub fn get_comment(&self, addr: u64) -> Option<&str> {
        self.annotations
            .get(&addr)
            .and_then(|a| a.comment.as_deref())
    }

    /// Get the label at an address.
    pub fn get_label(&self, addr: u64) -> Option<&str> {
        self.annotations.get(&addr).and_then(|a| a.label.as_deref())
    }

    // -- Type system methods (Sprint 11) --

    /// Add or replace a type definition.
    pub fn add_type_def(&mut self, name: String, def: TypeDef) {
        self.type_definitions.insert(name, def);
    }

    /// Remove a type definition by name. Returns the removed definition if it existed.
    pub fn remove_type_def(&mut self, name: &str) -> Option<TypeDef> {
        self.type_definitions.remove(name)
    }

    /// Get a type definition by name.
    pub fn get_type_def(&self, name: &str) -> Option<&TypeDef> {
        self.type_definitions.get(name)
    }

    /// Apply a type at a specific address.
    pub fn apply_type_at(&mut self, addr: u64, applied: AppliedType) {
        self.applied_types.insert(addr, applied);
    }

    /// Remove the applied type at an address.
    pub fn remove_type_at(&mut self, addr: u64) -> Option<AppliedType> {
        self.applied_types.remove(&addr)
    }

    /// Get the applied type at an address.
    pub fn get_applied_type(&self, addr: u64) -> Option<&AppliedType> {
        self.applied_types.get(&addr)
    }

    /// Return names of all user-defined (non-primitive) types, sorted.
    pub fn user_type_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .type_definitions
            .iter()
            .filter(|(_, v)| !matches!(v, TypeDef::Primitive(_)))
            .map(|(k, _)| k.clone())
            .collect();
        names.sort();
        names
    }

    /// Return names of all type definitions (including primitives), sorted.
    pub fn all_type_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.type_definitions.keys().cloned().collect();
        names.sort();
        names
    }

    // -- Collaborative analysis methods (Sprint 15) --

    /// Export annotations as a JSON string.
    pub fn export_annotations_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(&self.annotations)
            .map_err(|e| format!("JSON export error: {e}"))
    }

    /// Import annotations from a JSON string.
    pub fn import_annotations_json(json: &str) -> Result<BTreeMap<u64, Annotation>, String> {
        serde_json::from_str(json).map_err(|e| format!("JSON import error: {e}"))
    }

    /// Merge remote annotations into local, returning merged result and conflicts.
    /// Non-conflicting annotations are merged automatically.
    /// Conflicting ones (both local and remote have annotations at the same address) are returned.
    pub fn merge_annotations(
        local: &BTreeMap<u64, Annotation>,
        remote: &BTreeMap<u64, Annotation>,
    ) -> (BTreeMap<u64, Annotation>, Vec<MergeConflict>) {
        let mut merged = local.clone();
        let mut conflicts = Vec::new();

        for (addr, remote_ann) in remote {
            if let Some(local_ann) = local.get(addr) {
                // Both have annotations at this address — conflict
                conflicts.push(MergeConflict {
                    address: *addr,
                    local: local_ann.clone(),
                    remote: remote_ann.clone(),
                });
            } else {
                // Only remote has it — auto-merge
                merged.insert(*addr, remote_ann.clone());
            }
        }

        (merged, conflicts)
    }

    /// Apply a resolution to a merge conflict.
    pub fn resolve_conflict(
        merged: &mut BTreeMap<u64, Annotation>,
        conflict: &MergeConflict,
        resolution: MergeResolution,
    ) {
        match resolution {
            MergeResolution::KeepLocal => {
                merged.insert(conflict.address, conflict.local.clone());
            }
            MergeResolution::KeepRemote => {
                merged.insert(conflict.address, conflict.remote.clone());
            }
        }
    }

    /// Save project to a file.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), ProjectError> {
        let data = bincode::serialize(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Load project from a file.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ProjectError> {
        let data = std::fs::read(path)?;
        let project = bincode::deserialize(&data)?;
        Ok(project)
    }
}

fn chrono_placeholder() -> String {
    "2025-01-01T00:00:00Z".to_string()
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_annotations() {
        let mut project = Project::new("/bin/ls".to_string());

        project.set_comment(0x401000, "Entry point".to_string());
        project.set_label(0x401000, "main".to_string());
        project.set_comment(0x401020, "Loop start".to_string());

        assert_eq!(project.get_comment(0x401000), Some("Entry point"));
        assert_eq!(project.get_label(0x401000), Some("main"));
        assert_eq!(project.get_comment(0x401020), Some("Loop start"));
        assert_eq!(project.get_label(0x401020), None);
        assert_eq!(project.get_comment(0x999999), None);
    }

    #[test]
    fn test_project_save_load() {
        let mut project = Project::new("/bin/ls".to_string());
        project.set_comment(0x401000, "test comment".to_string());
        project.set_label(0x401000, "test_func".to_string());

        let path = std::env::temp_dir().join("kiln_test_project.kproj");
        project.save(&path).unwrap();

        let loaded = Project::load(&path).unwrap();
        assert_eq!(loaded.get_comment(0x401000), Some("test comment"));
        assert_eq!(loaded.get_label(0x401000), Some("test_func"));
        assert_eq!(loaded.metadata.binary_path, "/bin/ls");

        // Clean up
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_primitive_type_sizes() {
        assert_eq!(PrimitiveType::U8.size_bytes(), 1);
        assert_eq!(PrimitiveType::U16.size_bytes(), 2);
        assert_eq!(PrimitiveType::U32.size_bytes(), 4);
        assert_eq!(PrimitiveType::U64.size_bytes(), 8);
        assert_eq!(PrimitiveType::I8.size_bytes(), 1);
        assert_eq!(PrimitiveType::I16.size_bytes(), 2);
        assert_eq!(PrimitiveType::I32.size_bytes(), 4);
        assert_eq!(PrimitiveType::I64.size_bytes(), 8);
        assert_eq!(PrimitiveType::F32.size_bytes(), 4);
        assert_eq!(PrimitiveType::F64.size_bytes(), 8);
        assert_eq!(PrimitiveType::Char.size_bytes(), 1);
        assert_eq!(PrimitiveType::Bool.size_bytes(), 1);
    }

    #[test]
    fn test_add_remove_type_def() {
        let mut project = Project::new("/bin/ls".to_string());

        // Primitives are pre-registered
        assert!(project.get_type_def("u8").is_some());
        assert!(project.get_type_def("u32").is_some());

        // Add a struct
        let my_struct = TypeDef::Struct(StructDef {
            name: "MyStruct".to_string(),
            fields: vec![
                StructField {
                    name: "x".to_string(),
                    type_name: "u32".to_string(),
                },
                StructField {
                    name: "y".to_string(),
                    type_name: "u32".to_string(),
                },
            ],
        });
        project.add_type_def("MyStruct".to_string(), my_struct);
        assert!(project.get_type_def("MyStruct").is_some());

        // Size should be 8 (two u32)
        let def = project.get_type_def("MyStruct").unwrap();
        assert_eq!(def.size_bytes(&project.type_definitions), Some(8));

        // Remove it
        let removed = project.remove_type_def("MyStruct");
        assert!(removed.is_some());
        assert!(project.get_type_def("MyStruct").is_none());
    }

    #[test]
    fn test_array_type_size() {
        let mut project = Project::new("/bin/ls".to_string());

        let arr = TypeDef::Array {
            element_type_name: "u8".to_string(),
            count: 256,
        };
        project.add_type_def("u8_256".to_string(), arr);
        let def = project.get_type_def("u8_256").unwrap();
        assert_eq!(def.size_bytes(&project.type_definitions), Some(256));
    }

    #[test]
    fn test_enum_type_def() {
        let mut project = Project::new("/bin/ls".to_string());

        let my_enum = TypeDef::Enum(EnumDef {
            name: "Color".to_string(),
            variants: vec![
                EnumVariant {
                    name: "Red".to_string(),
                    value: 0,
                },
                EnumVariant {
                    name: "Green".to_string(),
                    value: 1,
                },
                EnumVariant {
                    name: "Blue".to_string(),
                    value: 2,
                },
            ],
        });
        project.add_type_def("Color".to_string(), my_enum);
        let def = project.get_type_def("Color").unwrap();
        assert_eq!(def.size_bytes(&project.type_definitions), Some(4));
    }

    #[test]
    fn test_apply_remove_type() {
        let mut project = Project::new("/bin/ls".to_string());

        project.apply_type_at(
            0x401000,
            AppliedType {
                type_name: "u32".to_string(),
                label: Some("counter".to_string()),
            },
        );

        let applied = project.get_applied_type(0x401000);
        assert!(applied.is_some());
        assert_eq!(applied.unwrap().type_name, "u32");
        assert_eq!(applied.unwrap().label.as_deref(), Some("counter"));

        project.remove_type_at(0x401000);
        assert!(project.get_applied_type(0x401000).is_none());
    }

    #[test]
    fn test_type_def_serialization() {
        let mut project = Project::new("/bin/ls".to_string());

        project.add_type_def(
            "Point".to_string(),
            TypeDef::Struct(StructDef {
                name: "Point".to_string(),
                fields: vec![
                    StructField {
                        name: "x".to_string(),
                        type_name: "f32".to_string(),
                    },
                    StructField {
                        name: "y".to_string(),
                        type_name: "f32".to_string(),
                    },
                ],
            }),
        );
        project.apply_type_at(
            0x402000,
            AppliedType {
                type_name: "Point".to_string(),
                label: None,
            },
        );

        let path = std::env::temp_dir().join("kiln_test_types.kproj");
        project.save(&path).unwrap();

        let loaded = Project::load(&path).unwrap();
        assert!(loaded.get_type_def("Point").is_some());
        assert!(loaded.get_applied_type(0x402000).is_some());
        assert_eq!(
            loaded.get_applied_type(0x402000).unwrap().type_name,
            "Point"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_user_type_names() {
        let mut project = Project::new("/bin/ls".to_string());
        project.add_type_def(
            "Foo".to_string(),
            TypeDef::Struct(StructDef {
                name: "Foo".to_string(),
                fields: vec![],
            }),
        );
        let names = project.user_type_names();
        assert!(names.contains(&"Foo".to_string()));
        // Primitives should NOT appear
        assert!(!names.contains(&"u8".to_string()));
    }

    #[test]
    fn test_json_export_import_roundtrip() {
        let mut project = Project::new("/bin/ls".to_string());
        project.set_comment(0x1000, "hello".to_string());
        project.set_label(0x2000, "my_func".to_string());

        let json = project.export_annotations_json().unwrap();
        let imported = Project::import_annotations_json(&json).unwrap();

        assert_eq!(
            imported.get(&0x1000).unwrap().comment.as_deref(),
            Some("hello")
        );
        assert_eq!(
            imported.get(&0x2000).unwrap().label.as_deref(),
            Some("my_func")
        );
    }

    #[test]
    fn test_merge_no_conflicts() {
        let mut local = BTreeMap::new();
        local.insert(
            0x1000,
            Annotation {
                comment: Some("local only".into()),
                label: None,
                author: None,
                timestamp: None,
            },
        );
        let mut remote = BTreeMap::new();
        remote.insert(
            0x2000,
            Annotation {
                comment: Some("remote only".into()),
                label: None,
                author: None,
                timestamp: None,
            },
        );

        let (merged, conflicts) = Project::merge_annotations(&local, &remote);
        assert!(conflicts.is_empty());
        assert_eq!(merged.len(), 2);
        assert!(merged.contains_key(&0x1000));
        assert!(merged.contains_key(&0x2000));
    }

    #[test]
    fn test_merge_with_conflicts() {
        let mut local = BTreeMap::new();
        local.insert(
            0x1000,
            Annotation {
                comment: Some("local comment".into()),
                label: None,
                author: None,
                timestamp: None,
            },
        );
        let mut remote = BTreeMap::new();
        remote.insert(
            0x1000,
            Annotation {
                comment: Some("remote comment".into()),
                label: None,
                author: None,
                timestamp: None,
            },
        );

        let (_, conflicts) = Project::merge_annotations(&local, &remote);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].address, 0x1000);
    }

    #[test]
    fn test_author_tracking() {
        let mut project = Project::new("/bin/ls".to_string());
        project.author = "alice".to_string();
        project.set_comment(0x1000, "test".to_string());

        let ann = project.annotations.get(&0x1000).unwrap();
        assert_eq!(ann.author.as_deref(), Some("alice"));
        assert!(ann.timestamp.is_some());
    }

    #[test]
    fn test_history_recording() {
        let mut project = Project::new("/bin/ls".to_string());
        project.author = "bob".to_string();
        project.set_comment(0x1000, "first".to_string());
        project.set_label(0x2000, "func".to_string());
        project.remove_comment(0x1000);

        assert_eq!(project.history.len(), 3);
        assert!(matches!(
            project.history[0].change,
            AnnotationChange::SetComment(_)
        ));
        assert!(matches!(
            project.history[1].change,
            AnnotationChange::SetLabel(_)
        ));
        assert!(matches!(
            project.history[2].change,
            AnnotationChange::RemoveComment
        ));
        assert_eq!(project.history[0].author, "bob");
    }
}
