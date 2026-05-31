use std::fs;
use std::path::Path;

use chrono::NaiveDateTime;
use gray_matter::{engine::YAML, Matter};
use serde::Deserialize;

use crate::error::AppError;
use crate::models::task::{Priority, Task, TaskStatus, TaskType};

const DATETIME_FMT: &str = "%Y-%m-%dT%H:%M:%S";

// ── internal frontmatter representation ──────────────────────────────────────

/// V2 frontmatter. `type` is omitted — derived from folder name.
#[derive(Debug, Deserialize)]
struct Frontmatter {
    id: String,
    title: String,
    status: TaskStatus,
    created: String,
    #[serde(default)]
    done: Option<String>,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    priority: Priority,
    #[serde(default)]
    blocked: bool,
    #[serde(default)]
    order: Option<u32>,
    #[serde(default)]
    parent: Option<String>,
}

/// V1 frontmatter used only during migration. Reads the old `type:` and `assignee:` fields.
#[derive(Debug, Deserialize)]
pub struct V1Frontmatter {
    pub id: String,
    #[serde(rename = "type", default)]
    pub task_type: TaskType,
    pub title: String,
    pub status: TaskStatus,
    pub created: String,
    #[serde(default)]
    pub done: Option<String>,
    #[serde(default)]
    pub assignee: Option<String>,
}

// ── public API ────────────────────────────────────────────────────────────────

/// Parses a v2 task file. `task_type` is derived from the parent folder name
/// and injected — it is NOT read from the file.
pub fn read_task(path: &Path, task_type: TaskType) -> Result<Task, AppError> {
    let content = fs::read_to_string(path)?;
    parse_task_content(&content, task_type)
        .map_err(|e| AppError::Parse(format!("{}: {}", path.display(), e)))
}

/// Parses a v1 task file for migration purposes. Returns the raw frontmatter
/// so the caller can derive `task_type` from the old `type:` field.
pub fn read_v1_task(path: &Path) -> Result<(V1Frontmatter, Option<String>), AppError> {
    let content = fs::read_to_string(path)?;
    let matter = Matter::<YAML>::new();
    let parsed = matter.parse(&content);
    let fm: V1Frontmatter = parsed
        .data
        .ok_or_else(|| AppError::Parse(format!("{}: missing frontmatter", path.display())))?
        .deserialize()
        .map_err(|e| AppError::Parse(format!("{}: {}", path.display(), e)))?;
    let body = parsed.content.trim().to_string();
    let description = if body.is_empty() { None } else { Some(body) };
    Ok((fm, description))
}

/// Serializes a `Task` and writes it to `path`, creating parent directories if needed.
pub fn write_task(path: &Path, task: &Task) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serialize_task(task)).map_err(AppError::from)
}

/// Scans a type folder (`tasks/`, `bugs/`, `incidents/`) for v2 task files.
/// Unreadable files are skipped; their filenames are collected in the second return value.
pub fn scan_type_folder(folder: &Path, task_type: TaskType) -> Result<(Vec<Task>, Vec<String>), AppError> {
    let mut tasks = Vec::new();
    let mut warnings = Vec::new();
    collect_tasks(folder, task_type, &mut tasks, &mut warnings)?;
    Ok((tasks, warnings))
}

// ── parsing ───────────────────────────────────────────────────────────────────

fn parse_task_content(content: &str, task_type: TaskType) -> Result<Task, AppError> {
    let matter = Matter::<YAML>::new();
    let parsed = matter.parse(content);

    let fm: Frontmatter = parsed
        .data
        .ok_or_else(|| AppError::Parse("missing frontmatter".into()))?
        .deserialize()
        .map_err(|e| AppError::Parse(e.to_string()))?;

    let body = parsed.content.trim().to_string();
    let description = if body.is_empty() { None } else { Some(body) };

    Ok(Task {
        id: fm.id,
        task_type,
        title: fm.title,
        status: fm.status,
        created: parse_dt(&fm.created)?,
        done: fm
            .done
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(parse_dt)
            .transpose()?,
        description,
        owner: fm.owner,
        priority: fm.priority,
        blocked: fm.blocked,
        order: fm.order,
        parent_id: fm.parent,
    })
}

fn parse_dt(s: &str) -> Result<NaiveDateTime, AppError> {
    NaiveDateTime::parse_from_str(s.trim(), DATETIME_FMT)
        .map_err(|e| AppError::Parse(format!("invalid datetime '{}': {}", s, e)))
}

// ── serialization ─────────────────────────────────────────────────────────────

