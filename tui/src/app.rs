use std::{env, fs, path::{Path, PathBuf}, process::Command};

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

// ── context ───────────────────────────────────────────────────────────────────

#[derive(PartialEq, Clone, Copy)]
pub enum TaskContext {
    Personal,
    Backlog,
}

// ── screens ───────────────────────────────────────────────────────────────────

pub enum Screen {
    Setup {
        input: String,
        error: Option<String>,
        /// True when reached via Ctrl+O from the task list; Esc returns instead of quitting.
        can_cancel: bool,
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
        assignee: String,
        description: String,
        field: CreateField,
    },
    AssignTask {
        task_id: String,
        users: Vec<String>,
        selected: usize,
        filter: String,
    },
    PickAssignee {
        title: String,
        task_type: TaskType,
        description: String,
        users: Vec<String>,
        selected: usize,
        filter: String,
    },
    DeleteConfirm {
        task_id: String,
        task_title: String,
    },
    SyncConfirm,
    PushPrompt,
    TeamView {
        tasks: Vec<(String, git_task_core::models::task::Task)>,
        selected: usize,
    },
}

#[derive(PartialEq, Clone)]
pub enum CreateField {
    Title,
    Type,
    Assignee,
    Description,
}

// ── app ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub screen: Screen,
    pub repo: Option<TaskRepo>,
    pub config: Config,
    pub context: TaskContext,
    pub should_quit: bool,
    pub needs_clear: bool,
    pub exit_message: Option<String>,
    pub pull_error: Option<String>,
    pub lock_path: Option<PathBuf>,
}

impl App {
    pub fn new(config: Config) -> Self {
        if config.repo_path.is_none() {
            return Self {
                screen: Screen::Setup { input: String::new(), error: None, can_cancel: false },
                repo: None,
                config,
                context: TaskContext::Personal,
                should_quit: false,
                needs_clear: false,
                exit_message: None,
                pull_error: None,
                lock_path: None,
            };
        }

        let repo = config.repo_path.as_deref().and_then(|p| TaskRepo::open(p).ok());

        if repo.is_none() {
            return Self {
                screen: Screen::Setup {
                    input: config.repo_path.clone().unwrap_or_default(),
                    error: Some("Could not open repo — check the path.".into()),
                    can_cancel: false,
                },
                repo: None,
                config,
                context: TaskContext::Personal,
                should_quit: false,
                needs_clear: false,
                exit_message: None,
                pull_error: None,
                lock_path: None,
            };
        }

        // Auto-pull on startup; failures are non-fatal
        let (pull_msg, pull_error) = classify_pull_result(repo.as_ref().unwrap().pull());

        let repo_path = repo.as_ref().unwrap().info.path.clone();
        let (lock_path, lock_warn) = acquire_lock(&repo_path);

        let (raw_tasks, task_warnings) = repo.as_ref().unwrap().list_tasks().unwrap_or_default();
        let startup_msg = merge_messages(
            merge_messages(pull_msg, lock_warn),
            warn_summary(&task_warnings),
        );
        let tasks = sort_for_display(raw_tasks);
        let screen = Screen::TaskList { tasks, selected: 0, message: startup_msg };

        Self { screen, repo, config, context: TaskContext::Personal, should_quit: false, needs_clear: false, exit_message: None, pull_error, lock_path }
    }

    pub fn handle_event(&mut self, event: Event) {
        let Event::Key(key) = event else { return };
        match &self.screen {
            Screen::Setup { .. } => self.handle_setup(key),
            Screen::InitRepo { .. } => self.handle_init_repo(key),
            Screen::TaskList { .. } => self.handle_task_list(key),
            Screen::Detail { .. } => self.handle_detail(key),
            Screen::Create { .. } => self.handle_create(key),
            Screen::AssignTask { .. } => self.handle_assign_task(key),
            Screen::PickAssignee { .. } => self.handle_pick_assignee(key),
            Screen::DeleteConfirm { .. } => self.handle_delete_confirm(key),
            Screen::SyncConfirm => self.handle_sync_confirm(key),
            Screen::PushPrompt => self.handle_push_prompt(key),
            Screen::TeamView { .. } => self.handle_team_view(key),
        }
    }

