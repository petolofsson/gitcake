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
    /// Opens and validates a git-task repo at `path`.
    ///
    /// Requires:
    /// - A git repository (`.git` present)
    /// - `git-task.toml` at the root
    /// - `git config user.name` configured
    ///
    /// Creates `{username}/` if it does not yet exist.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, AppError> {
        let path = path.into();
        let git = GitRepo::new(&path);

        if !git.is_git_repo() {
            return Err(AppError::InvalidRepo("not a git repository".into()));
        }

        let toml_path = path.join("git-task.toml");
        if !toml_path.exists() {
            return Err(AppError::InvalidRepo(
                "git-task.toml not found — is this a git-task repo?".into(),
            ));
        }

        let toml_str = fs::read_to_string(&toml_path)?;
        let config: RepoConfig = toml::from_str(&toml_str)
            .map_err(|e| AppError::Parse(format!("git-task.toml: {e}")))?;

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

    // ── task queries ──────────────────────────────────────────────────────────

    /// Returns all tasks for the current user (active + completed).
    pub fn list_tasks(&self) -> Result<Vec<Task>, AppError> {
        task_file::scan_tasks(&self.user_folder(), &self.completed_folder())
    }

    // ── task mutations ────────────────────────────────────────────────────────

    /// Creates a new task file with the next sequential ID. Status is `open`.
    pub fn create_task(
        &self,
        title: String,
        task_type: TaskType,
        description: Option<String>,
    ) -> Result<Task, AppError> {
        let id = task_file::next_id(&self.user_folder(), &self.completed_folder())?;
        let task = Task {
            id: id.clone(),
            task_type,
            title,
            status: TaskStatus::Open,
            created: Local::now().naive_local(),
            done: None,
            description,
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

    /// Transitions an `open` task to `in-progress`. No-op if already
    /// `in-progress` or `done`.
    pub fn set_task_in_progress(&self, id: &str) -> Result<Task, AppError> {
        let path = self.task_path(id);
        if !path.exists() {
            return Err(AppError::TaskNotFound(id.to_string()));
        }
        let mut task = task_file::read_task(&path, false)?;
        if task.status == TaskStatus::Open {
            task.status = TaskStatus::InProgress;
            task_file::write_task(&path, &task)?;
        }
        Ok(task)
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
        let msg = format!("git-task: {}", self.info.username);
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
        let tasks =
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
        fs::write(p.join("git-task.toml"), "name = \"test\"\n").unwrap();
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
    fn open_fails_without_git_task_toml() {
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
        fs::write(p.join("git-task.toml"), "name = \"test\"\n").unwrap();
        assert!(matches!(
            TaskRepo::open(p),
            Err(AppError::UserNotConfigured)
        ));
    }

    // ── list_tasks ────────────────────────────────────────────────────────────

    #[test]
    fn list_tasks_empty_on_fresh_repo() {
        let (dir, repo) = make_repo("Alice Smith");
        assert!(repo.list_tasks().unwrap().is_empty());
        drop(dir);
    }

    // ── create_task ───────────────────────────────────────────────────────────

    #[test]
    fn create_task_writes_file_with_correct_fields() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo
            .create_task("Fix login".into(), TaskType::Bug, None)
            .unwrap();

        assert_eq!(task.id, "001");
        assert_eq!(task.title, "Fix login");
        assert_eq!(task.task_type, TaskType::Bug);
        assert_eq!(task.status, TaskStatus::Open);
        assert!(task.done.is_none());
        assert!(Path::new(&repo.info.path)
            .join("alice-smith/001.md")
            .exists());
        drop(dir);
    }

    #[test]
    fn create_task_increments_id() {
        let (dir, repo) = make_repo("Alice Smith");
        let t1 = repo.create_task("First".into(), TaskType::Task, None).unwrap();
        let t2 = repo.create_task("Second".into(), TaskType::Task, None).unwrap();
        assert_eq!(t1.id, "001");
        assert_eq!(t2.id, "002");
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
        repo.create_task("Old title".into(), TaskType::Task, None).unwrap();
        let updated = repo
            .update_task("001", Some("New title".into()), None)
            .unwrap();
        assert_eq!(updated.title, "New title");
        drop(dir);
    }

    #[test]
    fn update_task_description() {
        let (dir, repo) = make_repo("Alice Smith");
        repo.create_task("Task".into(), TaskType::Task, None).unwrap();
        let updated = repo
            .update_task("001", None, Some("Added description".into()))
            .unwrap();
        assert_eq!(updated.description.as_deref(), Some("Added description"));
        drop(dir);
    }

    #[test]
    fn update_task_not_found_returns_error() {
        let (dir, repo) = make_repo("Alice Smith");
        assert!(matches!(
            repo.update_task("999", Some("title".into()), None),
            Err(AppError::TaskNotFound(_))
        ));
        drop(dir);
    }

    // ── set_task_in_progress ──────────────────────────────────────────────────

    #[test]
    fn set_task_in_progress_changes_status() {
        let (dir, repo) = make_repo("Alice Smith");
        repo.create_task("Task".into(), TaskType::Task, None).unwrap();
        let task = repo.set_task_in_progress("001").unwrap();
        assert_eq!(task.status, TaskStatus::InProgress);
        drop(dir);
    }

    #[test]
    fn set_task_in_progress_is_noop_if_already_in_progress() {
        let (dir, repo) = make_repo("Alice Smith");
        repo.create_task("Task".into(), TaskType::Task, None).unwrap();
        repo.set_task_in_progress("001").unwrap();
        let task = repo.set_task_in_progress("001").unwrap();
        assert_eq!(task.status, TaskStatus::InProgress);
        drop(dir);
    }

    // ── mark_task_done ────────────────────────────────────────────────────────

    #[test]
    fn mark_task_done_sets_status_and_timestamp() {
        let (dir, repo) = make_repo("Alice Smith");
        repo.create_task("Task".into(), TaskType::Task, None).unwrap();
        let task = repo.mark_task_done("001").unwrap();
        assert_eq!(task.status, TaskStatus::Done);
        assert!(task.done.is_some());
        drop(dir);
    }

    #[test]
    fn done_task_still_in_user_folder_before_sync() {
        let (dir, repo) = make_repo("Alice Smith");
        repo.create_task("Task".into(), TaskType::Task, None).unwrap();
        repo.mark_task_done("001").unwrap();
        assert!(Path::new(&repo.info.path).join("alice-smith/001.md").exists());
        assert!(!Path::new(&repo.info.path)
            .join("completed/alice-smith/001.md")
            .exists());
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
        repo.create_task("Task".into(), TaskType::Task, None).unwrap();
        repo.mark_task_done("001").unwrap();
        repo.push().unwrap();

        let completed = Path::new(&repo.info.path).join("completed/alice-smith/001.md");
        let active = Path::new(&repo.info.path).join("alice-smith/001.md");
        assert!(completed.exists(), "task should be in completed/");
        assert!(!active.exists(), "task should not be in active folder");
        drop(work_dir);
        drop(bare_dir);
    }

    #[test]
    fn push_leaves_open_tasks_in_place() {
        let (work_dir, bare_dir, repo) = make_repo_with_remote("Alice Smith");
        repo.create_task("Open task".into(), TaskType::Task, None).unwrap();
        repo.push().unwrap();

        assert!(Path::new(&repo.info.path).join("alice-smith/001.md").exists());
        drop(work_dir);
        drop(bare_dir);
    }

    // ── sync ──────────────────────────────────────────────────────────────────

    #[test]
    fn sync_pull_then_push() {
        let (work_dir, bare_dir, repo) = make_repo_with_remote("Alice Smith");
        repo.create_task("Task".into(), TaskType::Task, None).unwrap();
        repo.mark_task_done("001").unwrap();
        repo.sync().unwrap();

        assert!(Path::new(&repo.info.path)
            .join("completed/alice-smith/001.md")
            .exists());
        drop(work_dir);
        drop(bare_dir);
    }
}
