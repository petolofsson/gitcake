use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;

use crate::{
    error::AppError,
    git::GitRepo,
    models::{
        config::{RepoConfig, RepoInfo},
        task::{Task, TaskStatus, TaskType},
    },
    task_file,
};

pub struct TaskRepo {
    git: GitRepo,
    pub info: RepoInfo,
}

impl TaskRepo {
    /// Opens and validates a gitcake repo at `path`.
    ///
    /// Requires:
    /// - A git repository (`.git` present)
    /// - `gitcake.toml` at the root
    /// - `git config user.name` configured
    ///
    /// Creates `{username}/` if it does not yet exist.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, AppError> {
        let path = path.into();
        let git = GitRepo::new(&path);

        if !git.is_git_repo() {
            return Err(AppError::InvalidRepo("not a git repository".into()));
        }

        let toml_path = path.join("gitcake.toml");
        if !toml_path.exists() {
            return Err(AppError::InvalidRepo(
                "gitcake.toml not found — is this a gitcake repo?".into(),
            ));
        }

        let toml_str = fs::read_to_string(&toml_path)?;
        let config: RepoConfig = toml::from_str(&toml_str)
            .map_err(|e| AppError::Parse(format!("gitcake.toml: {e}")))?;

        let username = git.get_username()?;

        let info = RepoInfo {
            name: config.name,
            path: path.to_string_lossy().into_owned(),
            username,
        };

        let user_folder = path.join(&info.username);
        if !user_folder.exists() {
            fs::create_dir_all(&user_folder)?;
        }

