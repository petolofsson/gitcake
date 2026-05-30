use std::fs;
use std::path::Path;

use chrono::NaiveDateTime;
use gray_matter::{engine::YAML, Matter};
use serde::Deserialize;

use crate::error::AppError;
use crate::models::task::{Task, TaskStatus, TaskType};

const DATETIME_FMT: &str = "%Y-%m-%dT%H:%M:%S";

// ── internal frontmatter representation ──────────────────────────────────────

/// Mirrors the YAML frontmatter fields exactly.
/// Uses `String` for timestamps so YAML type coercion cannot corrupt them.
#[derive(Debug, Deserialize)]
struct Frontmatter {
    id: String,
    #[serde(rename = "type")]
    task_type: TaskType,
    title: String,
    status: TaskStatus,
    created: String,
    #[serde(default)]
    done: Option<String>,
    #[serde(default)]
    assignee: Option<String>,
}

// ── public API ────────────────────────────────────────────────────────────────

/// Parses a single task file. `is_completed` should be `true` when the file
/// lives in `completed/{username}/`.
pub fn read_task(path: &Path, is_completed: bool) -> Result<Task, AppError> {
    let content = fs::read_to_string(path)?;
    parse_task_content(&content, is_completed)
        .map_err(|e| AppError::Parse(format!("{}: {}", path.display(), e)))
}

/// Serializes a `Task` and writes it to `path`, creating parent directories
/// if needed.
pub fn write_task(path: &Path, task: &Task) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serialize_task(task)).map_err(AppError::from)
}

/// Returns all tasks for a user by scanning both their active folder and their
/// completed folder. Missing folders are silently skipped. Unreadable files are
/// also skipped; their filenames are collected in the second return value.
pub fn scan_tasks(user_folder: &Path, completed_folder: &Path) -> Result<(Vec<Task>, Vec<String>), AppError> {
    let mut tasks = Vec::new();
    let mut warnings = Vec::new();
    collect_tasks(user_folder, false, &mut tasks, &mut warnings)?;
    collect_tasks(completed_folder, true, &mut tasks, &mut warnings)?;
    Ok((tasks, warnings))
}

/// Scans a single folder for task files. Used for backlog (no completed/ pair).
/// Unreadable files are skipped; their filenames are collected in the second return value.
pub fn scan_folder(folder: &Path) -> Result<(Vec<Task>, Vec<String>), AppError> {
    let mut tasks = Vec::new();
    let mut warnings = Vec::new();
    collect_tasks(folder, false, &mut tasks, &mut warnings)?;
    Ok((tasks, warnings))
}


// ── parsing ───────────────────────────────────────────────────────────────────

fn parse_task_content(content: &str, is_completed: bool) -> Result<Task, AppError> {
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
        task_type: fm.task_type,
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
        assignee: fm.assignee,
        is_completed,
    })
}

fn parse_dt(s: &str) -> Result<NaiveDateTime, AppError> {
    NaiveDateTime::parse_from_str(s.trim(), DATETIME_FMT)
        .map_err(|e| AppError::Parse(format!("invalid datetime '{}': {}", s, e)))
}

// ── serialization ─────────────────────────────────────────────────────────────