fn serialize_task(task: &Task) -> String {
    let status_str = match task.status {
        TaskStatus::Open => "open",
        TaskStatus::InProgress => "in-progress",
        TaskStatus::Done => "done",
    };
    let done_str = task
        .done
        .map(|d| d.format(DATETIME_FMT).to_string())
        .unwrap_or_default();
    let owner_line = task
        .owner
        .as_deref()
        .map(|o| format!("owner: {}\n", yaml_str(o)))
        .unwrap_or_default();
    let priority_line = match task.priority {
        Priority::Normal => String::new(),
        Priority::High => "priority: high\n".to_string(),
        Priority::Low => "priority: low\n".to_string(),
    };
    let blocked_line = if task.blocked { "blocked: true\n".to_string() } else { String::new() };
    let order_line = task.order.map(|o| format!("order: {o}\n")).unwrap_or_default();
    let parent_line = task.parent_id.as_deref()
        .map(|p| format!("parent: {}\n", yaml_str(p)))
        .unwrap_or_default();

    let mut out = format!(
        "---\nid: {}\ntitle: {}\nstatus: {}\ncreated: {}\ndone: {}\n{}{}{}{}{}---\n",
        yaml_str(&task.id),
        yaml_str(&task.title),
        status_str,
        task.created.format(DATETIME_FMT),
        done_str,
        owner_line,
        priority_line,
        blocked_line,
        order_line,
        parent_line,
    );

    if let Some(desc) = &task.description {
        out.push('\n');
        out.push_str(desc.trim());
        out.push('\n');
    }

    out
}

