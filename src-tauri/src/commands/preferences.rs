use tauri::State;

use crate::{error::AppError, models::config::Preferences, state::AppState};

/// Returns the current preferences. Falls back to defaults if none are persisted.
#[tauri::command]
pub async fn get_preferences(_state: State<'_, AppState>) -> Result<Preferences, AppError> {
    Err(AppError::NotImplemented)
}

/// Persists preferences to the Tauri app data directory.
#[tauri::command]
pub async fn set_preferences(
    _preferences: Preferences,
    _state: State<'_, AppState>,
) -> Result<(), AppError> {
    Err(AppError::NotImplemented)
}
