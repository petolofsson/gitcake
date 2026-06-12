use std::{collections::HashMap, fs, path::PathBuf};

use serde::{Deserialize, Serialize};

/// Named color aliases — referenced in style strings as bare names.
pub type Palette = HashMap<String, String>;

/// Style strings follow Starship's format: space-separated tokens.
/// Token kinds: `bold` `dim` `italic` `underline`  |  `fg:color`  |  `bg:color`  |  bare color → fg.
/// Colors: ANSI names (`blue`, `cyan` …), hex (`#268bd2`), ANSI index (`21`), palette names, `reset`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    // selection / interaction
    pub highlight:      String,   // full style for selected row — default "bg:#2a2420"
    pub accent:         String,   // frosting pink — titles, active tab, cursor
    pub in_progress:    String,   // honey — in-progress status, ahead warning
    pub flag:           String,   // raspberry — AI flag ⚑, errors, urgent
    pub priority_color: String,   // caramel — priority + / ++
    // base palette
    pub fg:             String,   // primary text
    pub bg:             String,   // terminal background
    pub dim:            String,   // secondary text, labels, metadata
    pub faint:          String,   // rules, dividers, inactive borders
    pub border:         String,   // panel borders
    pub sel_bg:         String,   // selection / focused-row background tint
    pub done_color:     String,   // pistachio — done status ✓
    pub open_color:     String,   // muted gray — open status ○, tree connectors
    // per-view backgrounds (subtle tinting for orientation)
    pub bg_personal:    String,
    pub bg_planner:     String,
    pub bg_backlog:     String,
    // symbols
    pub cursor:         String,
    pub sym_urgent:     String,
    pub sym_high:       String,
    pub sym_blocked:    String,
    pub sym_done:       String,
    pub sym_open:       String,
    pub sym_progress:   String,
    pub sym_dot:        String,
    #[serde(default)]
    pub palette: Palette,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            highlight:      "bg:#2a2420".into(),
            accent:         "#e6a4b4".into(),
            in_progress:    "#e6b450".into(),
            flag:           "#ea7079".into(),
            priority_color: "#d98c6a".into(),
            fg:             "#efe2d4".into(),
            bg:             "#1b1714".into(),
            dim:            "#9a8a78".into(),
            faint:          "#352d27".into(),
            border:         "#4a4039".into(),
            sel_bg:         "#2a2420".into(),
            done_color:     "#a7c080".into(),
            open_color:     "#6f6258".into(),
            bg_personal:    "#1b1714".into(),
            bg_planner:     "#1b1714".into(),
            bg_backlog:     "#1b1714".into(),
            cursor:         "⇒".into(),
            sym_urgent:     "++".into(),
            sym_high:       "+".into(),
            sym_blocked:    "⚑".into(),
            sym_done:       "✓".into(),
            sym_open:       "○".into(),
            sym_progress:   "●".into(),
            sym_dot:        "·".into(),
            palette:        HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMap {
    pub up:           String,
    pub down:         String,
    pub detail:       String,
    pub back:         String,
    pub create:       String,
    pub create_cake:  String,
    pub edit:         String,
    pub field_cycle:  String,   // cycle focused field value (detail view)
    pub status_cycle: String,   // advance status
    pub push:         String,
    pub pull:         String,
    pub filter:       String,
    pub assign:       String,
    pub add_bite:     String,
    pub quit:         String,
}

impl Default for KeyMap {
    fn default() -> Self {
        Self {
            up:           "w".into(),
            down:         "s".into(),
            detail:       "d".into(),
            back:         "a".into(),
            create:       "c".into(),
            create_cake:  "shift+c".into(),
            edit:         "e".into(),
            field_cycle:  "f".into(),
            status_cycle: "space".into(),
            push:         "t".into(),
            pull:         "shift+t".into(),
            filter:       "q".into(),
            assign:       "r".into(),
            add_bite:     "b".into(),
            quit:         "ctrl+q".into(),
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
        let c: Self = toml::from_str(&text).unwrap_or_default();
        if !text.contains("[theme]") {
            c.save();
        }
        c
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