/// Double-quotes a string for YAML, escaping backslashes and double-quotes.
fn yaml_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn collect_tasks(
    folder: &Path,
    task_type: TaskType,
    out: &mut Vec<Task>,
    warnings: &mut Vec<String>,
) -> Result<(), AppError> {
    if !folder.exists() {
        return Ok(());
    }
    let mut entries: Vec<_> = fs::read_dir(folder)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                == Some("md")
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        match read_task(&entry.path(), task_type.clone()) {
            Ok(task) => out.push(task),
            Err(_) => warnings.push(entry.file_name().to_string_lossy().into_owned()),
        }
    }
    Ok(())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::task::{TaskStatus, TaskType};
    use tempfile::TempDir;

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, DATETIME_FMT).unwrap()
    }

    fn minimal_task_content() -> &'static str {
        "---\nid: \"001\"\ntitle: Fix login\nstatus: open\ncreated: 2026-05-29T09:14:00\ndone: \n---\n"
    }

    // ── parse ─────────────────────────────────────────────────────────────────

    #[test]
    fn parse_minimal_task() {
        let task = parse_task_content(minimal_task_content(), TaskType::Task).unwrap();
        assert_eq!(task.id, "001");
        assert_eq!(task.task_type, TaskType::Task);
        assert_eq!(task.title, "Fix login");
        assert_eq!(task.status, TaskStatus::Open);
        assert_eq!(task.created, dt("2026-05-29T09:14:00"));
        assert!(task.done.is_none());
        assert!(task.description.is_none());
        assert!(task.owner.is_none());
    }

    #[test]
    fn parse_task_with_done_timestamp() {
        let content = "---\nid: \"002\"\ntitle: Crash on load\nstatus: done\ncreated: 2026-05-29T09:00:00\ndone: 2026-05-29T17:30:00\n---\n";
        let task = parse_task_content(content, TaskType::Bug).unwrap();
        assert_eq!(task.task_type, TaskType::Bug);
        assert_eq!(task.status, TaskStatus::Done);
        assert_eq!(task.done, Some(dt("2026-05-29T17:30:00")));
    }

    #[test]
    fn parse_task_with_owner() {
        let content = "---\nid: \"003\"\ntitle: My task\nstatus: in-progress\ncreated: 2026-05-29T09:00:00\ndone: \nowner: \"alice-smith\"\n---\n";
        let task = parse_task_content(content, TaskType::Task).unwrap();
        assert_eq!(task.owner.as_deref(), Some("alice-smith"));
    }

    #[test]
    fn parse_task_with_description() {
        let content = "---\nid: \"003\"\ntitle: DB outage\nstatus: in-progress\ncreated: 2026-05-29T10:00:00\ndone: \n---\n\nDatabase went down at 10am. Investigating.\n";
        let task = parse_task_content(content, TaskType::Incident).unwrap();
        assert_eq!(task.task_type, TaskType::Incident);
        assert_eq!(task.status, TaskStatus::InProgress);
        assert_eq!(
            task.description.as_deref(),
            Some("Database went down at 10am. Investigating.")
        );
    }

    #[test]
    fn parse_title_with_special_chars() {
        let content = "---\nid: \"004\"\ntitle: \"Fix \\\"quoted\\\" title\"\nstatus: open\ncreated: 2026-05-29T09:00:00\ndone: \n---\n";
        let task = parse_task_content(content, TaskType::Task).unwrap();
        assert_eq!(task.title, "Fix \"quoted\" title");
    }

    #[test]
    fn parse_errors_on_missing_frontmatter() {
        let result = parse_task_content("no frontmatter here", TaskType::Task);
        assert!(matches!(result, Err(AppError::Parse(_))));
    }

    // ── serialize ─────────────────────────────────────────────────────────────

    #[test]
    fn serialize_round_trip_no_done_no_description() {
        let original = parse_task_content(minimal_task_content(), TaskType::Task).unwrap();
        let serialized = serialize_task(&original);
        let reparsed = parse_task_content(&serialized, TaskType::Task).unwrap();

        assert_eq!(reparsed.id, original.id);
        assert_eq!(reparsed.title, original.title);
        assert_eq!(reparsed.status, original.status);
        assert_eq!(reparsed.created, original.created);
        assert_eq!(reparsed.done, original.done);
        assert_eq!(reparsed.description, original.description);
    }

    #[test]
    fn serialize_round_trip_with_done_and_description() {
        let content = "---\nid: \"005\"\ntitle: Memory leak\nstatus: done\ncreated: 2026-05-29T08:00:00\ndone: 2026-05-29T12:00:00\n---\n\nFound in the renderer thread.\n";
        let original = parse_task_content(content, TaskType::Bug).unwrap();
        let serialized = serialize_task(&original);
        let reparsed = parse_task_content(&serialized, TaskType::Bug).unwrap();

        assert_eq!(reparsed.done, original.done);
        assert_eq!(reparsed.description, original.description);
    }

    #[test]
    fn serialize_omits_type_field() {
        let original = parse_task_content(minimal_task_content(), TaskType::Task).unwrap();
        let serialized = serialize_task(&original);
        assert!(!serialized.contains("type:"), "type: should not appear in v2 files");
    }

    #[test]
    fn yaml_str_escapes_double_quotes() {
        assert_eq!(yaml_str("say \"hello\""), r#""say \"hello\"""#);
    }

    #[test]
    fn yaml_str_escapes_backslashes() {
        assert_eq!(yaml_str("C:\\path"), r#""C:\\path""#);
    }

    // ── filesystem ───────────────────────────────────────────────────────────

    #[test]
    fn write_and_read_round_trip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("001.md");

        let task = parse_task_content(minimal_task_content(), TaskType::Task).unwrap();
        write_task(&path, &task).unwrap();
        let read_back = read_task(&path, TaskType::Task).unwrap();

        assert_eq!(read_back.id, task.id);
        assert_eq!(read_back.title, task.title);
        assert_eq!(read_back.created, task.created);
    }

    #[test]
    fn write_task_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("tasks").join("001.md");
        let task = parse_task_content(minimal_task_content(), TaskType::Task).unwrap();
        write_task(&path, &task).unwrap();
        assert!(path.exists());
    }

    // ── scan_type_folder ──────────────────────────────────────────────────────

    #[test]
    fn scan_returns_empty_for_missing_folder() {
        let dir = TempDir::new().unwrap();
        let (tasks, warnings) = scan_type_folder(&dir.path().join("tasks"), TaskType::Task).unwrap();
        assert!(tasks.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn scan_finds_tasks_in_folder() {
        let dir = TempDir::new().unwrap();
        let folder = dir.path().join("tasks");
        fs::create_dir_all(&folder).unwrap();

        let task = parse_task_content(minimal_task_content(), TaskType::Task).unwrap();
        write_task(&folder.join("001.md"), &task).unwrap();

        let (tasks, warnings) = scan_type_folder(&folder, TaskType::Task).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].task_type, TaskType::Task);
        assert!(warnings.is_empty());
    }

    #[test]
    fn scan_ignores_non_md_files() {
        let dir = TempDir::new().unwrap();
        let folder = dir.path().join("bugs");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("notes.txt"), "not a task").unwrap();
        fs::write(folder.join(".gitkeep"), "").unwrap();

        let (tasks, warnings) = scan_type_folder(&folder, TaskType::Bug).unwrap();
        assert!(tasks.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn scan_skips_malformed_file_and_reports_warning() {
        let dir = TempDir::new().unwrap();
        let folder = dir.path().join("tasks");
        fs::create_dir_all(&folder).unwrap();

        let task = parse_task_content(minimal_task_content(), TaskType::Task).unwrap();
        write_task(&folder.join("001.md"), &task).unwrap();
        fs::write(folder.join("002.md"), "not valid yaml frontmatter").unwrap();

        let (tasks, warnings) = scan_type_folder(&folder, TaskType::Task).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "001");
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0], "002.md");
    }
}
