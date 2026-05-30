use serde::{Deserialize, Serialize};

/// Contents of git-task.toml at the task repo root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoConfig {
    pub name: String,
}

/// Runtime info about the connected repo, derived on connect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    /// From git-task.toml
    pub name: String,
    /// Absolute path to the repo root
    pub path: String,
    /// Derived from `git config user.name`: lowercased, spaces → hyphens
    pub username: String,
}
