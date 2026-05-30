use serde::{Deserialize, Serialize};

/// Contents of git-task.toml in the task repo root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoConfig {
    pub name: String,
}

/// Runtime info about the connected repo, derived on connect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    /// From git-task.toml
    pub name: String,
    /// Absolute path to the repo root
    pub path: String,
    /// Derived from `git config user.name`: lowercased, spaces → hyphens
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preferences {
    pub theme: Theme,
    pub font_size: u32,
    pub accent_color: String,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            font_size: 14,
            accent_color: "#6366f1".to_string(),
        }
    }
}

/// Persisted across sessions in the Tauri app data directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub repo_path: Option<String>,
    pub preferences: Preferences,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            repo_path: None,
            preferences: Preferences::default(),
        }
    }
}
