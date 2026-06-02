use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub struct Bite {
    pub text: String,
    pub done: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Crumb {
    pub text: String,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TaskType {
    #[default]
    Task,
    Bug,
    Incident,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStatus {
    Open,
    InProgress,
    Done,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub enum Priority {
    Urgent,
    High,
    #[default]
    Normal,
}

impl<'de> serde::Deserialize<'de> for Priority {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match String::deserialize(d)?.as_str() {
            "urgent"         => Ok(Priority::Urgent),
            "high"           => Ok(Priority::High),
            "low"            => Ok(Priority::Normal), // migrate old repos
            _                => Ok(Priority::Normal),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    /// Derived from the parent folder name (tasks/, bugs/, incidents/).
    /// Not stored in frontmatter.
    pub task_type: TaskType,
    pub title: String,
    pub status: TaskStatus,
    pub created: NaiveDateTime,
    pub done: Option<NaiveDateTime>,
    pub description: Option<String>,
    /// Who owns this task. None = unowned (backlog). Replaces both the old
    /// personal-folder ownership model and the assignee field.
    pub owner: Option<String>,
    pub priority: Priority,
    /// Set by AI when it needs human input to continue.
    pub ai_flagged: bool,
    /// Optional sequence number for AI-planned work ordering.
    pub order: Option<u32>,
    /// ID of a parent slice; used to group subtasks under a parent.
    pub parent_id: Option<String>,
    /// ID of the cake (epic) this slice belongs to. Optional.
    pub cake_id: Option<String>,
    /// Parsed from description body at load time. Not stored in frontmatter.
    #[serde(skip)]
    pub bites: Vec<Bite>,
    #[serde(skip)]
    pub crumbs: Vec<Crumb>,
}

/// Arguments for creating a new task. Use `..Default::default()` for optional fields.
#[derive(Default)]
pub struct NewTask {
    pub title: String,
    pub task_type: TaskType,
    pub description: Option<String>,
    pub priority: Priority,
    pub order: Option<u32>,
    pub parent_id: Option<String>,
    pub cake_id: Option<String>,
}

/// Patch applied by `update_task`.
/// - `None` on any field → leave unchanged
/// - `Some(None)` on description / order / parent_id → clear the field
/// - `Some(Some(v))` → set to v
#[derive(Default)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub priority: Option<Priority>,
    pub ai_flagged: Option<bool>,
    pub order: Option<Option<u32>>,
    pub parent_id: Option<Option<String>>,
    pub cake_id: Option<Option<String>>,
}
