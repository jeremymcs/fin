// Fin — .planning/ Directory Management
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use std::path::{Path, PathBuf};

/// Manages the .planning/ directory structure for a project.
#[allow(dead_code)]
pub struct PlanningDir {
    root: PathBuf,
}

#[allow(dead_code)]
impl PlanningDir {
    pub fn new(project_root: &Path) -> Self {
        Self {
            root: project_root.join(".planning"),
        }
    }

    /// Initialize .planning/ directory structure.
    pub fn init(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::create_dir_all(self.root.join("stages"))?;

        // Create STATUS.md if it doesn't exist
        let status_path = self.root.join("STATUS.md");
        if !status_path.exists() {
            std::fs::write(&status_path, "# Project Status\n\nStatus: initialized\n")?;
        }

        Ok(())
    }

    /// Check if .planning/ exists.
    pub fn exists(&self) -> bool {
        self.root.exists()
    }

    /// Get the project database path.
    pub fn db_path(&self) -> PathBuf {
        self.root.join("fin.db")
    }

    /// Get the STATUS.md path.
    pub fn status_path(&self) -> PathBuf {
        self.root.join("STATUS.md")
    }

    /// Get the VISION.md path.
    pub fn vision_path(&self) -> PathBuf {
        self.root.join("VISION.md")
    }

    /// Get the REQUIREMENTS.md path.
    pub fn requirements_path(&self) -> PathBuf {
        self.root.join("REQUIREMENTS.md")
    }

    /// Get a stage directory path.
    pub fn stage_dir(&self, blueprint_id: &str, section_id: Option<&str>) -> PathBuf {
        let mut path = self.root.join("stages").join(blueprint_id);
        if let Some(sid) = section_id {
            path = path.join(sid);
        }
        path
    }

    /// Create a stage directory.
    pub fn create_stage(
        &self,
        blueprint_id: &str,
        section_id: Option<&str>,
    ) -> anyhow::Result<PathBuf> {
        let dir = self.stage_dir(blueprint_id, section_id);
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// List all blueprint directories.
    pub fn list_blueprints(&self) -> Vec<String> {
        let stages_dir = self.root.join("stages");
        if !stages_dir.exists() {
            return Vec::new();
        }

        let mut blueprints = Vec::new();
        if let Ok(entries) = std::fs::read_dir(stages_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        blueprints.push(name.to_string());
                    }
                }
            }
        }
        blueprints.sort();
        blueprints
    }

    /// Read a file from .planning/ relative to root.
    pub fn read_file(&self, relative_path: &str) -> Option<String> {
        let path = self.root.join(relative_path);
        std::fs::read_to_string(path).ok()
    }

    /// Write a file to .planning/ relative to root.
    pub fn write_file(&self, relative_path: &str, content: &str) -> anyhow::Result<()> {
        let path = self.root.join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sets_planning_root() {
        let dir = PlanningDir::new(Path::new("/tmp/myproject"));
        assert_eq!(
            dir.db_path(),
            PathBuf::from("/tmp/myproject/.planning/fin.db")
        );
    }

    #[test]
    fn status_path() {
        let dir = PlanningDir::new(Path::new("/project"));
        assert_eq!(
            dir.status_path(),
            PathBuf::from("/project/.planning/STATUS.md")
        );
    }

    #[test]
    fn vision_path() {
        let dir = PlanningDir::new(Path::new("/project"));
        assert_eq!(
            dir.vision_path(),
            PathBuf::from("/project/.planning/VISION.md")
        );
    }

    #[test]
    fn requirements_path() {
        let dir = PlanningDir::new(Path::new("/project"));
        assert_eq!(
            dir.requirements_path(),
            PathBuf::from("/project/.planning/REQUIREMENTS.md")
        );
    }

    #[test]
    fn stage_dir_blueprint_only() {
        let dir = PlanningDir::new(Path::new("/project"));
        assert_eq!(
            dir.stage_dir("B001", None),
            PathBuf::from("/project/.planning/stages/B001")
        );
    }

    #[test]
    fn stage_dir_with_section() {
        let dir = PlanningDir::new(Path::new("/project"));
        assert_eq!(
            dir.stage_dir("B001", Some("S01")),
            PathBuf::from("/project/.planning/stages/B001/S01")
        );
    }

    #[test]
    fn init_and_list_blueprints() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = PlanningDir::new(tmp.path());
        dir.init().unwrap();

        assert!(dir.exists());
        assert!(dir.list_blueprints().is_empty());

        // Create a stage directory to simulate a blueprint
        dir.create_stage("B001", None).unwrap();
        let blueprints = dir.list_blueprints();
        assert_eq!(blueprints, vec!["B001"]);
    }

    #[test]
    fn read_write_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = PlanningDir::new(tmp.path());
        dir.init().unwrap();

        dir.write_file("test.md", "hello").unwrap();
        assert_eq!(dir.read_file("test.md"), Some("hello".to_string()));
    }

    #[test]
    fn read_nonexistent_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = PlanningDir::new(tmp.path());
        assert_eq!(dir.read_file("nope.md"), None);
    }
}
