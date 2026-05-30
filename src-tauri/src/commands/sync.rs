use serde::Serialize;
use tauri::State;

use crate::{error::AppError, state::AppState};

#[derive(Debug, Serialize)]
pub struct SyncResult {
    pub message: String,
}

/// Returns true if the user's folder has uncommitted changes.
/// Used to decide whether to show the on-exit push prompt.
#[tauri::command]
pub async fn has_local_changes(_state: State<'_, AppState>) -> Result<bool, AppError> {
    Err(AppError::NotImplemented)
}

/// Runs `git pull`. Used for the on-open pull prompt.
#[tauri::command]
pub async fn pull(_state: State<'_, AppState>) -> Result<SyncResult, AppError> {
    Err(AppError::NotImplemented)
}

/// Commits all changes in the user's folder and pushes.
/// No pull. Used for the on-exit push prompt.
/// Done tasks are moved to completed/{username}/ via `git mv` before committing.
#[tauri::command]
pub async fn push(_state: State<'_, AppState>) -> Result<SyncResult, AppError> {
    Err(AppError::NotImplemented)
}

/// Full sync: pull → move done tasks to completed/ → commit user folder → push.
/// Used for the manual sync button.
#[tauri::command]
pub async fn sync(_state: State<'_, AppState>) -> Result<SyncResult, AppError> {
    Err(AppError::NotImplemented)
}