fn serialize_task(task: &Task) -> String {
    let type_str = match task.task_type {
        TaskType::Task => "task",
        TaskType::Bug => "bug",
        TaskType::Incident => "incident",
    };
    let status_str = match task.status {
        TaskStatus::Open => "open",
        TaskStatus::InProgress => "in-progress",
        TaskStatus::Done => "done",
    };
    let done_str = task
        .done
        .map(|d| d.format(DATETIME_FMT).to_string())
        .unwrap_or_default();

    let assignee_line = task
        .assignee
        .as_deref()
        .map(|a| format!("assignee: {}\n", yaml_str(a)))
        .unwrap_or_default();

    let mut out = format!(
        "---\nid: {}\ntype: {}\ntitle: {}\nstatus: {}\ncreated: {}\ndone: {}\n{}---\n",
        yaml_str(&task.id),
        type_str,
        yaml_str(&task.title),
        status_str,
        task.created.format(DATETIME_FMT),
        done_str,
        assignee_line,
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

fn collect_tasks(folder: &Path, is_completed: bool, out: &mut Vec<Task>, warnings: &mut Vec<String>) -> Result<(), AppError> {
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
        match read_task(&entry.path(), is_completed) {
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
        "---\nid: \"001\"\ntype: task\ntitle: Fix login\nstatus: open\ncreated: 2026-05-29T09:14:00\ndone: \n---\n"
    }

    // ── parse ─────────────────────────────────────────────────────────────────

    #[test]
    fn parse_minimal_task() {
        let task = parse_task_content(minimal_task_content(), false).unwrap();
        assert_eq!(task.id, "001");
        assert_eq!(task.task_type, TaskType::Task);
        assert_eq!(task.title, "Fix login");
        assert_eq!(task.status, TaskStatus::Open);
        assert_eq!(task.created, dt("2026-05-29T09:14:00"));
        assert!(task.done.is_none());
        assert!(task.description.is_none());
        assert!(!task.is_completed);
    }

    #[test]
    fn parse_task_with_done_timestamp() {
        let content = "---\nid: \"002\"\ntype: bug\ntitle: Crash on load\nstatus: done\ncreated: 2026-05-29T09:00:00\ndone: 2026-05-29T17:30:00\n---\n";
        let task = parse_task_content(content, false).unwrap();
        assert_eq!(task.status, TaskStatus::Done);
        assert_eq!(task.done, Some(dt("2026-05-29T17:30:00")));
    }

    #[test]
    fn parse_task_with_description() {
        let content = "---\nid: \"003\"\ntype: incident\ntitle: DB outage\nstatus: in-progress\ncreated: 2026-05-29T10:00:00\ndone: \n---\n\nDatabase went down at 10am. Investigating.\n";
        let task = parse_task_content(content, false).unwrap();
        assert_eq!(task.status, TaskStatus::InProgress);
        assert_eq!(
            task.description.as_deref(),
            Some("Database went down at 10am. Investigating.")
        );
    }

    #[test]
    fn parse_is_completed_flag_passed_through() {
        let task = parse_task_content(minimal_task_content(), true).unwrap();
        assert!(task.is_completed);
    }

    #[test]
    fn parse_title_with_special_chars() {
        let content = "---\nid: \"004\"\ntype: task\ntitle: \"Fix \\\"quoted\\\" title\"\nstatus: open\ncreated: 2026-05-29T09:00:00\ndone: \n---\n";
        let task = parse_task_content(content, false).unwrap();
        assert_eq!(task.title, "Fix \"quoted\" title");
    }

    #[test]
    fn parse_errors_on_missing_frontmatter() {
        let result = parse_task_content("no frontmatter here", false);
        assert!(matches!(result, Err(AppError::Parse(_))));
    }

    // ── serialize ─────────────────────────────────────────────────────────────

    #[test]
    fn serialize_round_trip_no_done_no_description() {
        let original = parse_task_content(minimal_task_content(), false).unwrap();
        let serialized = serialize_task(&original);
        let reparsed = parse_task_content(&serialized, false).unwrap();

        assert_eq!(reparsed.id, original.id);
        assert_eq!(reparsed.title, original.title);
        assert_eq!(reparsed.status, original.status);
        assert_eq!(reparsed.created, original.created);
        assert_eq!(reparsed.done, original.done);
        assert_eq!(reparsed.description, original.description);
    }

    #[test]
    fn serialize_round_trip_with_done_and_description() {
        let content = "---\nid: \"005\"\ntype: bug\ntitle: Memory leak\nstatus: done\ncreated: 2026-05-29T08:00:00\ndone: 2026-05-29T12:00:00\n---\n\nFound in the renderer thread.\n";
        let original = parse_task_content(content, false).unwrap();
        let serialized = serialize_task(&original);
        let reparsed = parse_task_content(&serialized, false).unwrap();

        assert_eq!(reparsed.done, original.done);
        assert_eq!(reparsed.description, original.description);
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

        let task = parse_task_content(minimal_task_content(), false).unwrap();
        write_task(&path, &task).unwrap();
        let read_back = read_task(&path, false).unwrap();

        assert_eq!(read_back.id, task.id);
        assert_eq!(read_back.title, task.title);
        assert_eq!(read_back.created, task.created);
    }

    #[test]
    fn write_task_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("alice-smith").join("001.md");
        let task = parse_task_content(minimal_task_content(), false).unwrap();
        write_task(&path, &task).unwrap();
        assert!(path.exists());
    }

    // ── scan_tasks ────────────────────────────────────────────────────────────

    #[test]
    fn scan_returns_empty_for_missing_folders() {
        let dir = TempDir::new().unwrap();
        let (tasks, warnings) = scan_tasks(
            &dir.path().join("alice"),
            &dir.path().join("completed/alice"),
        )
        .unwrap();
        assert!(tasks.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn scan_finds_tasks_in_both_folders() {
        let dir = TempDir::new().unwrap();
        let active = dir.path().join("alice");
        let completed = dir.path().join("completed").join("alice");
        fs::create_dir_all(&active).unwrap();
        fs::create_dir_all(&completed).unwrap();

        let task_a = parse_task_content(minimal_task_content(), false).unwrap();
        write_task(&active.join("001.md"), &task_a).unwrap();

        let content2 = "---\nid: \"002\"\ntype: bug\ntitle: Old bug\nstatus: done\ncreated: 2026-05-28T10:00:00\ndone: 2026-05-28T11:00:00\n---\n";
        let task_b = parse_task_content(content2, true).unwrap();
        write_task(&completed.join("002.md"), &task_b).unwrap();

        let (tasks, warnings) = scan_tasks(&active, &completed).unwrap();
        assert_eq!(tasks.len(), 2);
        assert!(warnings.is_empty());
        assert!(!tasks.iter().find(|t| t.id == "001").unwrap().is_completed);
        assert!(tasks.iter().find(|t| t.id == "002").unwrap().is_completed);
    }

    #[test]
    fn scan_ignores_non_md_files() {
        let dir = TempDir::new().unwrap();
        let active = dir.path().join("alice");
        fs::create_dir_all(&active).unwrap();
        fs::write(active.join("notes.txt"), "not a task").unwrap();
        fs::write(active.join(".gitkeep"), "").unwrap();

        let (tasks, warnings) = scan_tasks(&active, &dir.path().join("completed/alice")).unwrap();
        assert!(tasks.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn scan_skips_malformed_file_and_reports_warning() {
        let dir = TempDir::new().unwrap();
        let active = dir.path().join("alice");
        fs::create_dir_all(&active).unwrap();

        // valid task
        let task = parse_task_content(minimal_task_content(), false).unwrap();
        write_task(&active.join("001.md"), &task).unwrap();

        // malformed task — missing frontmatter
        fs::write(active.join("002.md"), "not valid yaml frontmatter").unwrap();

        let (tasks, warnings) = scan_tasks(&active, &dir.path().join("completed/alice")).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "001");
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0], "002.md");
    }

}
