use tauri::{AppHandle, State};

use crate::{
    error::AppError,
    models::config::RepoInfo,
    state::AppState,
};

/// Opens a directory picker and returns the chosen path.
/// The caller should pass the result to `connect_repo`.
#[tauri::command]
pub async fn pick_repo(_app: AppHandle) -> Result<String, AppError> {
    Err(AppError::NotImplemented)
}

/// Validates `path` as a git-task repo, reads git-task.toml, reads
/// `git config user.name`, creates the user folder if absent, and
/// stores the result in AppState. Fails if user.name is not configured.
#[tauri::command]
pub async fn connect_repo(
    _path: String,
    _state: State<'_, AppState>,
) -> Result<RepoInfo, AppError> {
    Err(AppError::NotImplemented)
}

/// Returns the currently connected repo, or None if no repo is connected.
#[tauri::command]
pub async fn get_connected_repo(
    _state: State<'_, AppState>,
) -> Result<Option<RepoInfo>, AppError> {
    Err(AppError::NotImplemented)
}
