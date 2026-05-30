use std::{env, fs, path::Path, process::Command};

use crossterm::{
    event::{Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use git_task_core::{
    models::task::{Task, TaskStatus, TaskType},
    repo::TaskRepo,
};

use crate::config::Config;

// ── screens ───────────────────────────────────────────────────────────────────

pub enum Screen {
    Setup {
        input: String,
        error: Option<String>,
    },
    InitRepo {
        path: String,
        name: String,
        error: Option<String>,
    },
    TaskList {
        tasks: Vec<Task>,
        selected: usize,
        message: Option<String>,
    },
    Detail {
        task: Task,
        message: Option<String>,
    },
    Create {
        title: String,
        task_type: TaskType,
        description: String,
        field: CreateField,
    },
    Edit {
        task_id: String,
        title: String,
        description: String,
        field: EditField,
    },
    SyncConfirm,
    PushPrompt,
}

#[derive(PartialEq)]
pub enum CreateField {
    Title,
    Type,
    Description,
}

#[derive(PartialEq)]
pub enum EditField {
    Title,
    Description,
}

// ── app ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub screen: Screen,
    pub repo: Option<TaskRepo>,
    pub config: Config,
    pub should_quit: bool,
    pub needs_clear: bool,
}

impl App {
    pub fn new(config: Config) -> Self {
        if config.repo_path.is_none() {
            return Self {
                screen: Screen::Setup { input: String::new(), error: None },
                repo: None,
                config,
                should_quit: false,
                needs_clear: false,
            };
        }

        let repo = config.repo_path.as_deref().and_then(|p| TaskRepo::open(p).ok());

        if repo.is_none() {
            return Self {
                screen: Screen::Setup {
                    input: config.repo_path.clone().unwrap_or_default(),
                    error: Some("Could not open repo — check the path.".into()),
                },
                repo: None,
                config,
                should_quit: false,
                needs_clear: false,
            };
        }

        // Auto-pull on startup; failures are non-fatal
        let pull_msg = match repo.as_ref().unwrap().pull() {
            Ok(out) if out.trim().is_empty() || out.contains("Already up to date") => None,
            Ok(_) => Some("Pulled latest changes.".to_string()),
            Err(e) => Some(format!("Pull failed ({})", e)),
        };

        let tasks = repo.as_ref().unwrap().list_tasks().unwrap_or_default();
        let screen = Screen::TaskList { tasks, selected: 0, message: pull_msg };

        Self { screen, repo, config, should_quit: false, needs_clear: false }
    }

    pub fn handle_event(&mut self, event: Event) {
        let Event::Key(key) = event else { return };
        match &self.screen {
            Screen::Setup { .. } => self.handle_setup(key),
            Screen::InitRepo { .. } => self.handle_init_repo(key),
            Screen::TaskList { .. } => self.handle_task_list(key),
            Screen::Detail { .. } => self.handle_detail(key),
            Screen::Create { .. } => self.handle_create(key),
            Screen::Edit { .. } => self.handle_edit(key),
            Screen::SyncConfirm => self.handle_sync_confirm(key),
            Screen::PushPrompt => self.handle_push_prompt(key),
        }
    }

    // ── setup ─────────────────────────────────────────────────────────────────

    fn handle_setup(&mut self, key: KeyEvent) {
        let Screen::Setup { input, error: _ } = &mut self.screen else {
            return;
        };
        // Ctrl+Q always quits
        if is_ctrl_q(&key) { self.should_quit = true; return; }
        match key.code {
            // 'q' in a path input types the letter — Esc quits
            KeyCode::Char(c) => input.push(c),
            KeyCode::Backspace => { input.pop(); }
            KeyCode::Enter => {
                let path = input.trim().to_string();
                self.evaluate_path(path);
            }
            KeyCode::Esc => self.should_quit = true,
            _ => {}
        }
    }

    // ── init repo ─────────────────────────────────────────────────────────────

    fn handle_init_repo(&mut self, key: KeyEvent) {
        let Screen::InitRepo { path, name, error: _ } = &mut self.screen else {
            return;
        };
        match key.code {
            KeyCode::Char(c) => name.push(c),
            KeyCode::Backspace => { name.pop(); }
            KeyCode::Enter => {
                let path = path.clone();
                let name = name.trim().to_string();
                if name.is_empty() {
                    return;
                }
                match TaskRepo::init(&path, &name) {
                    Ok(repo) => {
                        self.config.repo_path = Some(path);
                        self.config.save();
                        self.repo = Some(repo);
                        self.enter_task_list(Some("Repo initialized.".into()));
                    }
                    Err(e) => {
                        self.screen = Screen::InitRepo {
                            path,
                            name,
                            error: Some(e.to_string()),
                        };
                    }
                }
            }
            KeyCode::Esc => {
                let path = path.clone();
                self.screen = Screen::Setup { input: path, error: None };
            }
            _ => {}
        }
    }

    // ── task list ─────────────────────────────────────────────────────────────

    fn handle_task_list(&mut self, key: KeyEvent) {
        let km = self.config.keys.clone();

        if is_key(&key, &km.quit) {
            self.try_quit();
            return;
        }
        if is_key(&key, &km.sync) {
            self.screen = Screen::SyncConfirm;
            return;
        }

        let Screen::TaskList { tasks, selected, .. } = &mut self.screen else {
            return;
        };

        let task_count = tasks.len();

        match key.code {
            KeyCode::Char(c) if c == km.up.chars().next().unwrap_or('w') && km.up.len() == 1 => {
                if *selected > 0 {
                    *selected -= 1;
                }
            }
            KeyCode::Char(c) if c == km.down.chars().next().unwrap_or('s') && km.down.len() == 1 => {
                if task_count > 0 && *selected < task_count - 1 {
                    *selected += 1;
                }
            }
            KeyCode::Up => {
                if *selected > 0 {
                    *selected -= 1;
                }
            }
            KeyCode::Down => {
                if task_count > 0 && *selected < task_count - 1 {
                    *selected += 1;
                }
            }
            KeyCode::Char(c) if c == km.detail.chars().next().unwrap_or('d') && km.detail.len() == 1 => {
                if let Some(task) = tasks.get(*selected).cloned() {
                    self.screen = Screen::Detail { task, message: None };
                }
            }
            KeyCode::Char(c) if c == km.create.chars().next().unwrap_or('c') && km.create.len() == 1 => {
                self.screen = Screen::Create {
                    title: String::new(),
                    task_type: TaskType::Task,
                    description: String::new(),
                    field: CreateField::Title,
                };
            }
            KeyCode::Char(c) if c == km.edit.chars().next().unwrap_or('e') && km.edit.len() == 1 => {
                if let Some(task) = tasks.get(*selected).cloned() {
                    self.screen = Screen::Edit {
                        task_id: task.id.clone(),
                        title: task.title.clone(),
                        description: task.description.clone().unwrap_or_default(),
                        field: EditField::Description,
                    };
                }
            }
            KeyCode::Char(c)
                if c == km.status_cycle.chars().next().unwrap_or('f')
                    && km.status_cycle.len() == 1 =>
            {
                let sel = *selected;
                if let Some(task) = tasks.get(sel).cloned() {
                    self.cycle_status(&task.id);
                }
            }
            _ => {}
        }
    }

    // ── detail ────────────────────────────────────────────────────────────────

    fn handle_detail(&mut self, key: KeyEvent) {
        let km = self.config.keys.clone();
        let Screen::Detail { task, .. } = &self.screen else {
            return;
        };
        let task_id = task.id.clone();

        if is_key(&key, &km.back) || matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
            self.enter_task_list(None);
            return;
        }
        if is_key(&key, &km.edit) {
            let Screen::Detail { task, .. } = &self.screen else {
                return;
            };
            self.screen = Screen::Edit {
                task_id: task.id.clone(),
                title: task.title.clone(),
                description: task.description.clone().unwrap_or_default(),
                field: EditField::Description,
            };
            return;
        }
        if is_key(&key, &km.status_cycle) {
            self.cycle_status(&task_id);
            // Reload the task after cycling
            if let Some(repo) = &self.repo {
                if let Ok(tasks) = repo.list_tasks() {
                    if let Some(updated) = tasks.into_iter().find(|t| t.id == task_id) {
                        self.screen = Screen::Detail { task: updated, message: None };
                    }
                }
            }
        }
    }

    // ── create ────────────────────────────────────────────────────────────────

    fn handle_create(&mut self, key: KeyEvent) {
        if is_ctrl_q(&key) { self.should_quit = true; return; }

        let Screen::Create { title, task_type, description, field } = &mut self.screen else {
            return;
        };

        // Ctrl+S saves from any field
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            if title.trim().is_empty() {
                return;
            }
            let title = title.trim().to_string();
            let task_type = task_type.clone();
            let desc = description.trim().to_string();
            let desc = if desc.is_empty() { None } else { Some(desc) };
            if let Some(repo) = &self.repo {
                match repo.create_task(title, task_type, desc) {
                    Ok(_) => self.enter_task_list(Some("Task created.".into())),
                    Err(e) => self.enter_task_list(Some(e.to_string())),
                }
            }
            return;
        }

        let mut launched_editor = false;
        match key.code {
            KeyCode::Esc => self.enter_task_list(None),
            KeyCode::Tab => {
                *field = match field {
                    CreateField::Title => CreateField::Type,
                    CreateField::Type => CreateField::Description,
                    CreateField::Description => CreateField::Title,
                };
            }
            KeyCode::Enter if *field == CreateField::Title => {
                *field = CreateField::Type;
            }
            KeyCode::Enter if *field == CreateField::Type => {
                *field = CreateField::Description;
            }
            // Enter on description opens $EDITOR
            KeyCode::Enter if *field == CreateField::Description => {
                let current = description.clone();
                if let Some(edited) = open_in_editor(&current) {
                    *description = edited;
                }
                launched_editor = true;
            }
            KeyCode::Char(' ') if *field == CreateField::Type => {
                *task_type = match task_type {
                    TaskType::Task => TaskType::Bug,
                    TaskType::Bug => TaskType::Incident,
                    TaskType::Incident => TaskType::Task,
                };
            }
            KeyCode::Backspace if *field == CreateField::Title => { title.pop(); }
            KeyCode::Char(c) if *field == CreateField::Title => title.push(c),
            _ => {}
        }
        if launched_editor {
            self.needs_clear = true;
        }
    }

    // ── edit ──────────────────────────────────────────────────────────────────

    fn handle_edit(&mut self, key: KeyEvent) {
        if is_ctrl_q(&key) { self.should_quit = true; return; }

        let Screen::Edit { task_id, title, description, field } = &mut self.screen else {
            return;
        };

        // Ctrl+S saves from any field
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            let id = task_id.clone();
            let new_title = Some(title.trim().to_string()).filter(|s| !s.is_empty());
            let new_desc = Some(description.trim().to_string()).filter(|s| !s.is_empty());
            if let Some(repo) = &self.repo {
                match repo.update_task(&id, new_title, new_desc) {
                    Ok(_) => self.enter_task_list(Some("Task updated.".into())),
                    Err(e) => self.enter_task_list(Some(e.to_string())),
                }
            }
            return;
        }

        let mut launched_editor = false;
        match key.code {
            KeyCode::Esc => self.enter_task_list(None),
            KeyCode::Tab => {
                *field = match field {
                    EditField::Title => EditField::Description,
                    EditField::Description => EditField::Title,
                };
            }
            KeyCode::Enter if *field == EditField::Title => {
                *field = EditField::Description;
            }
            // Enter on description opens $EDITOR
            KeyCode::Enter if *field == EditField::Description => {
                let current = description.clone();
                if let Some(edited) = open_in_editor(&current) {
                    *description = edited;
                }
                launched_editor = true;
            }
            KeyCode::Backspace if *field == EditField::Title => { title.pop(); }
            KeyCode::Char(c) if *field == EditField::Title => title.push(c),
            _ => {}
        }
        if launched_editor {
            self.needs_clear = true;
        }
    }

    // ── sync confirm ──────────────────────────────────────────────────────────

    fn handle_sync_confirm(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                let result = self
                    .repo
                    .as_ref()
                    .map(|r| r.push())
                    .unwrap_or(Err(git_task_core::error::AppError::NoRepo));
                let msg = match result {
                    Ok(out) => {
                        if out.trim().is_empty() { "Synced.".into() } else { out }
                    }
                    Err(e) => e.to_string(),
                };
                self.enter_task_list(Some(msg));
            }
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('n') | KeyCode::Char('N') => {
                self.enter_task_list(None);
            }
            _ => {}
        }
    }

    // ── push prompt (on quit) ─────────────────────────────────────────────────

    fn handle_push_prompt(&mut self, key: KeyEvent) {
        match key.code {
            // Y is the default — Enter or Y pushes then quits
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(repo) = &self.repo {
                    let _ = repo.push();
                }
                self.should_quit = true;
            }
            // n quits without pushing
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.should_quit = true;
            }
            // Esc cancels the quit entirely — back to task list
            KeyCode::Esc => {
                self.enter_task_list(None);
            }
            _ => {}
        }
    }


    // ── helpers ───────────────────────────────────────────────────────────────

    // Check 1: has git-task.toml → open.
    // Check 2: has other files, no git-task.toml → assume code repo, reject.
    // Check 3: only README/.gitignore/empty → offer to initialize.
    fn evaluate_path(&mut self, path: String) {
        let path = expand_tilde(&path);
        let p = Path::new(&path);

        if !p.join(".git").exists() {
            self.screen = Screen::Setup {
                input: path,
                error: Some("Not a git repository.".into()),
            };
            return;
        }

        if p.join("git-task.toml").exists() {
            match TaskRepo::open(&path) {
                Ok(repo) => {
                    self.config.repo_path = Some(path);
                    self.config.save();
                    self.repo = Some(repo);
                    let pull_msg = match self.repo.as_ref().unwrap().pull() {
                        Ok(out) if out.trim().is_empty() || out.contains("Already up to date") => None,
                        Ok(_) => Some("Pulled latest changes.".to_string()),
                        Err(e) => Some(format!("Pull failed ({e})")),
                    };
                    self.enter_task_list(pull_msg);
                }
                Err(e) => {
                    self.screen = Screen::Setup { input: path, error: Some(e.to_string()) };
                }
            }
            return;
        }

        if is_empty_repo(p) {
            self.screen = Screen::InitRepo { path, name: String::new(), error: None };
        } else {
            self.screen = Screen::Setup {
                input: path,
                error: Some("This looks like a code repo. Point to a dedicated git-task repo.".into()),
            };
        }
    }

    fn enter_task_list(&mut self, message: Option<String>) {
        let tasks = self
            .repo
            .as_ref()
            .and_then(|r| r.list_tasks().ok())
            .unwrap_or_default();
        self.screen = Screen::TaskList { tasks, selected: 0, message };
    }

    fn cycle_status(&mut self, task_id: &str) {
        let Some(repo) = &self.repo else { return };
        let tasks = repo.list_tasks().unwrap_or_default();
        let Some(task) = tasks.iter().find(|t| t.id == task_id) else { return };

        let result = match task.status {
            TaskStatus::Open => repo.set_task_in_progress(task_id),
            TaskStatus::InProgress => repo.mark_task_done(task_id),
            TaskStatus::Done => repo.set_task_in_progress(task_id),
        };

        let msg = match result {
            Ok(t) => match t.status {
                TaskStatus::InProgress => Some("Marked in-progress.".into()),
                TaskStatus::Done => Some("Marked done.".into()),
                TaskStatus::Open => Some("Marked open.".into()),
            },
            Err(e) => Some(e.to_string()),
        };

        if let Screen::TaskList { .. } = &self.screen {
            self.enter_task_list(msg);
        }
    }

    fn try_quit(&mut self) {
        self.screen = Screen::PushPrompt;
    }
}

