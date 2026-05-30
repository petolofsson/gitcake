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
}
