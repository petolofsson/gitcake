use git_task_core::{
    error::AppError,
    models::task::{Task, TaskType},
};
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub async fn list_tasks(_state: State<'_, AppState>) -> Result<Vec<Task>, AppError> {
    Err(AppError::NotImplemented)
}

#[tauri::command]
pub async fn create_task(
    _title: String,
    _task_type: TaskType,
    _description: Option<String>,
    _state: State<'_, AppState>,
) -> Result<Task, AppError> {
    Err(AppError::NotImplemented)
}

#[tauri::command]
pub async fn update_task(
    _id: String,
    _title: Option<String>,
    _description: Option<String>,
    _state: State<'_, AppState>,
) -> Result<Task, AppError> {
    Err(AppError::NotImplemented)
}

#[tauri::command]
pub async fn set_task_in_progress(
    _id: String,
    _state: State<'_, AppState>,
) -> Result<Task, AppError> {
    Err(AppError::NotImplemented)
}

#[tauri::command]
pub async fn mark_task_done(
    _id: String,
    _state: State<'_, AppState>,
) -> Result<Task, AppError> {
    Err(AppError::NotImplemented)
}
