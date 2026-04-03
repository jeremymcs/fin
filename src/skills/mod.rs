// Fin + Skills System (Runtime Markdown Loading)

use std::path::Path;

/// A skill is a curated knowledge base loaded from markdown files.
#[derive(Debug, Clone)]
pub struct Skill {
    #[allow(dead_code)]
    pub id: String,
    pub name: String,
    #[allow(dead_code)]
    pub description: String,
    pub content: String,
}

/// Discover and load skills from a directory.
pub fn load_skills(skills_dir: &Path) -> Vec<Skill> {
    let mut skills = Vec::new();

    if !skills_dir.exists() {
        return skills;
    }

    let entries = match std::fs::read_dir(skills_dir) {
        Ok(e) => e,
        Err(_) => return skills,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(skill) = load_skill_dir(&path) {
                skills.push(skill);
            }
        } else if path.extension().is_some_and(|e| e == "md") {
            if let Some(skill) = load_skill_file(&path) {
                skills.push(skill);
            }
        }
    }

    skills
}

fn load_skill_dir(dir: &Path) -> Option<Skill> {
    // Look for AGENTS.md (compiled output) or README.md
    let content_path = ["AGENTS.md", "README.md", "index.md"]
        .iter()
        .map(|f| dir.join(f))
        .find(|p| p.exists())?;

    let content = std::fs::read_to_string(&content_path).ok()?;
    let id = dir.file_name()?.to_str()?.to_string();

    Some(Skill {
        name: id.replace('-', " "),
        description: content.lines().next().unwrap_or("").to_string(),
        id,
        content,
    })
}

fn load_skill_file(path: &Path) -> Option<Skill> {
    let content = std::fs::read_to_string(path).ok()?;
    let id = path.file_stem()?.to_str()?.to_string();

    Some(Skill {
        name: id.replace('-', " "),
        description: content.lines().next().unwrap_or("").to_string(),
        id,
        content,
    })
}
