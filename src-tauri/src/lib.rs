mod commands;
mod error;
mod models;
mod state;

use commands::{
    preferences::{get_preferences, set_preferences},
    repo::{connect_repo, get_connected_repo, pick_repo},
    sync::{has_local_changes, pull, push, sync},
    tasks::{create_task, list_tasks, mark_task_done, set_task_in_progress, update_task},
};
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            // repo
            pick_repo,
            connect_repo,
            get_connected_repo,
            // tasks
            list_tasks,
            create_task,
            update_task,
            set_task_in_progress,
            mark_task_done,
            // sync
            has_local_changes,
            pull,
            push,
            sync,
            // preferences
            get_preferences,
            set_preferences,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
