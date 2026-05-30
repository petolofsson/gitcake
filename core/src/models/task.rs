use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum TaskType {
    Task,
    Bug,
    Incident,
}

impl Default for TaskType {
    fn default() -> Self {
        TaskType::Task
    }
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
    /// true when the file lives in completed/{username}/
    pub is_completed: bool,
}
