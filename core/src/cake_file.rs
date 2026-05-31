use std::fs;
use std::path::Path;

use chrono::NaiveDateTime;
use gray_matter::{engine::YAML, Matter};
use serde::Deserialize;

use crate::error::AppError;
use crate::models::cake::Cake;

const DATETIME_FMT: &str = "%Y-%m-%dT%H:%M:%S";

#[derive(Debug, Deserialize)]
struct CakeFrontmatter {
    id: String,
    title: String,
    created: String,
    #[serde(default)]
    owner: Option<String>,
}

pub fn read_cake(path: &Path) -> Result<Cake, AppError> {
    let content = fs::read_to_string(path)?;
    parse_cake_content(&content)
        .map_err(|e| AppError::Parse(format!("{}: {}", path.display(), e)))
}

pub fn write_cake(path: &Path, cake: &Cake) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serialize_cake(cake)).map_err(AppError::from)
}

pub fn scan_cakes(folder: &Path) -> (Vec<Cake>, Vec<String>) {
    let mut cakes = Vec::new();
    let mut warnings = Vec::new();
    if !folder.exists() { return (cakes, warnings); }
    let Ok(entries) = fs::read_dir(folder) else { return (cakes, warnings); };
    let mut paths: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
        .collect();
    paths.sort_by_key(|e| e.file_name());
    for entry in paths {
        match read_cake(&entry.path()) {
            Ok(cake) => cakes.push(cake),
            Err(_)   => warnings.push(entry.file_name().to_string_lossy().into_owned()),
        }
    }
    (cakes, warnings)
}

fn parse_cake_content(content: &str) -> Result<Cake, AppError> {
    let matter = Matter::<YAML>::new();
    let parsed = matter.parse(content);
    let fm: CakeFrontmatter = parsed
        .data
        .ok_or_else(|| AppError::Parse("missing frontmatter".into()))?
        .deserialize()
        .map_err(|e| AppError::Parse(e.to_string()))?;
    let body = parsed.content.trim().to_string();
    let description = if body.is_empty() { None } else { Some(body) };
    let created = NaiveDateTime::parse_from_str(fm.created.trim(), DATETIME_FMT)
        .map_err(|e| AppError::Parse(format!("invalid datetime '{}': {}", fm.created, e)))?;
    Ok(Cake { id: fm.id, title: fm.title, created, owner: fm.owner, description })
}

fn serialize_cake(cake: &Cake) -> String {
    let owner_line = cake.owner.as_deref()
        .map(|o| format!("owner: \"{o}\"\n"))
        .unwrap_or_default();
    let mut out = format!(
        "---\nid: \"{}\"\ntitle: \"{}\"\ncreated: {}\n{}---\n",
        cake.id,
        cake.title.replace('"', "\\\""),
        cake.created.format(DATETIME_FMT),
        owner_line,
    );
    if let Some(desc) = &cake.description {
        out.push('\n');
        out.push_str(desc.trim());
        out.push('\n');
    }
    out
}
