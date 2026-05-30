use std::sync::Mutex;

use gitcake_core::models::config::RepoInfo;

use crate::models::config::AppConfig;

pub struct AppState {
    #[allow(dead_code)] // GUI scaffold not yet wired
    pub config: Mutex<AppConfig>,
    #[allow(dead_code)] // GUI scaffold not yet wired
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