        Ok(Self { git, info })
    }

    /// Initializes a plain git repo as a gitcake repo.
    ///
    /// Creates `gitcake.toml`, stages it, commits, then opens the repo.
    /// The repo must already exist and have `git config user.name` set.
    /// Does NOT require a remote — that is validated on first sync.
    pub fn init(path: impl Into<PathBuf>, name: &str) -> Result<Self, AppError> {
        let path = path.into();
        let git = GitRepo::new(&path);

        if !git.is_git_repo() {
            return Err(AppError::InvalidRepo("not a git repository".into()));
        }

        let toml_content = format!("name = \"{name}\"\n");
        fs::write(path.join("gitcake.toml"), toml_content)?;

        git.stage("gitcake.toml")?;
        git.commit_staged("init gitcake")?;

        Self::open(path)
    }

    // ── task queries ──────────────────────────────────────────────────────────

    /// Returns all tasks for the current user (active + completed).
    /// Unreadable files are skipped; their filenames appear in the second return value.
    pub fn list_tasks(&self) -> Result<(Vec<Task>, Vec<String>), AppError> {
        task_file::scan_tasks(&self.user_folder(), &self.completed_folder())
    }

    /// Returns active (open + in-progress) tasks for every user folder in the
    /// repo, paired with the folder owner's username. Used for team view.
    pub fn list_team_tasks(&self) -> Result<Vec<(String, Task)>, AppError> {
        let root = Path::new(&self.info.path);
        let users = self.list_users()?;
        let mut result = Vec::new();
        for user in &users {
            let user_folder = root.join(user);
            let completed_folder = root.join("completed").join(user);
            let (tasks, _) = task_file::scan_tasks(&user_folder, &completed_folder)?;
            for task in tasks.into_iter().filter(|t| !t.is_completed && t.status != TaskStatus::Done) {
                result.push((user.clone(), task));
            }
        }
        Ok(result)
    }

    // ── task mutations ────────────────────────────────────────────────────────

    /// Creates a new task file with a unique 8-char hex ID. Status is `open`.
    pub fn create_task(
        &self,
        title: String,
        task_type: TaskType,
        description: Option<String>,
    ) -> Result<Task, AppError> {
        let id = hex_id(&self.user_folder());
        let task = Task {
            id: id.clone(),
            task_type,
            title,
            status: TaskStatus::Open,
            created: Local::now().naive_local(),
            done: None,
            description,
            assignee: None,
            is_completed: false,
        };
        task_file::write_task(&self.task_path(&id), &task)?;
        Ok(task)
    }

    /// Updates the title and/or description of an existing task (active or
    /// completed). Passing `None` for a field leaves it unchanged.
    pub fn update_task(
        &self,
        id: &str,
        title: Option<String>,
        description: Option<String>,
    ) -> Result<Task, AppError> {
        let (path, is_completed) = self.find_task(id)?;
        let mut task = task_file::read_task(&path, is_completed)?;
        if let Some(t) = title {
            task.title = t;
        }
        if description.is_some() {
            task.description = description;
        }
        task_file::write_task(&path, &task)?;
        Ok(task)
    }

    /// Transitions a task to `in-progress`. No-op if already `in-progress`.
    /// If the task has been synced to `completed/`, moves it back via `git mv`.
    pub fn set_task_in_progress(&self, id: &str) -> Result<Task, AppError> {
        let active = self.task_path(id);

        if active.exists() {
            let mut task = task_file::read_task(&active, false)?;
            if task.status != TaskStatus::InProgress {
                task.status = TaskStatus::InProgress;
                task.done = None;
                task_file::write_task(&active, &task)?;
            }
            return Ok(task);
        }

        let completed = self.completed_task_path(id);
        if completed.exists() {
            let from = Path::new("completed")
                .join(&self.info.username)
                .join(format!("{id}.md"));
            let to = PathBuf::from(&self.info.username).join(format!("{id}.md"));
            self.git.stage(&format!("completed/{}/{id}.md", self.info.username))?;
            self.git.move_file(&from, &to)?;
            let mut task = task_file::read_task(&active, false)?;
            task.status = TaskStatus::InProgress;
            task.done = None;
            task_file::write_task(&active, &task)?;
            return Ok(task);
        }

        Err(AppError::TaskNotFound(id.to_string()))
    }

    /// Sets a task's status to `done` and records the `done` timestamp.
    /// The file stays in `{username}/` until the next sync.
    pub fn mark_task_done(&self, id: &str) -> Result<Task, AppError> {
        let path = self.task_path(id);
        if !path.exists() {
            return Err(AppError::TaskNotFound(id.to_string()));
        }
        let mut task = task_file::read_task(&path, false)?;
        task.status = TaskStatus::Done;
        task.done = Some(Local::now().naive_local());
        task_file::write_task(&path, &task)?;
        Ok(task)
    }

    // ── sync ──────────────────────────────────────────────────────────────────

    /// Returns true if the user's folder has uncommitted changes.
    pub fn has_local_changes(&self) -> Result<bool, AppError> {
        self.git.has_local_changes(&self.info.username)
    }

    /// Runs `git pull`.
    pub fn pull(&self) -> Result<String, AppError> {
        self.git.pull()
    }

    /// Moves done tasks to `completed/`, stages all user folder changes,
    /// creates a single commit, and pushes. No pull.
    pub fn push(&self) -> Result<String, AppError> {
        self.move_done_tasks_to_completed()?;
        let msg = format!("gitcake: {}", self.info.username);
        self.git.stage(&self.info.username)?;
        // Only stage completed/ if it exists — avoids git error on first push
        // when no tasks have been completed yet.
        if self.completed_folder().exists() {
            let completed_rel = format!("completed/{}", self.info.username);
            self.git.stage(&completed_rel)?;
        }
        self.git.commit_staged(&msg)?;
        self.git.push()
    }

    /// Full sync: `git pull` → move done tasks → commit → `git push`.
    pub fn sync(&self) -> Result<String, AppError> {
        self.git.pull()?;
        self.push()
    }

    // ── backlog ───────────────────────────────────────────────────────────────

    pub fn list_backlog_tasks(&self) -> Result<(Vec<Task>, Vec<String>), AppError> {
        task_file::scan_folder(&self.backlog_folder())
    }

    pub fn create_backlog_task(
        &self,
        title: String,
        task_type: TaskType,
        description: Option<String>,
    ) -> Result<Task, AppError> {
        let folder = self.backlog_folder();
        fs::create_dir_all(&folder)?;
        let id = hex_id(&folder);
        let task = Task {
            id: id.clone(),
            task_type,
            title,
            status: TaskStatus::Open,
            created: Local::now().naive_local(),
            done: None,
            description,
            assignee: None,
            is_completed: false,
        };
        task_file::write_task(&self.backlog_task_path(&id), &task)?;
        Ok(task)
    }

    pub fn update_backlog_task(
        &self,
        id: &str,
        title: Option<String>,
        description: Option<String>,
    ) -> Result<Task, AppError> {
        let path = self.backlog_task_path(id);
        if !path.exists() {
            return Err(AppError::TaskNotFound(id.to_string()));
        }
        let mut task = task_file::read_task(&path, false)?;
        if let Some(t) = title { task.title = t; }
        if description.is_some() { task.description = description; }
        task_file::write_task(&path, &task)?;
        Ok(task)
    }

    pub fn set_backlog_task_in_progress(&self, id: &str) -> Result<Task, AppError> {
        let path = self.backlog_task_path(id);
        if !path.exists() { return Err(AppError::TaskNotFound(id.to_string())); }
        let mut task = task_file::read_task(&path, false)?;
        if task.status != TaskStatus::InProgress {
            task.status = TaskStatus::InProgress;
            task.done = None;
            task_file::write_task(&path, &task)?;
        }
        Ok(task)
    }

    pub fn mark_backlog_task_done(&self, id: &str) -> Result<Task, AppError> {
        let path = self.backlog_task_path(id);
        if !path.exists() { return Err(AppError::TaskNotFound(id.to_string())); }
        let mut task = task_file::read_task(&path, false)?;
        task.status = TaskStatus::Done;
        task.done = Some(Local::now().naive_local());
        task_file::write_task(&path, &task)?;
        Ok(task)
    }

    /// Moves a backlog task into the current user's personal folder.
    /// The hex ID is preserved — the task keeps its identity when claimed.
    /// Sets status to `open` and assignee to the current user.
    pub fn claim_backlog_task(&self, id: &str) -> Result<Task, AppError> {
        let backlog_path = self.backlog_task_path(id);
        if !backlog_path.exists() {
            return Err(AppError::TaskNotFound(id.to_string()));
        }
        let mut task = task_file::read_task(&backlog_path, false)?;
        task.status = TaskStatus::Open;
        task.done = None;
        task.assignee = Some(self.info.username.clone());
        task_file::write_task(&self.task_path(id), &task)?;
        self.git.stage(&format!("{}/{id}.md", self.info.username))?;
        self.git.remove_tracked(&format!("backlog/{id}.md"))?;
        Ok(task)
    }

    pub fn push_backlog(&self) -> Result<String, AppError> {
        let msg = format!("gitcake: {} (backlog)", self.info.username);
        self.git.stage("backlog")?;
        self.git.commit_staged(&msg)?;
        self.git.push()
    }

    // ── assign ────────────────────────────────────────────────────────────────

    pub fn assign_task(&self, id: &str, assignee: Option<String>) -> Result<Task, AppError> {
        let (path, is_completed) = self.find_task(id)?;
        let mut task = task_file::read_task(&path, is_completed)?;
        task.assignee = assignee;
        task_file::write_task(&path, &task)?;
        Ok(task)
    }

    pub fn assign_backlog_task(&self, id: &str, assignee: Option<String>) -> Result<Task, AppError> {
        let path = self.backlog_task_path(id);
        if !path.exists() { return Err(AppError::TaskNotFound(id.to_string())); }
        if let Some(ref name) = assignee {
            let users = self.list_users()?;
            if !users.contains(name) {
                return Err(AppError::InvalidRepo(format!(
                    "'{name}' is not a known user in this repo"
                )));
            }
        }
        let mut task = task_file::read_task(&path, false)?;
        task.assignee = assignee;
        task_file::write_task(&path, &task)?;
        Ok(task)
    }

    // ── delete ────────────────────────────────────────────────────────────────

    pub fn delete_task(&self, id: &str) -> Result<(), AppError> {
        let path = self.task_path(id);
        if !path.exists() { return Err(AppError::TaskNotFound(id.to_string())); }
        self.git.remove_tracked(&format!("{}/{id}.md", self.info.username))
    }

    pub fn delete_backlog_task(&self, id: &str) -> Result<(), AppError> {
        let path = self.backlog_task_path(id);
        if !path.exists() { return Err(AppError::TaskNotFound(id.to_string())); }
        self.git.remove_tracked(&format!("backlog/{id}.md"))
    }

    // ── users ─────────────────────────────────────────────────────────────────

    /// Returns all user folder names found at the repo root (excluding system dirs).
    pub fn list_users(&self) -> Result<Vec<String>, AppError> {
        let root = Path::new(&self.info.path);
        let mut users: Vec<String> = fs::read_dir(root)?
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| !n.starts_with('.') && n != "backlog" && n != "completed")
            .collect();
        users.sort();
        Ok(users)
    }

    // ── private helpers ───────────────────────────────────────────────────────

    fn user_folder(&self) -> PathBuf {
        Path::new(&self.info.path).join(&self.info.username)
    }

    fn completed_folder(&self) -> PathBuf {
        Path::new(&self.info.path)
            .join("completed")
            .join(&self.info.username)
    }

    fn task_path(&self, id: &str) -> PathBuf {
        self.user_folder().join(format!("{id}.md"))
    }

    fn completed_task_path(&self, id: &str) -> PathBuf {
        self.completed_folder().join(format!("{id}.md"))
    }

    fn backlog_folder(&self) -> PathBuf {
        Path::new(&self.info.path).join("backlog")
    }

    fn backlog_task_path(&self, id: &str) -> PathBuf {
        self.backlog_folder().join(format!("{id}.md"))
    }

    /// Returns the filesystem path for a personal task (active or completed).
    pub fn find_task_file_path(&self, id: &str) -> Option<PathBuf> {
        let active = self.task_path(id);
        if active.exists() { return Some(active); }
        let completed = self.completed_task_path(id);
        if completed.exists() { return Some(completed); }
        None
    }

    /// Returns the filesystem path for a backlog task.
    pub fn backlog_file_path(&self, id: &str) -> Option<PathBuf> {
        let path = self.backlog_task_path(id);
        if path.exists() { Some(path) } else { None }
    }

    /// Finds a task file by ID, checking active folder first then completed.
    fn find_task(&self, id: &str) -> Result<(PathBuf, bool), AppError> {
        let active = self.task_path(id);
        if active.exists() {
            return Ok((active, false));
        }
        let completed = self.completed_task_path(id);
        if completed.exists() {
            return Ok((completed, true));
        }
        Err(AppError::TaskNotFound(id.to_string()))
    }

    /// Moves all done (non-completed) tasks to `completed/{username}/` via
    /// `git mv`. Stages the source file first so git tracks the rename.
    fn move_done_tasks_to_completed(&self) -> Result<(), AppError> {
        let (tasks, _) =
            task_file::scan_tasks(&self.user_folder(), &self.completed_folder())?;
        for task in tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Done && !t.is_completed)
        {
            // Stage the file before git mv so it is tracked in the index.
            let file_rel = format!("{}/{}.md", self.info.username, task.id);
            self.git.stage(&file_rel)?;

            let from =
                PathBuf::from(&self.info.username).join(format!("{}.md", task.id));
            let to = Path::new("completed")
                .join(&self.info.username)
                .join(format!("{}.md", task.id));
            self.git.move_file(&from, &to)?;
        }
        Ok(())
    }
}

