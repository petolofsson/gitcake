use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMap {
    pub up: String,
    pub down: String,
    pub detail: String,
    pub back: String,
    pub create: String,
    pub edit: String,
    pub status_cycle: String,
    pub sync: String,
    pub quit: String,
}

impl Default for KeyMap {
    fn default() -> Self {
        Self {
            up: "w".into(),
            down: "s".into(),
            detail: "d".into(),
            back: "a".into(),
            create: "c".into(),
            edit: "e".into(),
            status_cycle: "f".into(),
            sync: "ctrl+r".into(),
            quit: "ctrl+q".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub repo_path: Option<String>,
    #[serde(default)]
    pub keys: KeyMap,
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        let Ok(text) = fs::read_to_string(&path) else {
            return Self::default();
        };
        toml::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(text) = toml::to_string_pretty(self) {
            let _ = fs::write(path, text);
        }
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gitcake")
        .join("config.toml")
}
