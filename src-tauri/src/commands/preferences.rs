use gitcake_core::error::AppError;
use tauri::State;

use crate::{models::config::Preferences, state::AppState};

#[tauri::command]
pub async fn get_preferences(_state: State<'_, AppState>) -> Result<Preferences, AppError> {
    Err(AppError::NotImplemented)
}

#[tauri::command]
pub async fn set_preferences(
    _preferences: Preferences,
    _state: State<'_, AppState>,
) -> Result<(), AppError> {
    Err(AppError::NotImplemented)
}