    // ── setup ─────────────────────────────────────────────────────────────────

    fn handle_setup(&mut self, key: KeyEvent) {
        let Screen::Setup { input, error: _, can_cancel } = &mut self.screen else {
            return;
        };
        let can_cancel = *can_cancel;
        if is_ctrl_q(&key) { self.should_quit = true; return; }
        match key.code {
            KeyCode::Char(c) => input.push(c),
            KeyCode::Backspace => { input.pop(); }
            KeyCode::Enter => {
                let path = input.trim().to_string();
                self.evaluate_path(path, can_cancel);
            }
            KeyCode::Esc => {
                if can_cancel {
                    self.enter_task_list(None, None);
                } else {
                    self.should_quit = true;
                }
            }
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
                        self.enter_task_list(Some("Repo initialized.".into()), None);
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
                self.screen = Screen::Setup { input: path, error: None, can_cancel: false };
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
        if key.code == KeyCode::Esc {
            self.pull_error = None;
            return;
        }
        if key.code == KeyCode::Char('o') && key.modifiers.contains(KeyModifiers::CONTROL) {
            let current_path = self.config.repo_path.clone().unwrap_or_default();
            self.screen = Screen::Setup { input: current_path, error: None, can_cancel: true };
            return;
        }
        if key.code == KeyCode::Char('p') && key.modifiers.contains(KeyModifiers::CONTROL) {
            let current_id = if let Screen::TaskList { tasks, selected, .. } = &self.screen {
                tasks.get(*selected).map(|t| t.id.clone())
            } else {
                None
            };
            let pull_result = self.repo.as_ref().map(|r| r.pull());
            let (pull_msg, pull_err) = match pull_result {
                Some(result) => classify_pull_result(result),
                None => (None, Some("No repo connected.".to_string())),
            };
            self.pull_error = pull_err;
            self.enter_task_list(pull_msg, current_id.as_deref());
            return;
        }

        let Screen::TaskList { tasks, selected, message } = &mut self.screen else {
            return;
        };

        let task_count = tasks.len();

        match key.code {
            KeyCode::Char(c) if c == km.up.chars().next().unwrap_or('w') && km.up.len() == 1 && key.modifiers == KeyModifiers::NONE => {
                if task_count > 0 {
                    *selected = selected.checked_sub(1).unwrap_or(task_count - 1);
                    *message = None;
                }
            }
            KeyCode::Char(c) if c == km.down.chars().next().unwrap_or('s') && km.down.len() == 1 && key.modifiers == KeyModifiers::NONE => {
                if task_count > 0 {
                    *selected = (*selected + 1) % task_count;
                    *message = None;
                }
            }
            KeyCode::Up => {
                if task_count > 0 {
                    *selected = selected.checked_sub(1).unwrap_or(task_count - 1);
                    *message = None;
                }
            }
            KeyCode::Down => {
                if task_count > 0 {
                    *selected = (*selected + 1) % task_count;
                    *message = None;
                }
            }
            KeyCode::Char(c) if c == km.detail.chars().next().unwrap_or('d') && km.detail.len() == 1 && key.modifiers == KeyModifiers::NONE => {
                if let Some(task) = tasks.get(*selected).cloned() {
                    self.screen = Screen::Detail { task, message: None };
                }
            }
            KeyCode::Char(c) if c == km.create.chars().next().unwrap_or('c') && km.create.len() == 1 && key.modifiers == KeyModifiers::NONE => {
                let default_assignee = self.repo.as_ref()
                    .map(|r| r.info.username.clone())
                    .unwrap_or_default();
                self.screen = Screen::Create {
                    title: String::new(),
                    task_type: TaskType::Task,
                    assignee: default_assignee,
                    description: String::new(),
                    field: CreateField::Title,
                };
            }
            KeyCode::Char(c) if c == km.edit.chars().next().unwrap_or('e') && km.edit.len() == 1 && key.modifiers == KeyModifiers::NONE => {
                let sel = *selected;
                if let Some(task) = tasks.get(sel).cloned() {
                    let id = task.id.clone();
                    let title = task.title.clone();
                    let desc = task.description.clone().unwrap_or_default();
                    let ctx = self.context;
                    if let Some((new_title, new_desc)) = edit_task_in_editor(&title, &desc) {
                        let msg = self.repo.as_ref().map(|r| match ctx {
                            TaskContext::Personal => r.update_task(&id, Some(new_title), new_desc),
                            TaskContext::Backlog => r.update_backlog_task(&id, Some(new_title), new_desc),
                        }).map(|res| match res {
                            Ok(_) => "Task updated.".to_string(),
                            Err(e) => e.to_string(),
                        });
                        self.needs_clear = true;
                        self.enter_task_list(msg, Some(&id));
                    } else {
                        self.needs_clear = true;
                        self.enter_task_list(None, Some(&id));
                    }
                }
            }
            KeyCode::Char(c)
                if c == km.status_cycle.chars().next().unwrap_or('f')
                    && km.status_cycle.len() == 1
                    && key.modifiers == KeyModifiers::NONE =>
            {
                let sel = *selected;
                if let Some(task) = tasks.get(sel).cloned() {
                    self.cycle_status(&task.id);
                }
            }
            // b — toggle personal ↔ backlog context
            KeyCode::Char('b') if key.modifiers == KeyModifiers::NONE => {
                self.context = match self.context {
                    TaskContext::Personal => TaskContext::Backlog,
                    TaskContext::Backlog => TaskContext::Personal,
                };
                self.enter_task_list(None, None);
            }
            // t — team view (read-only, all users' active tasks)
            KeyCode::Char('t') if key.modifiers == KeyModifiers::NONE => {
                let tasks = self.repo.as_ref()
                    .and_then(|r| r.list_team_tasks().ok())
                    .unwrap_or_default();
                self.screen = Screen::TeamView { tasks, selected: 0 };
            }
            // Ctrl+A — assign selected task
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let sel = *selected;
                if let Some(task) = tasks.get(sel).cloned() {
                    let users = self.repo.as_ref()
                        .and_then(|r| r.list_users().ok())
                        .unwrap_or_default();
                    self.screen = Screen::AssignTask {
                        task_id: task.id.clone(),
                        users,
                        selected: 0,
                        filter: String::new(),
                    };
                }
            }
            // Ctrl+D — delete selected task
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let sel = *selected;
                if let Some(task) = tasks.get(sel).cloned() {
                    self.screen = Screen::DeleteConfirm {
                        task_id: task.id.clone(),
                        task_title: task.title.clone(),
                    };
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
            self.enter_task_list(None, Some(&task_id));
            return;
        }
        if is_key(&key, &km.edit) {
            // Clone task data before releasing borrow on self.screen
            let (title, desc, ctx) = {
                let Screen::Detail { task, .. } = &self.screen else { return };
                (task.title.clone(), task.description.clone().unwrap_or_default(), self.context)
            };
            if let Some((new_title, new_desc)) = edit_task_in_editor(&title, &desc) {
                let msg = self.repo.as_ref().map(|r| match ctx {
                    TaskContext::Personal => r.update_task(&task_id, Some(new_title), new_desc),
                    TaskContext::Backlog => r.update_backlog_task(&task_id, Some(new_title), new_desc),
                }).map(|res| match res {
                    Ok(_) => "Task updated.".to_string(),
                    Err(e) => e.to_string(),
                });
                self.needs_clear = true;
                self.enter_task_list(msg, Some(&task_id));
            } else {
                self.needs_clear = true;
                self.enter_task_list(None, Some(&task_id));
            }
            return;
        }
        if is_key(&key, &km.status_cycle) {
            self.cycle_status(&task_id);
            // Reload the task after cycling
            if let Some(repo) = &self.repo {
                if let Ok((tasks, _)) = repo.list_tasks() {
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

        let Screen::Create { title, task_type, assignee, description, field } = &mut self.screen else {
            return;
        };

        // Ctrl+S saves from any field
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            if title.trim().is_empty() { return; }
            let t = title.trim().to_string();
            let tt = task_type.clone();
            let desc = Some(description.trim().to_string()).filter(|s| !s.is_empty());
            let asgn = Some(assignee.trim().to_string()).filter(|s| !s.is_empty());
            let ctx = self.context;
            if let Some(repo) = &self.repo {
                let result = match ctx {
                    TaskContext::Personal => repo.create_task(t, tt, desc),
                    TaskContext::Backlog => repo.create_backlog_task(t, tt, desc),
                };
                match result {
                    Ok(task) => {
                        if let Some(a) = asgn {
                            let _ = match ctx {
                                TaskContext::Personal => repo.assign_task(&task.id, Some(a)),
                                TaskContext::Backlog => repo.assign_backlog_task(&task.id, Some(a)),
                            };
                        }
                        self.enter_task_list(Some("Task created.".into()), None);
                    }
                    Err(e) => self.enter_task_list(Some(e.to_string()), None),
                }
            }
            return;
        }

        match key.code {
            KeyCode::Esc => self.enter_task_list(None, None),
            KeyCode::Tab => {
                *field = match field {
                    CreateField::Title => CreateField::Type,
                    CreateField::Type => CreateField::Assignee,
                    CreateField::Assignee => CreateField::Description,
                    CreateField::Description => CreateField::Title,
                };
            }
            KeyCode::Enter if *field == CreateField::Title => { *field = CreateField::Type; }
            KeyCode::Enter if *field == CreateField::Type => { *field = CreateField::Assignee; }
            KeyCode::Enter if *field == CreateField::Assignee => {
                let t = title.clone();
                let tt = task_type.clone();
                let d = description.clone();
                let users = self.repo.as_ref()
                    .and_then(|r| r.list_users().ok())
                    .unwrap_or_default();
                self.screen = Screen::PickAssignee {
                    title: t, task_type: tt, description: d,
                    users, selected: 0, filter: String::new(),
                };
                return;
            }
            KeyCode::Enter if *field == CreateField::Description => {
                let t = title.clone();
                let tt = task_type.clone();
                let asgn = assignee.clone();
                let desc = description.clone();
                let edited = open_in_editor(if desc.is_empty() { "" } else { &desc });
                self.needs_clear = true;
                self.screen = Screen::Create {
                    title: t,
                    task_type: tt,
                    assignee: asgn,
                    description: edited
                        .map(|s| s.trim().to_string())
                        .unwrap_or(desc),
                    field: CreateField::Description,
                };
                return;
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
    }

    // ── sync confirm ──────────────────────────────────────────────────────────

    fn handle_sync_confirm(&mut self, key: KeyEvent) {
        match key.code {
            // y confirms — N is the default so Enter cancels
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let result = self.repo.as_ref()
                    .map(|r| match self.context {
                        TaskContext::Personal => r.push(),
                        TaskContext::Backlog => r.push_backlog(),
                    })
                    .unwrap_or(Err(git_task_core::error::AppError::NoRepo));
                let msg = match result {
                    Ok(_) => "Successfully pushed.".into(),
                    Err(e) => classify_push_error(&e.to_string()),
                };
                self.enter_task_list(Some(msg), None);
            }
            KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('n') | KeyCode::Char('N') => {
                self.enter_task_list(None, None);
            }
            _ => {}
        }
    }

    // ── push prompt (on quit) ─────────────────────────────────────────────────

    fn handle_push_prompt(&mut self, key: KeyEvent) {
        match key.code {
            // y pushes then quits — N is the default so Enter just quits
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.exit_message = Some(match self.repo.as_ref().map(|r| r.push()) {
                    Some(Ok(_)) => "Successfully pushed.".into(),
                    Some(Err(e)) => classify_push_error(&e.to_string()),
                    None => "No repo connected.".into(),
                });
                self.should_quit = true;
            }
            // Enter or n quits without pushing
            KeyCode::Enter | KeyCode::Char('n') | KeyCode::Char('N') => {
                self.should_quit = true;
            }
            // Esc cancels the quit entirely — back to task list
            KeyCode::Esc => {
                self.enter_task_list(None, None);
            }
            _ => {}
        }
    }


    // ── helpers ───────────────────────────────────────────────────────────────

    // Check 1: has git-task.toml → open.
    // Check 2: has other files, no git-task.toml → assume code repo, reject.
    // Check 3: only README/.gitignore/empty → offer to initialize.
    fn evaluate_path(&mut self, path: String, can_cancel: bool) {
        let path = expand_tilde(&path);
        let p = Path::new(&path);

        if !p.join(".git").exists() {
            self.screen = Screen::Setup {
                input: path,
                error: Some("Not a git repository.".into()),
                can_cancel,
            };
            return;
        }

        if p.join("git-task.toml").exists() {
            match TaskRepo::open(&path) {
                Ok(repo) => {
                    self.config.repo_path = Some(path);
                    self.config.save();
                    self.repo = Some(repo);
                    let repo_path = self.repo.as_ref().unwrap().info.path.clone();
                    let (lock_path, lock_warn) = acquire_lock(&repo_path);
                    self.lock_path = lock_path;
                    let (pull_msg, pull_err) = classify_pull_result(self.repo.as_ref().unwrap().pull());
                    self.pull_error = pull_err;
                    self.enter_task_list(merge_messages(pull_msg, lock_warn), None);
                }
                Err(e) => {
                    self.screen = Screen::Setup { input: path, error: Some(e.to_string()), can_cancel };
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
                can_cancel,
            };
        }
    }

    fn enter_task_list(&mut self, message: Option<String>, preserve_id: Option<&str>) {
        let (tasks, warnings) = self.repo.as_ref().map(|r| match self.context {
            TaskContext::Personal => r.list_tasks().unwrap_or_default(),
            TaskContext::Backlog => r.list_backlog_tasks().unwrap_or_default(),
        }).unwrap_or_default();
        let msg = merge_messages(message, warn_summary(&warnings));
        let sorted = sort_for_display(tasks);
        let selected = preserve_id
            .and_then(|id| sorted.iter().position(|t| t.id == id))
            .unwrap_or(0);
        self.screen = Screen::TaskList { tasks: sorted, selected, message: msg };
    }

    fn cycle_status(&mut self, task_id: &str) {
        let Some(repo) = &self.repo else { return };
        let ctx = self.context;

        let (tasks, _) = match ctx {
            TaskContext::Personal => repo.list_tasks().unwrap_or_default(),
            TaskContext::Backlog => repo.list_backlog_tasks().unwrap_or_default(),
        };
        let Some(task) = tasks.iter().find(|t| t.id == task_id) else { return };

        let result = match (ctx, &task.status) {
            (TaskContext::Personal, TaskStatus::Open) => repo.set_task_in_progress(task_id),
            (TaskContext::Personal, TaskStatus::InProgress) => repo.mark_task_done(task_id),
            (TaskContext::Personal, TaskStatus::Done) => repo.set_task_in_progress(task_id),
            (TaskContext::Backlog, TaskStatus::Open) => repo.set_backlog_task_in_progress(task_id),
            (TaskContext::Backlog, TaskStatus::InProgress) => repo.mark_backlog_task_done(task_id),
            (TaskContext::Backlog, TaskStatus::Done) => repo.set_backlog_task_in_progress(task_id),
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
            self.enter_task_list(msg, Some(task_id));
        }
    }

    fn handle_pick_assignee(&mut self, key: KeyEvent) {
        let Screen::PickAssignee { title, task_type, description, users, selected, filter } = &mut self.screen else { return };

        let filtered_len = users.iter().filter(|u| u.to_lowercase().starts_with(&filter.to_lowercase())).count();

        match key.code {
            KeyCode::Up => {
                if *selected > 0 { *selected -= 1; }
            }
            KeyCode::Down => {
                if filtered_len > 0 && *selected < filtered_len - 1 { *selected += 1; }
            }
            KeyCode::Backspace => {
                filter.pop();
                *selected = 0;
            }
            KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE => {
                filter.push(c);
                *selected = 0;
            }
            KeyCode::Enter => {
                let f = filter.to_lowercase();
                let assignee = users.iter()
                    .filter(|u| u.to_lowercase().starts_with(&f))
                    .nth(*selected)
                    .cloned()
                    .unwrap_or_default();
                let t = title.clone(); let tt = task_type.clone(); let d = description.clone();
                self.screen = Screen::Create {
                    title: t, task_type: tt, assignee, description: d,
                    field: CreateField::Assignee,
                };
            }
            KeyCode::Esc => {
                let t = title.clone(); let tt = task_type.clone(); let d = description.clone();
                let default = self.repo.as_ref().map(|r| r.info.username.clone()).unwrap_or_default();
                self.screen = Screen::Create {
                    title: t, task_type: tt, assignee: default, description: d,
                    field: CreateField::Assignee,
                };
            }
            _ => {}
        }
    }

    fn handle_assign_task(&mut self, key: KeyEvent) {
        let Screen::AssignTask { task_id, users, selected, filter } = &mut self.screen else { return };

        let filtered_len = users.iter().filter(|u| u.to_lowercase().starts_with(&filter.to_lowercase())).count();

        match key.code {
            KeyCode::Up => {
                if *selected > 0 { *selected -= 1; }
            }
            KeyCode::Down => {
                if filtered_len > 0 && *selected < filtered_len - 1 { *selected += 1; }
            }
            KeyCode::Backspace => {
                filter.pop();
                *selected = 0;
            }
            KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE => {
                filter.push(c);
                *selected = 0;
            }
            KeyCode::Enter => {
                let f = filter.to_lowercase();
                let assignee = users.iter()
                    .filter(|u| u.to_lowercase().starts_with(&f))
                    .nth(*selected)
                    .cloned();
                let id = task_id.clone();
                let result = self.repo.as_ref().map(|r| match self.context {
                    TaskContext::Personal => r.assign_task(&id, assignee),
                    TaskContext::Backlog => r.assign_backlog_task(&id, assignee),
                });
                let msg = match result {
                    Some(Ok(_)) => Some("Assigned.".into()),
                    Some(Err(e)) => Some(e.to_string()),
                    None => None,
                };
                self.enter_task_list(msg, Some(&id));
            }
            KeyCode::Esc => {
                let id = task_id.clone();
                self.enter_task_list(None, Some(&id));
            }
            _ => {}
        }
    }

    fn handle_delete_confirm(&mut self, key: KeyEvent) {
        let Screen::DeleteConfirm { task_id, .. } = &self.screen else { return };
        let id = task_id.clone();

        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let result = self.repo.as_ref().map(|r| match self.context {
                    TaskContext::Personal => r.delete_task(&id),
                    TaskContext::Backlog => r.delete_backlog_task(&id),
                });
                let msg = match result {
                    Some(Ok(())) => Some("Task deleted.".into()),
                    Some(Err(e)) => Some(e.to_string()),
                    None => None,
                };
                self.enter_task_list(msg, None);
            }
            KeyCode::Enter | KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.enter_task_list(None, Some(&id));
            }
            _ => {}
        }
    }

    fn handle_team_view(&mut self, key: KeyEvent) {
        if is_ctrl_q(&key) { self.try_quit(); return; }
        let Screen::TeamView { tasks, selected } = &mut self.screen else { return };
        let count = tasks.len();
        match key.code {
            KeyCode::Char('w') | KeyCode::Up => {
                if count > 0 { *selected = selected.checked_sub(1).unwrap_or(count - 1); }
            }
            KeyCode::Char('s') | KeyCode::Down => {
                if count > 0 { *selected = (*selected + 1) % count; }
            }
            KeyCode::Char('a') | KeyCode::Esc | KeyCode::Char('q') => {
                self.enter_task_list(None, None);
            }
            _ => {}
        }
    }

    fn try_quit(&mut self) {
        self.screen = Screen::PushPrompt;
    }

    /// Removes the session lock file. Call only on clean exit.
    pub fn cleanup(&self) {
        if let Some(ref path) = self.lock_path {
            let _ = fs::remove_file(path);
        }
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

/// Writes a session lock file for `repo_path` and returns its path plus an
/// optional warning when a stale or concurrent lock is detected.
fn acquire_lock(repo_path: &str) -> (Option<PathBuf>, Option<String>) {
    let Some(path) = lock_file_path(repo_path) else { return (None, None) };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let warn = if path.exists() {
        stale_lock_warning(&path)
    } else {
        None
    };
    let _ = fs::write(&path, std::process::id().to_string());
    (Some(path), warn)
}

/// Returns a user-facing warning if the lock at `path` belongs to a dead or
/// concurrent process, or `None` if the lock is absent or owned by us.
fn stale_lock_warning(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let pid: u32 = content.trim().parse().ok()?;
    if pid == std::process::id() {
        return None;
    }
    if process_running(pid) {
        Some("Another git-task session is already open for this repo".to_string())
    } else {
        Some("Last session ended without pushing — consider ^R to sync".to_string())
    }
}

/// Returns the lock file path for a repo, stored in the config directory so
/// it does not appear as an untracked file in the cake repo.
fn lock_file_path(repo_path: &str) -> Option<PathBuf> {
    let key: String = repo_path
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect();
    dirs::config_dir().map(|d| d.join("git-task").join(format!("{key}.lock")))
}

/// Checks whether a process with the given PID is currently running.
fn process_running(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// Formats a list of unreadable filenames into a single warning string.
/// Returns `None` when the list is empty.
fn warn_summary(warnings: &[String]) -> Option<String> {
    match warnings.len() {
        0 => None,
        1 => Some(format!("1 slice could not be read: {}", warnings[0])),
        n => Some(format!("{n} slices could not be read")),
    }
}

/// Combines an optional primary message with an optional warning.
/// Both present → joined with " · "; otherwise whichever is `Some`.
fn merge_messages(primary: Option<String>, secondary: Option<String>) -> Option<String> {
    match (primary, secondary) {
        (Some(a), Some(b)) => Some(format!("{a} · {b}")),
        (a, None) => a,
        (None, b) => b,
    }
}

/// Converts a push error into a user-facing message.
/// Rejected/conflict pushes get resolution steps; other errors get the raw text.
fn classify_push_error(err: &str) -> String {
    let lower = err.to_lowercase();
    if lower.contains("rejected")
        || lower.contains("non-fast-forward")
        || lower.contains("fetch first")
        || lower.contains("updates were rejected")
    {
        "Push rejected: remote has new commits — run `git pull` in the repo, then ^R to retry".to_string()
    } else {
        format!("Sync failed: {err}")
    }
}

/// Interprets a pull result into an ephemeral message and a persistent error.
/// Auth and network failures are classified so the user gets actionable text.
fn classify_pull_result(result: Result<String, git_task_core::error::AppError>) -> (Option<String>, Option<String>) {
    match result {
        Ok(out) if out.trim().is_empty() || out.contains("Already up to date") => (None, None),
        Ok(_) => (Some("Pulled latest changes.".to_string()), None),
        Err(e) => {
            let raw = e.to_string();
            let lower = raw.to_lowercase();
            let msg = if lower.contains("permission denied")
                || lower.contains("authentication failed")
                || lower.contains("could not read username")
                || lower.contains("access denied")
                || lower.contains("publickey")
                || lower.contains("invalid username or password")
            {
                "Pull failed: auth error — SSH key not loaded or credentials expired".to_string()
            } else if lower.contains("could not resolve host")
                || lower.contains("could not resolve hostname")
                || lower.contains("network is unreachable")
                || lower.contains("connection timed out")
                || lower.contains("no route to host")
                || lower.contains("unable to connect")
            {
                "Pull failed: no network — check VPN or connection".to_string()
            } else {
                format!("Pull failed: {raw}")
            };
            (None, Some(msg))
        }
    }
}

/// Sorts tasks into display order: in-progress → open → done.
/// This makes `selected` a direct index into visual position.
fn sort_for_display(mut tasks: Vec<Task>) -> Vec<Task> {
    tasks.sort_by_key(|t| match (&t.status, t.is_completed) {
        (TaskStatus::InProgress, false) => 0,
        (TaskStatus::Open, false) => 1,
        _ => 2,
    });
    tasks
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

// Opens a task for editing as "# Title\n\nDescription".
// Returns (new_title, new_description) parsed from the saved file,
// or None if the editor was cancelled or no # heading was found.
fn edit_task_in_editor(title: &str, description: &str) -> Option<(String, Option<String>)> {
    let content = if description.trim().is_empty() {
        format!("# {title}\n")
    } else {
        format!("# {title}\n\n{description}")
    };
    let edited = open_in_editor(&content)?;
    parse_editor_content(&edited)
}

// Parses "# Title\n\nDescription" back into (title, description).
fn parse_editor_content(content: &str) -> Option<(String, Option<String>)> {
    let mut title = String::new();
    let mut after_heading = false;
    let mut desc_lines: Vec<&str> = Vec::new();

    for line in content.lines() {
        if !after_heading && line.starts_with("# ") {
            title = line.trim_start_matches("# ").trim().to_string();
            after_heading = true;
        } else if after_heading {
            desc_lines.push(line);
        }
    }

    if title.is_empty() {
        return None;
    }

    let desc = desc_lines.join("\n").trim().to_string();
    Some((title, if desc.is_empty() { None } else { Some(desc) }))
}

// Suspends ratatui, opens content in $VISUAL/$EDITOR, resumes ratatui.
// Returns the edited content, or None if the editor couldn't be launched.
fn open_in_editor(content: &str) -> Option<String> {
    use crossterm::event::{DisableMouseCapture, EnableMouseCapture};

    let tmp_path = env::temp_dir().join(format!("gt-desc-{}.md", std::process::id()));
    fs::write(&tmp_path, content).ok()?;

    let _ = disable_raw_mode();
    let _ = execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture);

    match env::var("VISUAL").or_else(|_| env::var("EDITOR")) {
        Ok(editor) => {
            let _ = Command::new(&editor).arg(&tmp_path).status();
        }
        Err(_) => {
            // No $EDITOR/$VISUAL. Prefer nano (it shows its own key hints at the bottom).
            // Fall back to vi with explicit hints if nano is not on PATH.
            let nano_result = Command::new("nano").arg(&tmp_path).status();
            let nano_missing = matches!(&nano_result, Err(e) if e.kind() == std::io::ErrorKind::NotFound);
            if nano_missing {
                let _ = Command::new("vi").arg(&tmp_path).status();
            }
        }
    }

    let _ = enable_raw_mode();
    let _ = execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture);

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
