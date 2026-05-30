use tauri::State;

use crate::{
    error::AppError,
    models::task::{Task, TaskType},
    state::AppState,
};

/// Returns all tasks for the current user: open + in-progress from
/// {username}/, done from completed/{username}/.
#[tauri::command]
pub async fn list_tasks(_state: State<'_, AppState>) -> Result<Vec<Task>, AppError> {
    Err(AppError::NotImplemented)
}

/// Creates a new task file with the next sequential ID. Requires a
/// successful pull first to avoid ID collisions on multi-machine setups.
#[tauri::command]
pub async fn create_task(
    _title: String,
    _task_type: TaskType,
    _description: Option<String>,
    _state: State<'_, AppState>,
) -> Result<Task, AppError> {
    Err(AppError::NotImplemented)
}

/// Updates the title and/or description of an existing task.
#[tauri::command]
pub async fn update_task(
    _id: String,
    _title: Option<String>,
    _description: Option<String>,
    _state: State<'_, AppState>,
) -> Result<Task, AppError> {
    Err(AppError::NotImplemented)
}

/// Sets a task's status to in-progress and records the started timestamp.
/// No-op if the task is already in-progress or done.
#[tauri::command]
pub async fn set_task_in_progress(
    _id: String,
    _state: State<'_, AppState>,
) -> Result<Task, AppError> {
    Err(AppError::NotImplemented)
}

/// Sets a task's status to done and records the done timestamp.
/// The file stays in {username}/ until the next sync, when it is
/// moved to completed/{username}/ via `git mv`.
#[tauri::command]
pub async fn mark_task_done(
    _id: String,
    _state: State<'_, AppState>,
) -> Result<Task, AppError> {
    Err(AppError::NotImplemented)
}