// ── key matching ──────────────────────────────────────────────────────────────

pub fn is_key(event: &KeyEvent, binding: &str) -> bool {
    if let Some(ctrl_key) = binding.strip_prefix("ctrl+") {
        let ch = ctrl_key.chars().next().unwrap_or('\0');
        return event.modifiers.contains(KeyModifiers::CONTROL)
            && event.code == KeyCode::Char(ch);
    }
    if binding.len() == 1 {
        let ch = binding.chars().next().unwrap_or('\0');
        return event.modifiers == KeyModifiers::NONE && event.code == KeyCode::Char(ch);
    }
    false
}

pub fn is_ctrl_q(event: &KeyEvent) -> bool {
    event.modifiers.contains(KeyModifiers::CONTROL) && event.code == KeyCode::Char('q')
}

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().into_owned();
        }
    } else if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home.to_string_lossy().into_owned();
        }
    }
    path.to_string()
}

// Suspends ratatui, opens content in $VISUAL/$EDITOR, resumes ratatui.
// Returns the edited content, or None if the editor couldn't be launched.
fn open_in_editor(content: &str) -> Option<String> {
    let editor = env::var("VISUAL")
        .or_else(|_| env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());

    let tmp_path = env::temp_dir().join(format!("gt-desc-{}.md", std::process::id()));
    fs::write(&tmp_path, content).ok()?;

    let _ = disable_raw_mode();
    let _ = execute!(std::io::stdout(), LeaveAlternateScreen);

    let _ = Command::new(&editor).arg(&tmp_path).status();

    let _ = enable_raw_mode();
    let _ = execute!(std::io::stdout(), EnterAlternateScreen);

    let result = fs::read_to_string(&tmp_path).ok();
    let _ = fs::remove_file(&tmp_path);
    result
}

// Returns true if the repo contains only README/gitignore-style files —
// safe to offer initialization without risk of clobbering real code.
fn is_empty_repo(path: &Path) -> bool {
    let Ok(entries) = fs::read_dir(path) else { return false };
    entries.flatten().all(|entry| {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        matches!(
            name.as_str(),
            ".git" | "readme.md" | "readme" | "readme.txt" | ".gitignore" | ".gitattributes" | ".gitkeep"
        )
    })
}
