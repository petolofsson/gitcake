use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub repo_path: Option<String>,
    pub preferences: Preferences,
}
