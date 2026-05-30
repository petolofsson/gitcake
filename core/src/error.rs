use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("No repo connected. Open settings to connect a repo.")]
    NoRepo,
    #[error("git user.name is not configured. Run: git config --global user.name 'Your Name'")]
    UserNotConfigured,
    #[error("git user.name '{0}' contains non-ASCII characters that cannot be used as a folder name. Run: git config --global user.name 'Your-Name'")]
    UsernameInvalid(String),
    #[error("Not a valid git-task repo: {0}")]
    InvalidRepo(String),
    #[error("Git error: {0}")]
    Git(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Task not found: {0}")]
    TaskNotFound(String),
    #[error("Not implemented")]
    NotImplemented,
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}
