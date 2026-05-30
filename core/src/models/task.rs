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
    #[serde(rename = "type")]
    pub task_type: TaskType,
    pub title: String,
    pub status: TaskStatus,
    pub created: NaiveDateTime,
    pub done: Option<NaiveDateTime>,
    pub description: Option<String>,
    pub assignee: Option<String>,
    /// true when the file lives in completed/{username}/
    pub is_completed: bool,
}
