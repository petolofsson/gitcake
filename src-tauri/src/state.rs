use std::sync::Mutex;

use git_task_core::models::config::RepoInfo;

use crate::models::config::AppConfig;

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub repo: Mutex<Option<RepoInfo>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            config: Mutex::new(AppConfig::default()),
            repo: Mutex::new(None),
        }
    }
}
