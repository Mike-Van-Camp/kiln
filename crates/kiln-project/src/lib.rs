//! Project save/load and annotation storage for Kiln.
//!
//! This crate handles serializing and deserializing the analysis state,
//! user annotations (comments, renames), and project metadata.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
}

/// Project metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub binary_path: String,
    pub binary_hash: String,
    pub created_at: String,
    pub modified_at: String,
}

/// A saved project containing annotations and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub metadata: ProjectMetadata,
    pub annotations: BTreeMap<u64, Annotation>,
}

impl Project {
    /// Create a new empty project for the given binary.
    pub fn new(binary_path: String) -> Self {
        let now = chrono_placeholder();
        Self {
            metadata: ProjectMetadata {
                binary_path,
                binary_hash: String::new(),
                created_at: now.clone(),
                modified_at: now,
            },
            annotations: BTreeMap::new(),
        }
    }

    /// Add or update a comment at an address.
    pub fn set_comment(&mut self, addr: u64, comment: String) {
        let entry = self.annotations.entry(addr).or_insert_with(|| Annotation {
            comment: None,
            label: None,
        });
        entry.comment = Some(comment);
    }

    /// Add or update a label (rename) at an address.
    pub fn set_label(&mut self, addr: u64, label: String) {
        let entry = self.annotations.entry(addr).or_insert_with(|| Annotation {
            comment: None,
            label: None,
        });
        entry.label = Some(label);
    }

    /// Remove the comment at an address.
    pub fn remove_comment(&mut self, addr: u64) {
        if let Some(entry) = self.annotations.get_mut(&addr) {
            entry.comment = None;
            if entry.label.is_none() {
                self.annotations.remove(&addr);
            }
        }
    }

    /// Remove the label at an address.
    pub fn remove_label(&mut self, addr: u64) {
        if let Some(entry) = self.annotations.get_mut(&addr) {
            entry.label = None;
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
}
