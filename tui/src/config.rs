use std::{collections::HashMap, fs, path::PathBuf};

use serde::{Deserialize, Serialize};

/// Named color aliases — referenced in style strings as bare names.
/// Example: `[theme.palette]` with `brand = "#268bd2"`, then use `fg:brand` anywhere.
pub type Palette = HashMap<String, String>;

/// Style strings follow Starship's format: space-separated tokens.
/// Token kinds: `bold` `dim` `italic` `underline`  |  `fg:color`  |  `bg:color`  |  bare color → fg.
/// Colors: ANSI names (`blue`, `cyan` …), hex (`#268bd2`), ANSI index (`21`), palette names, `reset`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    // interaction — style strings
    pub highlight: String,   // e.g. "bold bg:blue fg:white"
    pub accent:    String,   // e.g. "cyan"  (fg only — used for cursor)
    pub warning:   String,   // e.g. "yellow"
    pub danger:    String,   // e.g. "red"
    // base palette — style strings (fg = text color)
    pub text:      String,   // e.g. "reset" or "white"
    pub bg:        String,   // e.g. "reset" or "black"  (drives navbar chip fg)
    pub border:    String,   // e.g. "reset" or "cyan"
    // symbols — single display-cell characters
    pub cursor:       String,
    pub sym_high:     String,
    pub sym_low:      String,
    pub sym_blocked:  String,
    pub sym_done:     String,
    pub sym_open:     String,
    pub sym_progress: String,
    pub sym_arrow:    String,
    pub sym_dot:      String,
    // named color aliases
    #[serde(default)]
    pub palette: Palette,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            highlight: "bold bg:blue fg:white".into(),
            accent:    "cyan".into(),
            warning:   "yellow".into(),
            danger:    "red".into(),
            text:      "reset".into(),
            bg:        "reset".into(),
            border:    "reset".into(),
            cursor:       "▶".into(),
            sym_high:     "^".into(),
            sym_low:      "v".into(),
            sym_blocked:  "!".into(),
            sym_done:     "✓".into(),
            sym_open:     "○".into(),
            sym_progress: "●".into(),
            sym_arrow:    "→".into(),
            sym_dot:      "·".into(),
            palette: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMap {
    pub up: String,
    pub down: String,
    pub detail: String,
    pub back: String,
    pub create: String,
    pub edit: String,
    pub status_cycle: String,
    pub push: String,
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
            push: "ctrl+r".into(),
            quit: "ctrl+q".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub repo_path: Option<String>,
    #[serde(default)]
    pub keys: KeyMap,
    #[serde(default)]
    pub theme: ThemeConfig,
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
