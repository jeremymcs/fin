// Fin + Path Resolution (XDG-Compliant)

use std::path::PathBuf;

/// All filesystem paths used by Fin.
pub struct FinPaths {
    /// Base config directory (~/.config/fin or FIN_HOME)
    pub config_dir: PathBuf,
    /// Data directory (~/.local/share/fin)
    pub data_dir: PathBuf,
    /// Sessions directory
    pub sessions_dir: PathBuf,
    /// Extensions directory
    #[allow(dead_code)]
    pub extensions_dir: PathBuf,
    /// Skills directory
    pub skills_dir: PathBuf,
    /// Auth file
    pub auth_file: PathBuf,
    /// Preferences file
    pub preferences_file: PathBuf,
}

impl FinPaths {
    /// Resolve all paths, creating directories as needed.
    pub fn resolve() -> anyhow::Result<Self> {
        let config_dir = std::env::var("FIN_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::config_dir()
                    .unwrap_or_else(|| PathBuf::from("~/.config"))
                    .join("fin")
            });

        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("fin");

        let paths = Self {
            sessions_dir: data_dir.join("sessions"),
            extensions_dir: config_dir.join("extensions"),
            skills_dir: config_dir.join("skills"),
            auth_file: config_dir.join("auth.json"),
            preferences_file: config_dir.join("preferences.toml"),
            config_dir,
            data_dir,
        };

        // Create directories
        std::fs::create_dir_all(&paths.config_dir)?;
        std::fs::create_dir_all(&paths.data_dir)?;
        std::fs::create_dir_all(&paths.sessions_dir)?;

        Ok(paths)
    }

    /// Get project-specific .fin/ workflow directory.
    #[allow(dead_code)]
    pub fn fin_dir(project_root: &std::path::Path) -> PathBuf {
        project_root.join(".fin")
    }

    /// Get project database path.
    #[allow(dead_code)]
    pub fn project_db(project_root: &std::path::Path) -> PathBuf {
        project_root.join(".fin").join("fin.db")
    }
}
