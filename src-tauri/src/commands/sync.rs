use gitcake_core::error::AppError;
use serde::Serialize;
use tauri::State;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct SyncResult {
    pub message: String,
}

#[tauri::command]
pub async fn has_local_changes(_state: State<'_, AppState>) -> Result<bool, AppError> {
    Err(AppError::NotImplemented)
}

#[tauri::command]
pub async fn pull(_state: State<'_, AppState>) -> Result<SyncResult, AppError> {
    Err(AppError::NotImplemented)
}

#[tauri::command]
pub async fn push(_state: State<'_, AppState>) -> Result<SyncResult, AppError> {
    Err(AppError::NotImplemented)
}

#[tauri::command]
pub async fn sync(_state: State<'_, AppState>) -> Result<SyncResult, AppError> {
    Err(AppError::NotImplemented)
}