/// Generates a unique 8-char hex ID for a task in `folder`.
/// Loops until it finds one not already taken.
fn hex_id(folder: &Path) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let base = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id() as u128;
    let mut n = base ^ (pid * 6_364_136_223_846_793_005);
    loop {
        let id = format!("{:08x}", n & 0xFFFF_FFFF);
        if !folder.join(format!("{id}.md")).exists() {
            return id;
        }
        n = n.wrapping_add(1);
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn git(dir: &Path, args: &[&str]) {
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git command failed");
    }

    fn make_repo(username: &str) -> (TempDir, TaskRepo) {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        git(p, &["init"]);
        git(p, &["config", "user.name", username]);
        git(p, &["config", "user.email", "test@example.com"]);
        fs::write(p.join("gitcake.toml"), "name = \"test\"\n").unwrap();
        git(p, &["add", "."]);
        git(p, &["commit", "-m", "init"]);
        let repo = TaskRepo::open(p).unwrap();
        (dir, repo)
    }

    fn make_repo_with_remote(username: &str) -> (TempDir, TempDir, TaskRepo) {
        let (work_dir, repo) = make_repo(username);
        let bare_dir = TempDir::new().unwrap();
        git(bare_dir.path(), &["init", "--bare"]);
        git(
            work_dir.path(),
            &["remote", "add", "origin", &bare_dir.path().to_string_lossy()],
        );
        git(work_dir.path(), &["push", "-u", "origin", "HEAD"]);
        (work_dir, bare_dir, repo)
    }

    // ── open ──────────────────────────────────────────────────────────────────

    #[test]
    fn open_succeeds_on_valid_repo() {
        let (dir, repo) = make_repo("Alice Smith");
        assert_eq!(repo.info.username, "alice-smith");
        assert_eq!(repo.info.name, "test");
        drop(dir);
    }

    #[test]
    fn open_creates_user_folder() {
        let (dir, repo) = make_repo("Alice Smith");
        assert!(Path::new(&repo.info.path).join("alice-smith").exists());
        drop(dir);
    }

    #[test]
    fn open_fails_on_plain_directory() {
        let dir = TempDir::new().unwrap();
        assert!(matches!(
            TaskRepo::open(dir.path()),
            Err(AppError::InvalidRepo(_))
        ));
    }

    #[test]
    fn open_fails_without_gitcake_toml() {
        let dir = TempDir::new().unwrap();
        git(dir.path(), &["init"]);
        git(dir.path(), &["config", "user.name", "Alice"]);
        assert!(matches!(
            TaskRepo::open(dir.path()),
            Err(AppError::InvalidRepo(_))
        ));
    }

    #[test]
    fn open_fails_if_user_name_not_configured() {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        git(p, &["init"]);
        git(p, &["config", "--local", "user.name", ""]);
        fs::write(p.join("gitcake.toml"), "name = \"test\"\n").unwrap();
        assert!(matches!(
            TaskRepo::open(p),
            Err(AppError::UserNotConfigured)
        ));
    }

    // ── list_tasks ────────────────────────────────────────────────────────────

    #[test]
    fn list_tasks_empty_on_fresh_repo() {
        let (dir, repo) = make_repo("Alice Smith");
        let (tasks, warnings) = repo.list_tasks().unwrap();
        assert!(tasks.is_empty());
        assert!(warnings.is_empty());
        drop(dir);
    }

    #[test]
    fn list_tasks_skips_malformed_file_and_warns() {
        let (dir, repo) = make_repo("Alice Smith");
        repo.create_task("Good task".into(), TaskType::Task, None).unwrap();
        fs::write(
            Path::new(&repo.info.path).join("alice-smith/00000000.md"),
            "not valid frontmatter",
        )
        .unwrap();
        let (tasks, warnings) = repo.list_tasks().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Good task");
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0], "00000000.md");
        drop(dir);
    }

    // ── create_task ───────────────────────────────────────────────────────────

    #[test]
    fn create_task_writes_file_with_correct_fields() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo
            .create_task("Fix login".into(), TaskType::Bug, None)
            .unwrap();

        assert_eq!(task.id.len(), 8);
        assert!(task.id.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(task.title, "Fix login");
        assert_eq!(task.task_type, TaskType::Bug);
        assert_eq!(task.status, TaskStatus::Open);
        assert!(task.done.is_none());
        assert!(Path::new(&repo.info.path)
            .join(format!("alice-smith/{}.md", task.id))
            .exists());
        drop(dir);
    }

    #[test]
    fn create_task_produces_unique_ids() {
        let (dir, repo) = make_repo("Alice Smith");
        let t1 = repo.create_task("First".into(), TaskType::Task, None).unwrap();
        let t2 = repo.create_task("Second".into(), TaskType::Task, None).unwrap();
        assert_eq!(t1.id.len(), 8);
        assert_eq!(t2.id.len(), 8);
        assert_ne!(t1.id, t2.id);
        drop(dir);
    }

    #[test]
    fn create_task_with_description() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo
            .create_task("DB outage".into(), TaskType::Incident, Some("Details here".into()))
            .unwrap();
        assert_eq!(task.description.as_deref(), Some("Details here"));
        drop(dir);
    }

    // ── update_task ───────────────────────────────────────────────────────────

    #[test]
    fn update_task_title() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo.create_task("Old title".into(), TaskType::Task, None).unwrap();
        let updated = repo.update_task(&task.id, Some("New title".into()), None).unwrap();
        assert_eq!(updated.title, "New title");
        drop(dir);
    }

    #[test]
    fn update_task_description() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo.create_task("Task".into(), TaskType::Task, None).unwrap();
        let updated = repo.update_task(&task.id, None, Some("Added description".into())).unwrap();
        assert_eq!(updated.description.as_deref(), Some("Added description"));
        drop(dir);
    }

    #[test]
    fn update_task_not_found_returns_error() {
        let (dir, repo) = make_repo("Alice Smith");
        assert!(matches!(
            repo.update_task("nonexistent", Some("title".into()), None),
            Err(AppError::TaskNotFound(_))
        ));
        drop(dir);
    }

    // ── set_task_in_progress ──────────────────────────────────────────────────

    #[test]
    fn set_task_in_progress_changes_status() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo.create_task("Task".into(), TaskType::Task, None).unwrap();
        let task = repo.set_task_in_progress(&task.id).unwrap();
        assert_eq!(task.status, TaskStatus::InProgress);
        drop(dir);
    }

    #[test]
    fn set_task_in_progress_is_noop_if_already_in_progress() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo.create_task("Task".into(), TaskType::Task, None).unwrap();
        repo.set_task_in_progress(&task.id).unwrap();
        let task = repo.set_task_in_progress(&task.id).unwrap();
        assert_eq!(task.status, TaskStatus::InProgress);
        drop(dir);
    }

    #[test]
    fn set_task_in_progress_cycles_back_from_done() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo.create_task("Task".into(), TaskType::Task, None).unwrap();
        let id = &task.id.clone();
        repo.set_task_in_progress(id).unwrap();
        repo.mark_task_done(id).unwrap();
        let task = repo.set_task_in_progress(id).unwrap();
        assert_eq!(task.status, TaskStatus::InProgress);
        let task = repo.mark_task_done(id).unwrap();
        assert_eq!(task.status, TaskStatus::Done);
        drop(dir);
    }

    #[test]
    fn set_task_in_progress_moves_back_from_completed_after_sync() {
        let (work_dir, bare_dir, repo) = make_repo_with_remote("Alice Smith");
        let task = repo.create_task("Task".into(), TaskType::Task, None).unwrap();
        let id = task.id.clone();
        repo.mark_task_done(&id).unwrap();
        repo.push().unwrap();

        assert!(!Path::new(&repo.info.path).join(format!("alice-smith/{id}.md")).exists());
        assert!(Path::new(&repo.info.path).join(format!("completed/alice-smith/{id}.md")).exists());

        let task = repo.set_task_in_progress(&id).unwrap();
        assert_eq!(task.status, TaskStatus::InProgress);
        assert!(task.done.is_none());
        assert!(Path::new(&repo.info.path).join(format!("alice-smith/{id}.md")).exists());
        assert!(!Path::new(&repo.info.path).join(format!("completed/alice-smith/{id}.md")).exists());

        drop(work_dir);
        drop(bare_dir);
    }

    // ── mark_task_done ────────────────────────────────────────────────────────

    #[test]
    fn mark_task_done_sets_status_and_timestamp() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo.create_task("Task".into(), TaskType::Task, None).unwrap();
        let task = repo.mark_task_done(&task.id).unwrap();
        assert_eq!(task.status, TaskStatus::Done);
        assert!(task.done.is_some());
        drop(dir);
    }

    #[test]
    fn done_task_still_in_user_folder_before_sync() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo.create_task("Task".into(), TaskType::Task, None).unwrap();
        let id = task.id.clone();
        repo.mark_task_done(&id).unwrap();
        assert!(Path::new(&repo.info.path).join(format!("alice-smith/{id}.md")).exists());
        assert!(!Path::new(&repo.info.path).join(format!("completed/alice-smith/{id}.md")).exists());
        drop(dir);
    }

    // ── has_local_changes ─────────────────────────────────────────────────────

    #[test]
    fn has_local_changes_true_after_create() {
        let (dir, repo) = make_repo("Alice Smith");
        repo.create_task("Task".into(), TaskType::Task, None).unwrap();
        assert!(repo.has_local_changes().unwrap());
        drop(dir);
    }

    // ── push ──────────────────────────────────────────────────────────────────

    #[test]
    fn push_moves_done_task_to_completed_and_commits() {
        let (work_dir, bare_dir, repo) = make_repo_with_remote("Alice Smith");
        let task = repo.create_task("Task".into(), TaskType::Task, None).unwrap();
        let id = task.id.clone();
        repo.mark_task_done(&id).unwrap();
        repo.push().unwrap();

        assert!(Path::new(&repo.info.path).join(format!("completed/alice-smith/{id}.md")).exists(), "task should be in completed/");
        assert!(!Path::new(&repo.info.path).join(format!("alice-smith/{id}.md")).exists(), "task should not be in active folder");
        drop(work_dir);
        drop(bare_dir);
    }

    #[test]
    fn push_leaves_open_tasks_in_place() {
        let (work_dir, bare_dir, repo) = make_repo_with_remote("Alice Smith");
        let task = repo.create_task("Open task".into(), TaskType::Task, None).unwrap();
        let id = task.id.clone();
        repo.push().unwrap();
        assert!(Path::new(&repo.info.path).join(format!("alice-smith/{id}.md")).exists());
        drop(work_dir);
        drop(bare_dir);
    }

    // ── sync ──────────────────────────────────────────────────────────────────

    #[test]
    fn sync_pull_then_push() {
        let (work_dir, bare_dir, repo) = make_repo_with_remote("Alice Smith");
        let task = repo.create_task("Task".into(), TaskType::Task, None).unwrap();
        let id = task.id.clone();
        repo.mark_task_done(&id).unwrap();
        repo.sync().unwrap();
        assert!(Path::new(&repo.info.path).join(format!("completed/alice-smith/{id}.md")).exists());
        drop(work_dir);
        drop(bare_dir);
    }

    // ── claim_backlog_task ────────────────────────────────────────────────────

    #[test]
    fn claim_backlog_task_moves_to_personal_folder() {
        let (dir, repo) = make_repo("Alice Smith");
        let backlog = repo.create_backlog_task("Team item".into(), TaskType::Task, None).unwrap();
        let claimed = repo.claim_backlog_task(&backlog.id).unwrap();

        assert_eq!(claimed.id, backlog.id, "hex ID is preserved across claim");
        assert_eq!(claimed.assignee.as_deref(), Some("alice-smith"));
        assert_eq!(claimed.status, TaskStatus::Open);
        assert!(Path::new(&repo.info.path).join(format!("alice-smith/{}.md", backlog.id)).exists());
        assert!(!Path::new(&repo.info.path).join(format!("backlog/{}.md", backlog.id)).exists());
        drop(dir);
    }

    #[test]
    fn claim_backlog_task_preserves_id_regardless_of_personal_tasks() {
        let (dir, repo) = make_repo("Alice Smith");
        repo.create_task("Existing".into(), TaskType::Task, None).unwrap();
        let backlog = repo.create_backlog_task("Team item".into(), TaskType::Task, None).unwrap();
        let claimed = repo.claim_backlog_task(&backlog.id).unwrap();
        assert_eq!(claimed.id, backlog.id);
        drop(dir);
    }

    // ── assign_backlog_task validation ────────────────────────────────────────

    #[test]
    fn assign_backlog_task_rejects_unknown_user() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo.create_backlog_task("Backlog item".into(), TaskType::Task, None).unwrap();
        let result = repo.assign_backlog_task(&task.id, Some("nobody".into()));
        assert!(matches!(result, Err(AppError::InvalidRepo(_))));
        drop(dir);
    }

    #[test]
    fn assign_backlog_task_accepts_known_user() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo.create_backlog_task("Backlog item".into(), TaskType::Task, None).unwrap();
        assert!(repo.assign_backlog_task(&task.id, Some("alice-smith".into())).is_ok());
        drop(dir);
    }

    #[test]
    fn assign_backlog_task_accepts_no_assignee() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo.create_backlog_task("Backlog item".into(), TaskType::Task, None).unwrap();
        assert!(repo.assign_backlog_task(&task.id, None).is_ok());
        drop(dir);
    }
}
