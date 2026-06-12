use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;

use crate::{
    cake_file,
    error::AppError,
    git::GitRepo,
    models::{
        cake::{Cake, NewCake},
        config::{RepoConfig, RepoInfo},
        task::{NewTask, Task, TaskPatch, TaskStatus, TaskType},
    },
    task_file,
};

pub struct TaskRepo {
    git: GitRepo,
    pub info: RepoInfo,
}

impl TaskRepo {
    /// Opens a gitcake v2 repo. Auto-migrates v1 repos on first open.
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

        // Auto-migrate v1 → v2: detect by presence of personal folder and absence of type folders
        if path.join(&info.username).is_dir() && !path.join("tasks").exists() {
            migrate_v1_to_v2(&path, &git)?;
        }

        Ok(Self { git, info })
    }

    /// Initializes a plain git repo as a gitcake repo.
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

    /// Returns all tasks owned by the current user.
    pub fn list_tasks(&self) -> Result<(Vec<Task>, Vec<String>), AppError> {
        let root = Path::new(&self.info.path);
        let (all, warnings) = scan_all(root);
        let mine = all
            .into_iter()
            .filter(|t| t.owner.as_deref() == Some(&self.info.username))
            .collect();
        Ok((mine, warnings))
    }

    /// Returns all unowned tasks (the shared backlog).
    pub fn list_backlog_tasks(&self) -> Result<(Vec<Task>, Vec<String>), AppError> {
        let root = Path::new(&self.info.path);
        let (all, warnings) = scan_all(root);
        let backlog = all.into_iter().filter(|t| t.owner.is_none()).collect();
        Ok((backlog, warnings))
    }

    /// Returns all owned tasks grouped by owner. Used for planner view.
    pub fn list_team_tasks(&self) -> Result<Vec<(String, Task)>, AppError> {
        let root = Path::new(&self.info.path);
        let (all, _) = scan_all(root);
        let result = all
            .into_iter()
            .filter(|t| t.owner.is_some())
            .map(|t| (t.owner.clone().expect("owner is Some — filtered above"), t))
            .collect();
        Ok(result)
    }

    pub fn list_all_tasks(&self) -> Result<Vec<Task>, AppError> {
        let (all, _) = scan_all(Path::new(&self.info.path));
        Ok(all)
    }

    // ── task mutations ────────────────────────────────────────────────────────

    /// Creates a new personal task in the appropriate type folder.
    pub fn create_task(&self, args: NewTask) -> Result<Task, AppError> {
        let root = Path::new(&self.info.path);
        let folder = type_folder(root, &args.task_type);
        fs::create_dir_all(&folder)?;
        let id = hex_id(&folder);
        let task = Task {
            id: id.clone(),
            task_type: args.task_type,
            title: args.title,
            status: TaskStatus::Open,
            created: Local::now().naive_local(),
            done: None,
            description: args.description,
            owner: Some(self.info.username.clone()),
            priority: args.priority,
            ai_flagged: false,
            order: args.order,
            parent_id: args.parent_id,
            cake_id: args.cake_id,
            bites: Vec::new(),
            crumbs: Vec::new(),
        };
        task_file::write_task(&folder.join(format!("{id}.md")), &task)?;
        Ok(task)
    }

    /// Creates an unowned (backlog) task in the appropriate type folder.
    pub fn create_backlog_task(&self, args: NewTask) -> Result<Task, AppError> {
        let root = Path::new(&self.info.path);
        let folder = type_folder(root, &args.task_type);
        fs::create_dir_all(&folder)?;
        let id = hex_id(&folder);
        let task = Task {
            id: id.clone(),
            task_type: args.task_type,
            title: args.title,
            status: TaskStatus::Open,
            created: Local::now().naive_local(),
            done: None,
            description: args.description,
            owner: None,
            priority: args.priority,
            ai_flagged: false,
            order: args.order,
            parent_id: args.parent_id,
            cake_id: args.cake_id,
            bites: Vec::new(),
            crumbs: Vec::new(),
        };
        task_file::write_task(&folder.join(format!("{id}.md")), &task)?;
        Ok(task)
    }

    /// Applies a patch to any task. `None` fields in the patch are left unchanged.
    pub fn update_task(&self, id: &str, patch: TaskPatch) -> Result<Task, AppError> {
        let root = Path::new(&self.info.path);
        let (path, task_type) = find_task_path(root, id)?;
        let mut task = task_file::read_task(&path, task_type)?;
        if let Some(t) = patch.title       { task.title       = t; }
        if let Some(d) = patch.description { task.description = d; }
        if let Some(p) = patch.priority    { task.priority    = p; }
        if let Some(b) = patch.ai_flagged     { task.ai_flagged = b; }
        if let Some(o) = patch.order       { task.order       = o; }
        if let Some(p) = patch.parent_id   { task.parent_id   = p; }
        if let Some(c) = patch.cake_id     { task.cake_id     = c; }
        task_file::write_task(&path, &task)?;
        Ok(task)
    }

    /// Alias for update_task — backlog tasks live in the same type folders.
    pub fn update_backlog_task(&self, id: &str, patch: TaskPatch) -> Result<Task, AppError> {
        self.update_task(id, patch)
    }

    /// Sets a task to in-progress, clearing the done timestamp.
    pub fn set_task_in_progress(&self, id: &str) -> Result<Task, AppError> {
        let root = Path::new(&self.info.path);
        let (path, task_type) = find_task_path(root, id)?;
        let mut task = task_file::read_task(&path, task_type)?;
        task.status = TaskStatus::InProgress;
        task.done = None;
        task_file::write_task(&path, &task)?;
        Ok(task)
    }

    /// Alias for consistency — all tasks are in type folders now.
    pub fn set_backlog_task_in_progress(&self, id: &str) -> Result<Task, AppError> {
        self.set_task_in_progress(id)
    }

    /// Sets a task's status to done with a timestamp. The file stays in place.
    pub fn mark_task_done(&self, id: &str) -> Result<Task, AppError> {
        let root = Path::new(&self.info.path);
        let (path, task_type) = find_task_path(root, id)?;
        let mut task = task_file::read_task(&path, task_type)?;
        task.status = TaskStatus::Done;
        task.done = Some(Local::now().naive_local());
        task_file::write_task(&path, &task)?;
        Ok(task)
    }

    /// Alias for consistency.
    pub fn mark_backlog_task_done(&self, id: &str) -> Result<Task, AppError> {
        self.mark_task_done(id)
    }

    // ── sync ──────────────────────────────────────────────────────────────────

    /// Returns true if any type folder has uncommitted changes.
    pub fn has_local_changes(&self) -> Result<bool, AppError> {
        for folder in ["tasks", "bugs", "incidents"] {
            if self.git.has_local_changes(folder)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Runs `git pull`.
    pub fn pull(&self) -> Result<String, AppError> {
        self.git.pull()
    }

    /// Returns (commits ahead of remote, last push relative time), or None if no upstream.
    pub fn git_status(&self) -> Option<(u32, String)> {
        self.git.ahead_status()
    }

    /// Stages all type folders, commits, and pushes.
    pub fn push(&self) -> Result<String, AppError> {
        let root = Path::new(&self.info.path);
        let msg = format!("gitcake: {}", self.info.username);
        for folder in ["tasks", "bugs", "incidents", "cakes"] {
            if root.join(folder).exists() {
                self.git.stage(folder)?;
            }
        }
        self.git.commit_staged(&msg)?;
        self.git.push()
    }

    // ── cake operations ───────────────────────────────────────────────────────

    pub fn list_cakes(&self) -> Result<Vec<Cake>, AppError> {
        let folder = Path::new(&self.info.path).join("cakes");
        let (cakes, _) = cake_file::scan_cakes(&folder);
        Ok(cakes)
    }

    pub fn create_cake(&self, args: NewCake) -> Result<Cake, AppError> {
        let folder = Path::new(&self.info.path).join("cakes");
        fs::create_dir_all(&folder)?;
        let id = hex_id(&folder);
        let cake = Cake {
            id: id.clone(),
            title: args.title,
            created: Local::now().naive_local(),
            owner: args.owner,
            description: args.description,
        };
        cake_file::write_cake(&folder.join(format!("{id}.md")), &cake)?;
        Ok(cake)
    }

    pub fn get_cake(&self, id: &str) -> Result<Cake, AppError> {
        let path = Path::new(&self.info.path).join("cakes").join(format!("{id}.md"));
        cake_file::read_cake(&path)
    }

    /// Alias — backlog lives in the same type folders.
    pub fn push_backlog(&self) -> Result<String, AppError> {
        self.push()
    }

    /// Full sync: pull → commit type folders → push.
    pub fn sync(&self) -> Result<String, AppError> {
        self.git.pull()?;
        self.push()
    }

    // ── claim / backlog ───────────────────────────────────────────────────────

    /// Claims an unowned task by setting its owner to the current user.
    /// No file move — the task stays in its type folder.
    pub fn claim_backlog_task(&self, id: &str) -> Result<Task, AppError> {
        let root = Path::new(&self.info.path);
        let (path, task_type) = find_task_path(root, id)?;
        let mut task = task_file::read_task(&path, task_type)?;
        if task.owner.is_some() {
            return Err(AppError::InvalidRepo("task is already owned".into()));
        }
        task.owner = Some(self.info.username.clone());
        task.status = TaskStatus::Open;
        task.done = None;
        task_file::write_task(&path, &task)?;
        let rel = rel_path(root, &path);
        self.git.stage(&rel)?;
        Ok(task)
    }

    /// Releases a personal task back to the backlog by clearing its owner.
    pub fn move_task_to_backlog(&self, id: &str) -> Result<(), AppError> {
        let root = Path::new(&self.info.path);
        let (path, task_type) = find_task_path(root, id)?;
        let mut task = task_file::read_task(&path, task_type)?;
        task.owner = None;
        task.status = TaskStatus::Open;
        task.done = None;
        task_file::write_task(&path, &task)?;
        let rel = rel_path(root, &path);
        self.git.stage(&rel)?;
        Ok(())
    }

    // ── assign ────────────────────────────────────────────────────────────────

    /// Assigns (delegates) a task by setting a new owner. No file move.
    pub fn assign_task(&self, id: &str, owner: Option<String>) -> Result<Task, AppError> {
        let root = Path::new(&self.info.path);
        let (path, task_type) = find_task_path(root, id)?;
        let mut task = task_file::read_task(&path, task_type)?;
        task.owner = owner;
        task_file::write_task(&path, &task)?;
        let rel = rel_path(root, &path);
        self.git.stage(&rel)?;
        Ok(task)
    }

    // ── delete ────────────────────────────────────────────────────────────────

    /// Deletes a task permanently (git rm).
    pub fn delete_task(&self, id: &str) -> Result<(), AppError> {
        let root = Path::new(&self.info.path);
        let (path, _) = find_task_path(root, id)?;
        let rel = rel_path(root, &path);
        self.git.remove_tracked(&rel)
    }

    /// Alias — all tasks are in type folders now.
    pub fn delete_backlog_task(&self, id: &str) -> Result<(), AppError> {
        self.delete_task(id)
    }

    // ── users ─────────────────────────────────────────────────────────────────

    /// Returns all known usernames by scanning owner fields across all type folders.
    /// Always includes the current user.
    pub fn list_users(&self) -> Result<Vec<String>, AppError> {
        let root = Path::new(&self.info.path);
        let (all, _) = scan_all(root);
        let mut users: HashSet<String> = all.into_iter().filter_map(|t| t.owner).collect();
        users.insert(self.info.username.clone());
        let mut sorted: Vec<String> = users.into_iter().collect();
        sorted.sort();
        Ok(sorted)
    }

    /// Fetches a single task by ID.
    pub fn get_task(&self, id: &str) -> Result<Task, AppError> {
        let root = Path::new(&self.info.path);
        let (path, task_type) = find_task_path(root, id)?;
        task_file::read_task(&path, task_type)
    }

    /// Moves a task to a different type folder, preserving all other fields.
    pub fn change_task_type(&self, id: &str, new_type: TaskType) -> Result<Task, AppError> {
        let root = Path::new(&self.info.path);
        let (old_path, old_type) = find_task_path(root, id)?;
        if old_type == new_type {
            return task_file::read_task(&old_path, old_type);
        }
        let new_folder = type_folder(root, &new_type);
        fs::create_dir_all(&new_folder)?;
        let new_path = new_folder.join(format!("{id}.md"));
        let old_rel = rel_path(root, &old_path);
        let new_rel = rel_path(root, &new_path);
        self.git.move_file(Path::new(&old_rel), Path::new(&new_rel))?;
        task_file::read_task(&new_path, new_type)
    }

    // ── path helpers (for TUI detail view) ───────────────────────────────────

    pub fn find_task_file_path(&self, id: &str) -> Option<PathBuf> {
        find_task_path(Path::new(&self.info.path), id).ok().map(|(p, _)| p)
    }
}

// ── module-level helpers ──────────────────────────────────────────────────────

pub fn type_folder(root: &Path, t: &TaskType) -> PathBuf {
    match t {
        TaskType::Task => root.join("tasks"),
        TaskType::Bug => root.join("bugs"),
        TaskType::Incident => root.join("incidents"),
    }
}

pub fn type_from_folder_name(name: &str) -> Option<TaskType> {
    match name {
        "tasks" => Some(TaskType::Task),
        "bugs" => Some(TaskType::Bug),
        "incidents" => Some(TaskType::Incident),
        _ => None,
    }
}

fn scan_all(root: &Path) -> (Vec<Task>, Vec<String>) {
    let mut tasks = Vec::new();
    let mut warnings = Vec::new();
    for (name, task_type) in [
        ("tasks", TaskType::Task),
        ("bugs", TaskType::Bug),
        ("incidents", TaskType::Incident),
    ] {
        if let Ok((mut t, mut w)) = task_file::scan_type_folder(&root.join(name), task_type) {
            tasks.append(&mut t);
            warnings.append(&mut w);
        }
    }
    (tasks, warnings)
}

fn find_task_path(root: &Path, id: &str) -> Result<(PathBuf, TaskType), AppError> {
    for (name, task_type) in [
        ("tasks", TaskType::Task),
        ("bugs", TaskType::Bug),
        ("incidents", TaskType::Incident),
    ] {
        let path = root.join(name).join(format!("{id}.md"));
        if path.exists() {
            return Ok((path, task_type));
        }
    }
    Err(AppError::TaskNotFound(id.to_string()))
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

/// Generates a unique 8-char hex ID for a task in `folder`.
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

// ── v1 → v2 migration ────────────────────────────────────────────────────────

fn migrate_v1_to_v2(root: &Path, git: &GitRepo) -> Result<(), AppError> {
    let reserved = ["tasks", "bugs", "incidents", "backlog", "completed", ".git"];

    // Find all v1 user folders
    let user_dirs: Vec<String> = fs::read_dir(root)?
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| !n.starts_with('.') && !reserved.contains(&n.as_str()))
        .collect();

    // Migrate personal tasks for each user
    for user in &user_dirs {
        migrate_folder(root, &root.join(user), Some(user), git)?;
        let completed = root.join("completed").join(user);
        if completed.is_dir() {
            migrate_folder(root, &completed, Some(user), git)?;
        }
    }

    // Migrate backlog
    let backlog = root.join("backlog");
    if backlog.is_dir() {
        migrate_folder(root, &backlog, None, git)?;
    }

    // Remove old folders via git rm + fs cleanup
    for user in &user_dirs {
        let _ = git.remove_tracked(user);
        let _ = fs::remove_dir_all(root.join(user));
        let completed_rel = format!("completed/{user}");
        let completed_full = root.join("completed").join(user);
        if completed_full.exists() {
            let _ = git.remove_tracked(&completed_rel);
            let _ = fs::remove_dir_all(&completed_full);
        }
    }
    let completed_dir = root.join("completed");
    if completed_dir.exists() {
        let _ = fs::remove_dir(&completed_dir);
    }
    if backlog.exists() {
        let _ = git.remove_tracked("backlog");
        let _ = fs::remove_dir_all(&backlog);
    }

    git.commit_staged("gitcake: migrate to v2 storage layout")?;
    Ok(())
}

fn migrate_folder(
    root: &Path,
    folder: &Path,
    owner: Option<&str>,
    git: &GitRepo,
) -> Result<(), AppError> {
    if !folder.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(folder)?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|x| x.to_str()) != Some("md") {
            continue;
        }
        let Ok((fm, description)) = task_file::read_v1_task(&path) else {
            continue;
        };
        let dest_folder = type_folder(root, &fm.task_type);
        fs::create_dir_all(&dest_folder)?;
        let file_name = path.file_name()
            .ok_or_else(|| AppError::Parse(format!("invalid path during migration: {}", path.display())))?;
        let dest = dest_folder.join(file_name);
        if dest.exists() {
            continue; // already migrated by another user's pass
        }
        let done = fm.done.as_deref().filter(|s| !s.is_empty()).and_then(|s| {
            chrono::NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%dT%H:%M:%S").ok()
        });
        let task = Task {
            id: fm.id,
            task_type: fm.task_type,
            title: fm.title,
            status: fm.status,
            created: chrono::NaiveDateTime::parse_from_str(fm.created.trim(), "%Y-%m-%dT%H:%M:%S")
                .unwrap_or_else(|_| Local::now().naive_local()),
            done,
            description,
            owner: owner.map(|s| s.to_string()),
            priority: Default::default(),
            ai_flagged: false,
            order: None,
            parent_id: None,
            cake_id: None,
            bites: Vec::new(),
            crumbs: Vec::new(),
        };
        task_file::write_task(&dest, &task)?;
        let rel = rel_path(root, &dest);
        git.stage(&rel)?;
    }
    Ok(())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn git_cmd(dir: &Path, args: &[&str]) {
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git command failed");
    }

    fn make_repo(username: &str) -> (TempDir, TaskRepo) {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        git_cmd(p, &["init"]);
        git_cmd(p, &["config", "user.name", username]);
        git_cmd(p, &["config", "user.email", "test@example.com"]);
        fs::write(p.join("gitcake.toml"), "name = \"test\"\n").unwrap();
        git_cmd(p, &["add", "."]);
        git_cmd(p, &["commit", "-m", "init"]);
        let repo = TaskRepo::open(p).unwrap();
        (dir, repo)
    }

    fn make_repo_with_remote(username: &str) -> (TempDir, TempDir, TaskRepo) {
        let (work_dir, repo) = make_repo(username);
        let bare_dir = TempDir::new().unwrap();
        git_cmd(bare_dir.path(), &["init", "--bare"]);
        git_cmd(
            work_dir.path(),
            &["remote", "add", "origin", &bare_dir.path().to_string_lossy()],
        );
        git_cmd(work_dir.path(), &["push", "-u", "origin", "HEAD"]);
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
    fn open_no_personal_folder_created() {
        let (dir, repo) = make_repo("Alice Smith");
        assert!(!Path::new(&repo.info.path).join("alice-smith").exists(),
            "v2 should not create personal folders");
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
        git_cmd(dir.path(), &["init"]);
        git_cmd(dir.path(), &["config", "user.name", "Alice"]);
        assert!(matches!(
            TaskRepo::open(dir.path()),
            Err(AppError::InvalidRepo(_))
        ));
    }

    #[test]
    fn open_fails_if_user_name_not_configured() {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        git_cmd(p, &["init"]);
        git_cmd(p, &["config", "--local", "user.name", ""]);
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
    fn list_tasks_returns_only_owned_tasks() {
        let (dir, repo) = make_repo("Alice Smith");
        repo.create_task(NewTask { title: "Mine".into(), task_type: TaskType::Task, ..Default::default() }).unwrap();
        repo.create_backlog_task(NewTask { title: "Unowned".into(), task_type: TaskType::Bug, ..Default::default() }).unwrap();
        let (tasks, _) = repo.list_tasks().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Mine");
        assert_eq!(tasks[0].owner.as_deref(), Some("alice-smith"));
        drop(dir);
    }

    // ── list_backlog_tasks ────────────────────────────────────────────────────

    #[test]
    fn list_backlog_returns_only_unowned_tasks() {
        let (dir, repo) = make_repo("Alice Smith");
        repo.create_task(NewTask { title: "Mine".into(), task_type: TaskType::Task, ..Default::default() }).unwrap();
        repo.create_backlog_task(NewTask { title: "Shared".into(), task_type: TaskType::Bug, ..Default::default() }).unwrap();
        let (backlog, _) = repo.list_backlog_tasks().unwrap();
        assert_eq!(backlog.len(), 1);
        assert_eq!(backlog[0].title, "Shared");
        assert!(backlog[0].owner.is_none());
        drop(dir);
    }

    // ── create_task ───────────────────────────────────────────────────────────

    #[test]
    fn create_task_writes_to_type_folder() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo.create_task(NewTask { title: "Fix login".into(), task_type: TaskType::Bug, ..Default::default() }).unwrap();

        assert_eq!(task.id.len(), 8);
        assert!(task.id.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(task.task_type, TaskType::Bug);
        assert_eq!(task.owner.as_deref(), Some("alice-smith"));
        assert!(Path::new(&repo.info.path)
            .join(format!("bugs/{}.md", task.id))
            .exists());
        // No personal folder created
        assert!(!Path::new(&repo.info.path).join("alice-smith").exists());
        drop(dir);
    }

    #[test]
    fn create_backlog_task_has_no_owner() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo.create_backlog_task(NewTask { title: "Shared".into(), task_type: TaskType::Task, ..Default::default() }).unwrap();
        assert!(task.owner.is_none());
        assert!(Path::new(&repo.info.path)
            .join(format!("tasks/{}.md", task.id))
            .exists());
        drop(dir);
    }

    // ── claim ─────────────────────────────────────────────────────────────────

    #[test]
    fn claim_sets_owner_no_file_move() {
        let (dir, repo) = make_repo("Alice Smith");
        let backlog = repo.create_backlog_task(NewTask { title: "Team item".into(), task_type: TaskType::Task, ..Default::default() }).unwrap();
        let id = backlog.id.clone();
        let original_path = Path::new(&repo.info.path).join(format!("tasks/{id}.md"));
        assert!(original_path.exists());

        let claimed = repo.claim_backlog_task(&id).unwrap();
        assert_eq!(claimed.owner.as_deref(), Some("alice-smith"));
        assert_eq!(claimed.status, TaskStatus::Open);
        // File stays in same location
        assert!(original_path.exists(), "file should stay in tasks/ after claim");
        drop(dir);
    }

    #[test]
    fn claim_fails_if_already_owned() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo.create_task(NewTask { title: "Mine".into(), task_type: TaskType::Task, ..Default::default() }).unwrap();
        assert!(matches!(
            repo.claim_backlog_task(&task.id),
            Err(AppError::InvalidRepo(_))
        ));
        drop(dir);
    }

    // ── move_task_to_backlog ──────────────────────────────────────────────────

    #[test]
    fn move_to_backlog_clears_owner() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo.create_task(NewTask { title: "My task".into(), task_type: TaskType::Task, ..Default::default() }).unwrap();
        let id = task.id.clone();
        repo.move_task_to_backlog(&id).unwrap();

        let (backlog, _) = repo.list_backlog_tasks().unwrap();
        assert_eq!(backlog.len(), 1);
        assert!(backlog[0].owner.is_none());
        // File stays in tasks/
        assert!(Path::new(&repo.info.path).join(format!("tasks/{id}.md")).exists());
        drop(dir);
    }

    // ── mark_task_done ────────────────────────────────────────────────────────

    #[test]
    fn mark_task_done_stays_in_type_folder() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo.create_task(NewTask { title: "Task".into(), task_type: TaskType::Task, ..Default::default() }).unwrap();
        let id = task.id.clone();
        let task = repo.mark_task_done(&id).unwrap();
        assert_eq!(task.status, TaskStatus::Done);
        assert!(task.done.is_some());
        // File stays in tasks/, no completed/ folder created
        assert!(Path::new(&repo.info.path).join(format!("tasks/{id}.md")).exists());
        assert!(!Path::new(&repo.info.path).join("completed").exists());
        drop(dir);
    }

    // ── set_task_in_progress ──────────────────────────────────────────────────

    #[test]
    fn set_task_in_progress_changes_status() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo.create_task(NewTask { title: "Task".into(), task_type: TaskType::Task, ..Default::default() }).unwrap();
        let task = repo.set_task_in_progress(&task.id).unwrap();
        assert_eq!(task.status, TaskStatus::InProgress);
        drop(dir);
    }

    #[test]
    fn set_task_in_progress_cycles_back_from_done() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo.create_task(NewTask { title: "Task".into(), task_type: TaskType::Task, ..Default::default() }).unwrap();
        let id = task.id.clone();
        repo.mark_task_done(&id).unwrap();
        let task = repo.set_task_in_progress(&id).unwrap();
        assert_eq!(task.status, TaskStatus::InProgress);
        assert!(task.done.is_none());
        drop(dir);
    }

    // ── assign_task ───────────────────────────────────────────────────────────

    #[test]
    fn assign_task_changes_owner_no_file_move() {
        let (dir, repo) = make_repo("Alice Smith");
        let task = repo.create_task(NewTask { title: "Hand off".into(), task_type: TaskType::Task, ..Default::default() }).unwrap();
        let id = task.id.clone();
        let original_path = Path::new(&repo.info.path).join(format!("tasks/{id}.md"));

        let updated = repo.assign_task(&id, Some("bob-jones".into())).unwrap();
        assert_eq!(updated.owner.as_deref(), Some("bob-jones"));
        // File stays in same place
        assert!(original_path.exists());
        drop(dir);
    }

    // ── push ──────────────────────────────────────────────────────────────────

    #[test]
    fn push_stages_type_folders_no_completed_created() {
        let (work_dir, bare_dir, repo) = make_repo_with_remote("Alice Smith");
        let task = repo.create_task(NewTask { title: "Task".into(), task_type: TaskType::Task, ..Default::default() }).unwrap();
        let id = task.id.clone();
        repo.mark_task_done(&id).unwrap();
        repo.push().unwrap();

        // Task stays in tasks/, no completed/ folder
        assert!(Path::new(&repo.info.path).join(format!("tasks/{id}.md")).exists());
        assert!(!Path::new(&repo.info.path).join("completed").exists());
        drop(work_dir);
        drop(bare_dir);
    }

    // ── list_users ────────────────────────────────────────────────────────────

    #[test]
    fn list_users_includes_current_user() {
        let (dir, repo) = make_repo("Alice Smith");
        let users = repo.list_users().unwrap();
        assert!(users.contains(&"alice-smith".to_string()));
        drop(dir);
    }

    #[test]
    fn list_users_discovers_owners_from_tasks() {
        let (dir, repo) = make_repo("Alice Smith");
        // Create a task then manually set owner to another user
        let task = repo.create_task(NewTask { title: "Task".into(), task_type: TaskType::Task, ..Default::default() }).unwrap();
        repo.assign_task(&task.id, Some("bob-jones".into())).unwrap();
        let users = repo.list_users().unwrap();
        assert!(users.contains(&"bob-jones".to_string()));
        drop(dir);
    }

    // ── v1 migration ──────────────────────────────────────────────────────────

    #[test]
    fn open_migrates_v1_personal_tasks() {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        git_cmd(p, &["init"]);
        git_cmd(p, &["config", "user.name", "Alice Smith"]);
        git_cmd(p, &["config", "user.email", "test@example.com"]);
        fs::write(p.join("gitcake.toml"), "name = \"test\"\n").unwrap();

        // Create v1 structure
        fs::create_dir_all(p.join("alice-smith")).unwrap();
        fs::write(
            p.join("alice-smith/aabbccdd.md"),
            "---\nid: \"aabbccdd\"\ntype: task\ntitle: Old task\nstatus: in-progress\ncreated: 2026-05-01T10:00:00\ndone: \n---\n",
        ).unwrap();

        git_cmd(p, &["add", "."]);
        git_cmd(p, &["commit", "-m", "init"]);

        let repo = TaskRepo::open(p).unwrap();
        let (tasks, _) = repo.list_tasks().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Old task");
        assert_eq!(tasks[0].owner.as_deref(), Some("alice-smith"));
        assert_eq!(tasks[0].task_type, TaskType::Task);
        // Old folder gone, new folder exists
        assert!(!p.join("alice-smith").exists());
        assert!(p.join("tasks/aabbccdd.md").exists());
        drop(dir);
    }

    #[test]
    fn open_migrates_v1_backlog() {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        git_cmd(p, &["init"]);
        git_cmd(p, &["config", "user.name", "Alice Smith"]);
        git_cmd(p, &["config", "user.email", "test@example.com"]);
        fs::write(p.join("gitcake.toml"), "name = \"test\"\n").unwrap();
        fs::create_dir_all(p.join("alice-smith")).unwrap(); // trigger migration
        fs::create_dir_all(p.join("backlog")).unwrap();
        fs::write(
            p.join("backlog/11223344.md"),
            "---\nid: \"11223344\"\ntype: bug\ntitle: Backlog bug\nstatus: open\ncreated: 2026-05-01T10:00:00\ndone: \n---\n",
        ).unwrap();
        git_cmd(p, &["add", "."]);
        git_cmd(p, &["commit", "-m", "init"]);

        let repo = TaskRepo::open(p).unwrap();
        let (backlog, _) = repo.list_backlog_tasks().unwrap();
        assert_eq!(backlog.len(), 1);
        assert_eq!(backlog[0].title, "Backlog bug");
        assert!(backlog[0].owner.is_none());
        assert_eq!(backlog[0].task_type, TaskType::Bug);
        assert!(!p.join("backlog").exists());
        drop(dir);
    }
}
