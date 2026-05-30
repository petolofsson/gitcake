use std::sync::Mutex;

use crate::models::config::{AppConfig, RepoInfo};

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
