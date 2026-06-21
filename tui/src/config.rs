use std::{collections::HashMap, fs, io, path::PathBuf};

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
    pub backlog:      String,
    pub delete:       String,
    pub hide_done:    String,
    pub quit:         String,
}

impl Default for KeyMap {
    fn default() -> Self {
        Self {
            up:           "w".into(),
            down:         "s".into(),
            detail:       "d".into(),
            back:         "a".into(),
            create:       "ctrl+v".into(),
            create_cake:  "shift+v".into(),
            edit:         "e".into(),
            field_cycle:  "f".into(),
            status_cycle: "space".into(),
            push:         "ctrl+t".into(),
            pull:         "shift+t".into(),
            filter:       "q".into(),
            assign:       "ctrl+r".into(),
            backlog:      "ctrl+b".into(),
            delete:       "ctrl+d".into(),
            hide_done:    "H".into(),
            quit:         "ctrl+q".into(),
        }
    }
}

/// Precomputed display glyphs for all key bindings.
/// Built once from KeyMap at startup; avoids per-frame String allocations in render functions.
pub struct NavGlyphs {
    pub nav:             String,  // "W/S"
    pub detail:          String,
    pub create:          String,
    pub create_cake:     String,
    pub filter:          String,
    pub hide_done:       String,
    pub hide_done_col:   String,  // "ASSIGNED [H]" for column header
    pub empty_tasks_msg: String,  // "No tasks yet. ^V to create one."
    pub assign:          String,
    pub backlog:         String,
    pub delete:          String,
    pub pull:            String,
    pub push:            String,
    pub quit:            String,
    pub edit:            String,
    pub field_cycle:     String,
    pub status_cycle:    String,
}

impl NavGlyphs {
    pub fn from_keymap(km: &KeyMap) -> Self {
        let create    = binding_glyph(&km.create);
        let hide_done = binding_glyph(&km.hide_done);
        let empty_tasks_msg = format!("No tasks yet. {create} to create one.");
        let hide_done_col   = format!("ASSIGNED [{}]", hide_done);
        Self {
            nav:          binding_glyph(&km.up) + "/" + &binding_glyph(&km.down),
            detail:       binding_glyph(&km.detail),
            create,
            create_cake:  binding_glyph(&km.create_cake),
            filter:       binding_glyph(&km.filter),
            hide_done,
            hide_done_col,
            empty_tasks_msg,
            assign:       binding_glyph(&km.assign),
            backlog:      binding_glyph(&km.backlog),
            delete:       binding_glyph(&km.delete),
            pull:         binding_glyph(&km.pull),
            push:         binding_glyph(&km.push),
            quit:         binding_glyph(&km.quit),
            edit:         binding_glyph(&km.edit),
            field_cycle:  binding_glyph(&km.field_cycle),
            status_cycle: binding_glyph(&km.status_cycle),
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

/// Convert a binding string to its display glyph: "ctrl+v" → "^V", "shift+v" → "⇧V", "space" → "Spc".
pub fn binding_glyph(binding: &str) -> String {
    if let Some(ctrl_key) = binding.strip_prefix("ctrl+") {
        let ch = ctrl_key.chars().next().unwrap_or('?').to_ascii_uppercase();
        return format!("^{ch}");
    }
    if let Some(shift_key) = binding.strip_prefix("shift+") {
        let ch = shift_key.chars().next().unwrap_or('?').to_ascii_uppercase();
        return format!("⇧{ch}");
    }
    match binding {
        "space"   => "Spc".to_string(),
        "enter"   => "↵".to_string(),
        "tab"     => "⇥".to_string(),
        "backtab" => "⇤".to_string(),
        "esc"     => "Esc".to_string(),
        "up"      => "↑".to_string(),
        "down"    => "↓".to_string(),
        s if s.len() == 1 => s.to_ascii_uppercase(),
        s => s.to_string(),
    }
}

impl Config {
    /// Load config from disk, running migrations if needed.
    /// Returns (config, optional_write_error). On parse failure the file is left untouched.
    pub fn load() -> (Self, Option<String>) {
        let path = config_path();
        let Ok(text) = fs::read_to_string(&path) else {
            return (Self::default(), None);
        };
        // If parsing fails, return defaults without touching the file (auditor N1).
        let Ok(mut c) = toml::from_str::<Self>(&text) else {
            return (Self::default(), None);
        };
        let mut dirty = !text.contains("[theme]");
        // Migrate create off the C key. Both bare "c" (live config) and "ctrl+c" (old default)
        // must be rewritten; bare C conflicts with terminal shortcuts in some environments.
        if c.keys.create == "c" || c.keys.create == "ctrl+c" {
            c.keys.create = "ctrl+v".into();
            dirty = true;
        }
        if c.keys.create_cake == "shift+c" {
            c.keys.create_cake = "shift+v".into();
            dirty = true;
        }
        let warn = if dirty {
            c.save().err().map(|e| format!("config write failed: {e}"))
        } else {
            None
        };
        (c, warn)
    }

    /// Atomically write config to disk. Returns an error if the write fails.
    pub fn save(&self) -> io::Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)
            .map_err(|e| io::Error::other(e.to_string()))?;
        // Write to .tmp then rename for atomicity (avoids corrupt half-written config).
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, text.as_bytes())?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gitcake")
        .join("config.toml")
}
