use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Priority {
    High,
    #[default]
    Normal,
    Low,
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
    pub blocked: bool,
    /// Optional sequence number for AI-planned work ordering.
    pub order: Option<u32>,
    /// ID of a parent slice; used to group subtasks under a parent.
    pub parent_id: Option<String>,
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
}

/// Patch applied by `update_task`. `None` means leave the field unchanged.
#[derive(Default)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub priority: Option<Priority>,
    pub blocked: Option<bool>,
    pub order: Option<u32>,
    pub parent_id: Option<String>,
}
