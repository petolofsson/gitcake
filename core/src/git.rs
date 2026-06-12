use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::AppError;

fn abbrev_relative(s: &str) -> String {
    s.replace(" days ago",    "d ago")
     .replace(" day ago",     "d ago")
     .replace(" hours ago",   "h ago")
     .replace(" hour ago",    "h ago")
     .replace(" minutes ago", "m ago")
     .replace(" minute ago",  "m ago")
     .replace(" weeks ago",   "w ago")
     .replace(" week ago",    "w ago")
     .replace(" months ago",  "mo ago")
     .replace(" month ago",   "mo ago")
}

pub struct GitRepo {
    pub path: PathBuf,
}

impl GitRepo {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Returns true if `path` contains a `.git` entry (file or directory).
    pub fn is_git_repo(&self) -> bool {
        self.path.join(".git").exists()
    }

    /// Returns true if the repo has at least one remote configured.
    pub fn has_remote(&self) -> Result<bool, AppError> {
        let out = self.run_git(&["remote"])?;
        Ok(!out.is_empty())
    }

    /// Reads `git config user.name` from the repo context and derives
    /// the username: lowercased with spaces replaced by hyphens.
    /// Returns `AppError::UserNotConfigured` if user.name is unset or empty.
    /// Returns `AppError::UsernameInvalid` if user.name contains non-ASCII
    /// characters that would produce an unsafe or ambiguous folder name.
    pub fn get_username(&self) -> Result<String, AppError> {
        let name = self
            .run_git(&["config", "user.name"])
            .map_err(|_| AppError::UserNotConfigured)?;
        if name.is_empty() {
            return Err(AppError::UserNotConfigured);
        }
        let username = name.to_lowercase().replace(' ', "-");
        if !username.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(AppError::UsernameInvalid(name));
        }
        Ok(username)
    }

    /// Returns true if there are staged or unstaged changes under `folder`
    /// (relative to the repo root). Used to decide whether to show the
    /// on-exit push prompt.
    pub fn has_local_changes(&self, folder: &str) -> Result<bool, AppError> {
        let out = self.run_git(&["status", "--porcelain", "--", folder])?;
        Ok(!out.is_empty())
    }

    /// Runs `git pull` and returns stdout.
    pub fn pull(&self) -> Result<String, AppError> {
        self.run_git(&["pull"])
    }

    /// Moves `from` to `to` (both relative to the repo root) via `git mv`,
    /// creating the destination directory if it does not exist.
    pub fn move_file(&self, from: &Path, to: &Path) -> Result<(), AppError> {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(self.path.join(parent))?;
        }
        self.run_git(&[
            "mv",
            &from.to_string_lossy(),
            &to.to_string_lossy(),
        ])?;
        Ok(())
    }

    /// Stages all changes under `folder` and commits with `message`.
    /// Returns `Ok` with a "nothing to commit" note if the working tree
    /// is clean — does not treat that as an error.
    pub fn commit(&self, folder: &str, message: &str) -> Result<String, AppError> {
        if !self.has_local_changes(folder)? {
            return Ok("Nothing to commit".to_string());
        }
        self.run_git(&["add", folder])?;
        self.run_git(&["commit", "-m", message])
    }

    /// Stages changes under `path` (relative to repo root) without committing.
    /// Used when building a single commit from multiple paths.
    pub fn stage(&self, path: &str) -> Result<(), AppError> {
        self.run_git(&["add", path])?;
        Ok(())
    }

    /// Commits whatever is currently staged. Treats "nothing to commit" as
    /// `Ok` rather than an error.
    pub fn commit_staged(&self, message: &str) -> Result<String, AppError> {
        match self.run_git(&["commit", "-m", message]) {
            Ok(out) => Ok(out),
            Err(AppError::Git(msg)) if msg.contains("nothing to commit") => {
                Ok("Nothing to commit".to_string())
            }
            Err(e) => Err(e),
        }
    }

    /// Removes a file from git tracking and the filesystem.
    /// Uses `git rm --force` for tracked files; falls back to plain `fs::remove_file`
    /// for untracked (new, never committed) files.
    pub fn remove_tracked(&self, path: &str) -> Result<(), AppError> {
        match self.run_git(&["rm", "--force", path]) {
            Ok(_) => Ok(()),
            Err(_) => {
                std::fs::remove_file(self.path.join(path)).map_err(AppError::from)
            }
        }
    }

    /// Runs `git push` and returns stdout.
    pub fn push(&self) -> Result<String, AppError> {
        self.run_git(&["push"])
    }

    /// Returns (commits_ahead_of_remote, last_push_relative_time).
    /// Returns None if there is no upstream or the git calls fail.
    pub fn ahead_status(&self) -> Option<(u32, String)> {
        let count: u32 = self.run_git(&["rev-list", "--count", "@{u}..HEAD"]).ok()?.trim().parse().ok()?;
        let raw = self.run_git(&["log", "-1", "--format=%cr", "@{u}"]).ok()?;
        let time = abbrev_relative(raw.trim());
        if time.is_empty() { return None; }
        Some((count, time))
    }

    // ── private ──────────────────────────────────────────────────────────────

    fn run_git(&self, args: &[&str]) -> Result<String, AppError> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.path)
            .output()
            .map_err(|e| AppError::Git(format!("failed to run git: {e}")))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Err(AppError::Git(if stderr.is_empty() { stdout } else { stderr }))
        }
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Creates an initialised git repo in a temp dir with user.name configured.
    fn make_repo(username: &str) -> (TempDir, GitRepo) {
        let dir = TempDir::new().unwrap();
        let p = dir.path();

        git(p, &["init"]);
        git(p, &["config", "user.name", username]);
        git(p, &["config", "user.email", "test@example.com"]);
        // initial commit so HEAD exists
        fs::write(p.join("gitcake.toml"), "name = \"test\"\n").unwrap();
        git(p, &["add", "."]);
        git(p, &["commit", "-m", "init"]);

        let repo = GitRepo::new(p);
        (dir, repo)
    }

    /// Creates a bare repo as a local remote and pushes, returning both dirs.
    fn make_repo_with_remote(username: &str) -> (TempDir, TempDir, GitRepo) {
        let (work_dir, repo) = make_repo(username);
        let bare_dir = TempDir::new().unwrap();
        git(bare_dir.path(), &["init", "--bare"]);
        git(
            work_dir.path(),
            &[
                "remote",
                "add",
                "origin",
                &bare_dir.path().to_string_lossy(),
            ],
        );
        git(work_dir.path(), &["push", "-u", "origin", "HEAD"]);
        (work_dir, bare_dir, repo)
    }

    fn git(dir: &Path, args: &[&str]) {
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git command failed");
    }

    // ── is_git_repo ───────────────────────────────────────────────────────────

    #[test]
    fn is_git_repo_returns_true_for_git_dir() {
        let (dir, repo) = make_repo("Test User");
        assert!(repo.is_git_repo());
        drop(dir);
    }

    #[test]
    fn is_git_repo_returns_false_for_plain_dir() {
        let dir = TempDir::new().unwrap();
        let repo = GitRepo::new(dir.path());
        assert!(!repo.is_git_repo());
    }

    // ── has_remote ────────────────────────────────────────────────────────────

    #[test]
    fn has_remote_returns_false_with_no_remote() {
        let (dir, repo) = make_repo("Test User");
        assert!(!repo.has_remote().unwrap());
        drop(dir);
    }

    #[test]
    fn has_remote_returns_true_after_remote_added() {
        let (work_dir, bare_dir, repo) = make_repo_with_remote("Test User");
        assert!(repo.has_remote().unwrap());
        drop(work_dir);
        drop(bare_dir);
    }

    // ── get_username ──────────────────────────────────────────────────────────

    #[test]
    fn get_username_lowercases_and_hyphenates() {
        let (dir, repo) = make_repo("Alice Smith");
        assert_eq!(repo.get_username().unwrap(), "alice-smith");
        drop(dir);
    }

    #[test]
    fn get_username_handles_single_word_name() {
        let (dir, repo) = make_repo("alice");
        assert_eq!(repo.get_username().unwrap(), "alice");
        drop(dir);
    }

    #[test]
    fn get_username_errors_on_non_ascii_name() {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        git(p, &["init"]);
        git(p, &["config", "user.name", "José García"]);
        git(p, &["config", "user.email", "test@example.com"]);
        let repo = GitRepo::new(p);
        assert!(matches!(repo.get_username(), Err(AppError::UsernameInvalid(_))));
    }

    #[test]
    fn get_username_errors_on_emoji_name() {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        git(p, &["init"]);
        git(p, &["config", "user.name", "dev🦀"]);
        git(p, &["config", "user.email", "test@example.com"]);
        let repo = GitRepo::new(p);
        assert!(matches!(repo.get_username(), Err(AppError::UsernameInvalid(_))));
    }

    #[test]
    fn get_username_errors_when_unset() {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        git(p, &["init"]);
        git(p, &["config", "user.email", "test@example.com"]);
        // Set local user.name to empty string to override any global config
        git(p, &["config", "--local", "user.name", ""]);
        let repo = GitRepo::new(p);
        assert!(matches!(repo.get_username(), Err(AppError::UserNotConfigured)));
    }

    // ── has_local_changes ─────────────────────────────────────────────────────

    #[test]
    fn has_local_changes_false_on_clean_repo() {
        let (dir, repo) = make_repo("Test User");
        assert!(!repo.has_local_changes("alice-smith").unwrap());
        drop(dir);
    }

    #[test]
    fn has_local_changes_true_after_new_file() {
        let (dir, repo) = make_repo("Test User");
        fs::create_dir(dir.path().join("alice-smith")).unwrap();
        fs::write(dir.path().join("alice-smith/001.md"), "# task").unwrap();
        assert!(repo.has_local_changes("alice-smith").unwrap());
        drop(dir);
    }

    #[test]
    fn has_local_changes_scoped_to_folder() {
        let (dir, repo) = make_repo("Test User");
        // change outside the watched folder
        fs::write(dir.path().join("other.txt"), "noise").unwrap();
        assert!(!repo.has_local_changes("alice-smith").unwrap());
        drop(dir);
    }

    // ── commit ────────────────────────────────────────────────────────────────

    #[test]
    fn commit_stages_and_commits_folder() {
        let (dir, repo) = make_repo("Test User");
        fs::create_dir(dir.path().join("alice-smith")).unwrap();
        fs::write(dir.path().join("alice-smith/001.md"), "# task").unwrap();
        repo.commit("alice-smith", "test commit").unwrap();
        // after commit the folder should be clean
        assert!(!repo.has_local_changes("alice-smith").unwrap());
        drop(dir);
    }

    #[test]
    fn commit_returns_ok_when_nothing_to_commit() {
        let (dir, repo) = make_repo("Test User");
        let result = repo.commit("alice-smith", "empty commit");
        assert!(result.is_ok());
        drop(dir);
    }

    // ── move_file ─────────────────────────────────────────────────────────────

    #[test]
    fn move_file_relocates_file_and_creates_dest_dir() {
        let (dir, repo) = make_repo("Test User");
        let p = dir.path();

        // create and commit a task file
        fs::create_dir(p.join("alice-smith")).unwrap();
        fs::write(p.join("alice-smith/001.md"), "# task").unwrap();
        git(p, &["add", "."]);
        git(p, &["commit", "-m", "add task"]);

        repo.move_file(
            Path::new("alice-smith/001.md"),
            Path::new("completed/alice-smith/001.md"),
        )
        .unwrap();

        assert!(!p.join("alice-smith/001.md").exists());
        assert!(p.join("completed/alice-smith/001.md").exists());
        drop(dir);
    }

    // ── pull / push ───────────────────────────────────────────────────────────

    #[test]
    fn push_succeeds_with_remote() {
        let (work_dir, bare_dir, repo) = make_repo_with_remote("Test User");
        // make a new commit to push
        fs::write(work_dir.path().join("new.txt"), "data").unwrap();
        repo.commit(".", "new file").unwrap();
        assert!(repo.push().is_ok());
        drop(work_dir);
        drop(bare_dir);
    }

    #[test]
    fn pull_succeeds_when_up_to_date() {
        let (work_dir, bare_dir, repo) = make_repo_with_remote("Test User");
        assert!(repo.pull().is_ok());
        drop(work_dir);
        drop(bare_dir);
    }
}
