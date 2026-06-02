use std::{env, fs, path::{Path, PathBuf}, process::Command};

use crossterm::{
    event::{Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use gitcake_core::{
    models::{
        cake::{Cake, NewCake},
        task::{NewTask, Priority, Task, TaskPatch, TaskStatus, TaskType},
    },
    repo::TaskRepo,
};

use crate::config::Config;

const DESCRIPTION_MAX_CHARS: usize = 850;
const DESC_TOO_LONG_MSG: &str = "Whoa there buddy, this is a task tracker, not The Lord of The Rings! Keep it under 850 characters.";

fn desc_too_long(len: usize) -> String {
    format!("{DESC_TOO_LONG_MSG} ({len}/{DESCRIPTION_MAX_CHARS})")
}

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
        selected_field: DetailField,
        from_planner: bool,
    },
    Create {
        task_type: TaskType,
        assignee: String,
        cake_id: Option<String>,
        field: CreateField,
    },
    CreateCake {
        title: String,
    },
    PickCake {
        task_id: String,
        cakes: Vec<Cake>,
        selected: usize,
        filter: String,
        from_create: bool,
        create_task_type: Option<TaskType>,
        create_assignee: Option<String>,
    },
    AssignTask {
        task_id: String,
        users: Vec<String>,
        selected: usize,
        filter: String,
    },
    PickAssignee {
        task_type: TaskType,
        prev_assignee: String,
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
    PlannerView {
        cakes: Vec<Cake>,
        tasks: Vec<(String, Task)>,
        selected: usize,
    },
}

#[derive(PartialEq, Clone, Copy)]
pub enum CreateField {
    Type,
    Assignee,
    Cake,
    Confirm,
}

impl CreateField {
    pub fn next(self) -> Self {
        match self {
            Self::Type     => Self::Assignee,
            Self::Assignee => Self::Cake,
            Self::Cake     => Self::Confirm,
            Self::Confirm  => Self::Type,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Self::Type     => Self::Confirm,
            Self::Assignee => Self::Type,
            Self::Cake     => Self::Assignee,
            Self::Confirm  => Self::Cake,
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum DetailField {
    Type,
    Status,
    Priority,
    AiFlagged,
    Cake,
}

impl DetailField {
    pub fn next(self) -> Self {
        match self {
            Self::Type     => Self::Status,
            Self::Status   => Self::Priority,
            Self::Priority => Self::AiFlagged,
            Self::AiFlagged  => Self::Cake,
            Self::Cake     => Self::Type,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Self::Type     => Self::Cake,
            Self::Status   => Self::Type,
            Self::Priority => Self::Status,
            Self::AiFlagged  => Self::Priority,
            Self::Cake     => Self::AiFlagged,
        }
    }
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
    pub lock_warning: Option<String>,
    pub lock_path: Option<PathBuf>,
    /// Persists across screen transitions — cleared only by Esc.
    pub filter: String,
    pub filter_active: bool,
    /// Cached cake list for detail view and pickers. Refreshed when entering planner.
    pub cached_cakes: Vec<Cake>,
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
                lock_warning: None,
                lock_path: None,
                filter: String::new(),
                filter_active: false,
                cached_cakes: Vec::new(),
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
                lock_warning: None,
                lock_path: None,
                filter: String::new(),
                filter_active: false,
                cached_cakes: Vec::new(),
            };
        }

        // Auto-pull on startup; failures are non-fatal
        let (pull_msg, pull_error) = classify_pull_result(repo.as_ref().unwrap().pull());

        let repo_path = repo.as_ref().unwrap().info.path.clone();
        let (lock_path, lock_warning) = acquire_lock(&repo_path);

        let (raw_tasks, task_warnings) = repo.as_ref().unwrap().list_tasks().unwrap_or_default();
        let startup_msg = merge_messages(pull_msg, warn_summary(&task_warnings));
        let tasks = sort_for_display(raw_tasks);
        let screen = Screen::TaskList { tasks, selected: 0, message: startup_msg };

        Self { screen, repo, config, context: TaskContext::Personal, should_quit: false, needs_clear: false, exit_message: None, pull_error, lock_warning, lock_path, filter: String::new(), filter_active: false, cached_cakes: Vec::new() }
    }

    pub fn handle_event(&mut self, event: Event) {
        let Event::Key(key) = event else { return };
        if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
            let input_screen = matches!(&self.screen,
                Screen::Setup { .. } | Screen::InitRepo { .. } | Screen::Create { .. }
                | Screen::CreateCake { .. } | Screen::AssignTask { .. }
                | Screen::PickAssignee { .. } | Screen::DeleteConfirm { .. }
                | Screen::SyncConfirm | Screen::PushPrompt | Screen::PickCake { .. }
            );
            if !input_screen {
                if let Some(r) = &self.repo {
                    let assignee = r.info.username.clone();
                    self.screen = Screen::Create { task_type: TaskType::Task, assignee, cake_id: None, field: CreateField::Type };
                }
                return;
            }
        }
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
            Screen::PlannerView { .. } => self.handle_planner_view(key),
            Screen::CreateCake { .. } => self.handle_create_cake(key),
            Screen::PickCake { .. } => self.handle_pick_cake(key),
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
        if is_key(&key, &self.config.keys.quit) { self.try_quit(); return; }
        if self.filter_active { self.handle_filter_input(key); return; }
        if is_key(&key, &self.config.keys.push) { self.screen = Screen::SyncConfirm; return; }
        if key.code == KeyCode::Esc {
            if !self.filter.is_empty() {
                self.filter.clear();
                if let Screen::TaskList { selected, .. } = &mut self.screen { *selected = 0; }
                return;
            }
            self.pull_error = None;
            return;
        }
        if key.code == KeyCode::Char('R') && !key.modifiers.contains(KeyModifiers::CONTROL) {
            self.handle_list_pull();
            return;
        }
        let Screen::TaskList { tasks, selected, message } = &mut self.screen else { return };
        if key.code == KeyCode::Char('/') && key.modifiers == KeyModifiers::NONE {
            self.filter_active = true;
            self.filter.clear();
            *selected = 0;
            return;
        }
        let filter = self.filter.clone();
        let visible = apply_filter_indices(tasks, &filter);
        let vc = visible.len();
        let no_mod = key.modifiers == KeyModifiers::NONE;
        let up_k = self.config.keys.up.chars().next().unwrap_or('w');
        let dn_k = self.config.keys.down.chars().next().unwrap_or('s');
        let is_up = key.code == KeyCode::Up
            || (matches!(key.code, KeyCode::Char(c) if c == up_k) && self.config.keys.up.len() == 1 && no_mod);
        let is_dn = key.code == KeyCode::Down
            || (matches!(key.code, KeyCode::Char(c) if c == dn_k) && self.config.keys.down.len() == 1 && no_mod);
        if is_up && vc > 0 { *selected = selected.checked_sub(1).unwrap_or(vc - 1); *message = None; return; }
        if is_dn && vc > 0 { *selected = (*selected + 1) % vc; *message = None; return; }
        let sel_task = visible.get(*selected).and_then(|&i| tasks.get(i)).cloned();
        self.handle_list_action_keys(key, sel_task, no_mod);
    }

    fn handle_list_action_keys(&mut self, key: KeyEvent, sel_task: Option<Task>, no_mod: bool) {
        let km = &self.config.keys;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char(c) if c == km.detail.chars().next().unwrap_or('d') && km.detail.len() == 1 && no_mod => {
                if let Some(t) = sel_task { self.screen = Screen::Detail { task: t, message: None, selected_field: DetailField::Type, from_planner: false }; }
            }
            KeyCode::Char(c) if c == km.edit.chars().next().unwrap_or('e') && km.edit.len() == 1 && no_mod => {
                if let Some(t) = sel_task { self.do_task_list_edit(t.id, t.title, t.description.unwrap_or_default()); }
            }
            KeyCode::Char(c) if c == km.status_cycle.chars().next().unwrap_or('f') && km.status_cycle.len() == 1 && no_mod && self.context == TaskContext::Personal => {
                if let Some(t) = sel_task { self.cycle_status(&t.id); }
            }
            KeyCode::Char('f') if no_mod && self.context == TaskContext::Backlog => {
                if let Some(t) = sel_task { self.do_claim_backlog(t.id); }
            }
            KeyCode::Tab => {
                self.filter.clear(); self.filter_active = false;
                match self.context {
                    TaskContext::Personal => self.enter_planner_view(),
                    TaskContext::Backlog  => { self.context = TaskContext::Personal; self.enter_task_list(None, None); }
                }
            }
            KeyCode::BackTab => {
                self.filter.clear(); self.filter_active = false;
                match self.context {
                    TaskContext::Personal => { self.context = TaskContext::Backlog; self.enter_task_list(None, None); }
                    TaskContext::Backlog  => self.enter_planner_view(),
                }
            }
            KeyCode::Char('a') if ctrl => {
                if let Some(t) = sel_task {
                    let users = self.repo.as_ref().and_then(|r| r.list_users().ok()).unwrap_or_default();
                    self.screen = Screen::AssignTask { task_id: t.id, users, selected: 0, filter: String::new() };
                }
            }
            KeyCode::Char('b') if ctrl && self.context == TaskContext::Personal => {
                if let Some(t) = sel_task { self.screen = Screen::DeleteConfirm { task_id: t.id, task_title: t.title }; }
            }
            KeyCode::Char('d') if ctrl && self.context == TaskContext::Backlog => {
                if let Some(t) = sel_task { self.screen = Screen::DeleteConfirm { task_id: t.id, task_title: t.title }; }
            }
            _ => {}
        }
    }

    fn handle_filter_input(&mut self, key: KeyEvent) {
        let Screen::TaskList { selected, .. } = &mut self.screen else { return };
        match key.code {
            KeyCode::Esc => {
                if self.filter.is_empty() { self.filter_active = false; }
                else { self.filter.clear(); *selected = 0; }
            }
            KeyCode::Enter    => { self.filter_active = false; }
            KeyCode::Backspace => { self.filter.pop(); *selected = 0; }
            KeyCode::Char(c)  => { self.filter.push(c); *selected = 0; }
            _ => {}
        }
    }

    fn handle_list_pull(&mut self) {
        let current_id = if let Screen::TaskList { tasks, selected, .. } = &self.screen {
            let visible = apply_filter_indices(tasks, &self.filter);
            visible.get(*selected).and_then(|&i| tasks.get(i)).map(|t| t.id.clone())
        } else {
            None
        };
        let pull_result = self.repo.as_ref().map(|r| r.pull());
        let (pull_msg, pull_err) = match pull_result {
            Some(r) => classify_pull_result(r),
            None    => (None, Some("No repo connected.".to_string())),
        };
        let no_error = pull_err.is_none();
        self.pull_error = pull_err;
        let msg = pull_msg.or_else(|| no_error.then(|| "Already up to date.".to_string()));
        self.enter_task_list(msg, current_id.as_deref());
    }

    fn do_task_list_edit(&mut self, id: String, title: String, desc: String) {
        let edited = edit_task_in_editor(&title, &desc);
        self.needs_clear = true;
        if let Some((new_title, new_desc)) = edited {
            let char_count = new_desc.as_deref().unwrap_or("").chars().count();
            if char_count > DESCRIPTION_MAX_CHARS {
                self.enter_task_list(Some(desc_too_long(char_count)), Some(&id));
                return;
            }
            let msg = self.repo.as_ref().map(|r| {
                r.update_task(&id, TaskPatch { title: Some(new_title), description: Some(new_desc), ..Default::default() })
            }).map(|res| match res {
                Ok(_) => "Task updated.".to_string(),
                Err(e) => e.to_string(),
            });
            self.enter_task_list(msg, Some(&id));
        } else {
            self.enter_task_list(None, Some(&id));
        }
    }

    fn do_claim_backlog(&mut self, task_id: String) {
        let msg = self.repo.as_ref().map(|r| match r.claim_backlog_task(&task_id) {
            Ok(t)  => format!("Claimed — now #{} in your personal slices.", t.id),
            Err(e) => e.to_string(),
        });
        self.enter_task_list(msg, Some(&task_id));
    }

    fn enter_planner_view(&mut self) {
        let cakes = self.repo.as_ref().and_then(|r| r.list_cakes().ok()).unwrap_or_default();
        let tasks = self.repo.as_ref().and_then(|r| r.list_team_tasks().ok()).unwrap_or_default();
        self.cached_cakes = cakes.clone();
        self.screen = Screen::PlannerView { cakes, tasks, selected: 0 };
    }

    // ── detail ────────────────────────────────────────────────────────────────

    fn handle_detail(&mut self, key: KeyEvent) {
        let km = &self.config.keys;
        if is_ctrl_q(&key) { self.try_quit(); return; }
        if is_key(&key, &km.push) { self.screen = Screen::SyncConfirm; return; }
        if key.code == KeyCode::Char('R') && !key.modifiers.contains(KeyModifiers::CONTROL) {
            let (task_id, from_planner) = if let Screen::Detail { task, from_planner, .. } = &self.screen {
                (task.id.clone(), *from_planner)
            } else { return };
            let (pull_msg, pull_err) = match self.repo.as_ref().map(|r| r.pull()) {
                Some(r) => classify_pull_result(r),
                None    => (None, Some("No repo connected.".to_string())),
            };
            self.pull_error = pull_err;
            if from_planner { self.enter_planner_view(); } else { self.enter_task_list(pull_msg, Some(&task_id)); }
            return;
        }
        let (task_id, field, task_type, task_status, task_priority, task_ai_flagged, title, desc, from_planner) =
            match &self.screen {
                Screen::Detail { task, selected_field, from_planner, .. } => (
                    task.id.clone(), *selected_field,
                    task.task_type.clone(), task.status.clone(),
                    task.priority.clone(), task.ai_flagged,
                    task.title.clone(), task.description.clone().unwrap_or_default(),
                    *from_planner,
                ),
                _ => return,
            };
        if is_key(&key, &km.back) || key.code == KeyCode::Char('q') {
            if from_planner { self.enter_planner_view(); } else { self.enter_task_list(None, Some(&task_id)); }
            return;
        }
        if is_key(&key, &km.edit) {
            self.do_detail_edit(task_id, title, desc, from_planner); return;
        }
        if matches!(key.code, KeyCode::Char('w') | KeyCode::Up) && key.modifiers == KeyModifiers::NONE {
            if let Screen::Detail { selected_field, .. } = &mut self.screen { *selected_field = selected_field.prev(); }
            return;
        }
        if matches!(key.code, KeyCode::Char('s') | KeyCode::Down) && key.modifiers == KeyModifiers::NONE {
            if let Screen::Detail { selected_field, .. } = &mut self.screen { *selected_field = selected_field.next(); }
            return;
        }
        if is_key(&key, &km.status_cycle) {
            self.do_detail_field_cycle(task_id, field, task_type, task_status, task_priority, task_ai_flagged);
        }
    }

    fn do_detail_edit(&mut self, id: String, title: String, desc: String, from_planner: bool) {
        let ctx = self.context;
        let edited = edit_task_in_editor(&title, &desc);
        self.needs_clear = true;
        if let Some((new_title, new_desc)) = edited {
            let char_count = new_desc.as_deref().unwrap_or("").chars().count();
            if char_count > DESCRIPTION_MAX_CHARS {
                let msg = Some(desc_too_long(char_count));
                if from_planner { self.enter_planner_view(); } else { self.enter_task_list(msg, Some(&id)); }
                return;
            }
            let patch = TaskPatch { title: Some(new_title), description: Some(new_desc), ..Default::default() };
            let msg = self.repo.as_ref().map(|r| match ctx {
                TaskContext::Personal => r.update_task(&id, patch),
                TaskContext::Backlog  => r.update_backlog_task(&id, patch),
            }).map(|res| match res {
                Ok(_)  => "Task updated.".to_string(),
                Err(e) => e.to_string(),
            });
            if from_planner { self.enter_planner_view(); } else { self.enter_task_list(msg, Some(&id)); }
        } else if from_planner {
            self.enter_planner_view();
        } else {
            self.enter_task_list(None, Some(&id));
        }
    }

    fn do_detail_field_cycle(
        &mut self, task_id: String, field: DetailField,
        task_type: TaskType, task_status: TaskStatus, task_priority: Priority, task_ai_flagged: bool,
    ) {
        let updated = match field {
            DetailField::Type => {
                self.repo.as_ref().and_then(|r| r.change_task_type(&task_id, next_task_type(task_type)).ok())
            }
            DetailField::Status => {
                let ctx = self.context;
                self.repo.as_ref().and_then(|r| match (ctx, &task_status) {
                    (TaskContext::Personal, TaskStatus::Open)       => r.set_task_in_progress(&task_id).ok(),
                    (TaskContext::Personal, TaskStatus::InProgress) => r.mark_task_done(&task_id).ok(),
                    (TaskContext::Personal, TaskStatus::Done)       => r.set_task_in_progress(&task_id).ok(),
                    _ => None,
                })
            }
            DetailField::Priority => {
                let next = next_priority(task_priority);
                self.repo.as_ref().and_then(|r| r.update_task(&task_id, TaskPatch { priority: Some(next), ..Default::default() }).ok())
            }
            DetailField::AiFlagged => {
                self.repo.as_ref().and_then(|r| r.update_task(&task_id, TaskPatch { ai_flagged: Some(!task_ai_flagged), ..Default::default() }).ok())
            }
            DetailField::Cake => {
                let cakes = self.cached_cakes.clone();
                if cakes.is_empty() {
                    self.cached_cakes = self.repo.as_ref().and_then(|r| r.list_cakes().ok()).unwrap_or_default();
                }
                let cakes = self.cached_cakes.clone();
                self.screen = Screen::PickCake {
                    task_id, cakes, selected: 0, filter: String::new(),
                    from_create: false, create_task_type: None, create_assignee: None,
                };
                return;
            }
        };
        if let (Some(t), Screen::Detail { task, .. }) = (updated, &mut self.screen) {
            *task = t;
        }
    }

    // ── create ────────────────────────────────────────────────────────────────

    fn handle_create(&mut self, key: KeyEvent) {
        if is_ctrl_q(&key) { self.should_quit = true; return; }
        if key.code == KeyCode::Esc { self.enter_task_list(None, None); return; }



        let no_mod = key.modifiers == KeyModifiers::NONE;

        // Read field as Copy before any mutable borrow.
        let field = match &self.screen {
            Screen::Create { field, .. } => *field,
            _ => return,
        };

        if matches!(key.code, KeyCode::Char('w') | KeyCode::Up) && no_mod {
            if let Screen::Create { field: f, .. } = &mut self.screen { *f = f.prev(); }
            return;
        }
        if matches!(key.code, KeyCode::Char('s') | KeyCode::Down) && no_mod {
            if let Screen::Create { field: f, .. } = &mut self.screen { *f = f.next(); }
            return;
        }

        let activate = (key.code == KeyCode::Char('f') || key.code == KeyCode::Enter) && no_mod;
        if !activate { return; }

        match field {
            CreateField::Confirm => self.create_via_editor(),
            CreateField::Type => {
                if let Screen::Create { task_type, .. } = &mut self.screen {
                    *task_type = match task_type {
                        TaskType::Task     => TaskType::Bug,
                        TaskType::Bug      => TaskType::Incident,
                        TaskType::Incident => TaskType::Task,
                    };
                }
            }
            CreateField::Assignee => {
                let (tt, asgn) = match &self.screen {
                    Screen::Create { task_type, assignee, .. } => (task_type.clone(), assignee.clone()),
                    _ => return,
                };
                let users = self.repo.as_ref().and_then(|r| r.list_users().ok()).unwrap_or_default();
                self.screen = Screen::PickAssignee {
                    task_type: tt, prev_assignee: asgn,
                    users, selected: 0, filter: String::new(),
                };
            }
            CreateField::Cake => {
                let (tt, asgn) = match &self.screen {
                    Screen::Create { task_type, assignee, .. } => (task_type.clone(), assignee.clone()),
                    _ => return,
                };
                let cakes = self.cached_cakes.clone();
                self.screen = Screen::PickCake {
                    task_id: String::new(), cakes, selected: 0, filter: String::new(),
                    from_create: true,
                    create_task_type: Some(tt),
                    create_assignee: Some(asgn),
                };
            }
        }
    }

    /// Opens `$EDITOR` with a `# ` template, parses the result, and creates the task.
    /// Returns to the Create screen silently if the editor is cancelled or no title is entered.
    fn create_via_editor(&mut self) {
        let Screen::Create { task_type, assignee, cake_id, .. } = &self.screen else { return };
        let tt = task_type.clone();
        let asgn = assignee.trim().to_string();
        let cid = cake_id.clone();
        let ctx = self.context;

        let edited = open_in_editor("# \n\n");
        self.needs_clear = true;

        let Some(content) = edited else { return };
        let Some((title, desc)) = parse_editor_content(&content) else { return };
        let char_count = desc.as_deref().unwrap_or("").chars().count();
        if char_count > DESCRIPTION_MAX_CHARS {
            self.enter_task_list(Some(desc_too_long(char_count)), None);
            return;
        }
        let asgn_opt = Some(asgn).filter(|s| !s.is_empty());

        if let Some(repo) = &self.repo {
            let result = match ctx {
                TaskContext::Personal => repo.create_task(NewTask { title, task_type: tt, description: desc, cake_id: cid, ..Default::default() }),
                TaskContext::Backlog => repo.create_backlog_task(NewTask { title, task_type: tt, description: desc, cake_id: cid, ..Default::default() }),
            };
            match result {
                Ok(task) => {
                    let msg = if let Some(a) = asgn_opt {
                        match repo.assign_task(&task.id, Some(a)) {
                            Ok(_) => "Task created.".to_string(),
                            Err(e) => format!("Task created but assign failed: {e}"),
                        }
                    } else {
                        "Task created.".to_string()
                    };
                    self.enter_task_list(Some(msg), None);
                }
                Err(e) => self.enter_task_list(Some(e.to_string()), None),
            }
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
                    .unwrap_or(Err(gitcake_core::error::AppError::NoRepo));
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

    // Check 1: has gitcake.toml → open.
    // Check 2: has other files, no gitcake.toml → assume code repo, reject.
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

        if p.join("gitcake.toml").exists() {
            match TaskRepo::open(&path) {
                Ok(repo) => {
                    self.config.repo_path = Some(path);
                    self.config.save();
                    self.repo = Some(repo);
                    let repo_path = self.repo.as_ref().unwrap().info.path.clone();
                    let (lock_path, lock_warning) = acquire_lock(&repo_path);
                    self.lock_path = lock_path;
                    self.lock_warning = lock_warning;
                    let (pull_msg, pull_err) = classify_pull_result(self.repo.as_ref().unwrap().pull());
                    self.pull_error = pull_err;
                    self.enter_task_list(pull_msg, None);
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
                error: Some("This looks like a code repo. Point to a dedicated gitcake repo.".into()),
                can_cancel,
            };
        }
    }

    fn enter_task_list(&mut self, message: Option<String>, preserve_id: Option<&str>) {
        let (tasks, warnings) = self.repo.as_ref().map(|r| match self.context {
            TaskContext::Personal => r.list_tasks().unwrap_or_default(),
            TaskContext::Backlog => {
                let (all, w) = r.list_backlog_tasks().unwrap_or_default();
                let active = all.into_iter()
                    .filter(|t| t.status != TaskStatus::Done)
                    .collect();
                (active, w)
            }
        }).unwrap_or_default();
        let msg = merge_messages(message, warn_summary(&warnings));
        let sorted = sort_for_display(tasks);
        let selected = preserve_id
            .and_then(|id| {
                if self.filter.is_empty() {
                    sorted.iter().position(|t| t.id == id)
                } else {
                    let f = self.filter.to_lowercase();
                    sorted.iter().filter(|t| task_matches(t, &f)).position(|t| t.id == id)
                }
            })
            .unwrap_or(0);
        self.screen = Screen::TaskList { tasks: sorted, selected, message: msg };
    }

    fn cycle_status(&mut self, task_id: &str) {
        let Some(repo) = &self.repo else { return };
        let ctx = self.context;

        let Ok(task) = repo.get_task(task_id) else { return };

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
        let Screen::PickAssignee { task_type, prev_assignee, users, selected, filter } = &mut self.screen else { return };

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
                let tt = task_type.clone();
                self.screen = Screen::Create { task_type: tt, assignee, cake_id: None, field: CreateField::Assignee };
            }
            KeyCode::Esc => {
                let tt = task_type.clone();
                let asgn = prev_assignee.clone();
                self.screen = Screen::Create { task_type: tt, assignee: asgn, cake_id: None, field: CreateField::Assignee };
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
                let owner = users.iter()
                    .filter(|u| u.to_lowercase().starts_with(&f))
                    .nth(*selected)
                    .cloned();
                let id = task_id.clone();
                let result = self.repo.as_ref().map(|r| r.assign_task(&id, owner.clone()));
                let msg = match result {
                    Some(Ok(_)) => Some("Assigned.".into()),
                    Some(Err(e)) => Some(e.to_string()),
                    None => None,
                };
                // If assigning from backlog, switch to personal view so task is visible
                if self.context == TaskContext::Backlog && owner.is_some() {
                    self.context = TaskContext::Personal;
                }
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
                let (result, msg_ok) = match self.context {
                    TaskContext::Personal => (
                        self.repo.as_ref().map(|r| r.move_task_to_backlog(&id)),
                        "Moved to backlog.",
                    ),
                    TaskContext::Backlog => (
                        self.repo.as_ref().map(|r| r.delete_backlog_task(&id)),
                        "Deleted.",
                    ),
                };
                let msg = match result {
                    Some(Ok(())) => Some(msg_ok.into()),
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

    fn handle_planner_view(&mut self, key: KeyEvent) {
        if is_ctrl_q(&key) { self.try_quit(); return; }
        if is_key(&key, &self.config.keys.push) { self.screen = Screen::SyncConfirm; return; }
        if key.code == KeyCode::Char('R') && !key.modifiers.contains(KeyModifiers::CONTROL) {
            self.handle_list_pull(); return;
        }
        if self.filter_active {
            let Screen::PlannerView { selected, .. } = &mut self.screen else { return };
            match key.code {
                KeyCode::Esc       => { if self.filter.is_empty() { self.filter_active = false; } else { self.filter.clear(); *selected = 0; } }
                KeyCode::Enter     => { self.filter_active = false; }
                KeyCode::Backspace => { self.filter.pop(); *selected = 0; }
                KeyCode::Char(c)   => { self.filter.push(c); *selected = 0; }
                _ => {}
            }
            return;
        }
        if key.code == KeyCode::Esc && !self.filter.is_empty() {
            self.filter.clear();
            if let Screen::PlannerView { selected, .. } = &mut self.screen { *selected = 0; }
            return;
        }
        if key.code == KeyCode::Char('d') && key.modifiers == KeyModifiers::NONE {
            let f = if self.filter.is_empty() { String::new() } else { self.filter.to_lowercase() };
            let task = if let Screen::PlannerView { tasks, selected, .. } = &self.screen {
                tasks.iter().filter(|(_, t)| f.is_empty() || task_matches(t, &f)).nth(*selected).map(|(_, t)| t.clone())
            } else { None };
            if let Some(t) = task {
                self.screen = Screen::Detail { task: t, message: None, selected_field: DetailField::Type, from_planner: true };
            }
            return;
        }
        if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::NONE {
            self.screen = Screen::CreateCake { title: String::new() };
            return;
        }
        let Screen::PlannerView { tasks, selected, .. } = &mut self.screen else { return };
        let f = if self.filter.is_empty() { String::new() } else { self.filter.to_lowercase() };
        let visible_count = tasks.iter().filter(|(_, t)| f.is_empty() || task_matches(t, &f)).count();
        match key.code {
            KeyCode::Char('w') | KeyCode::Up => {
                if visible_count > 0 { *selected = selected.checked_sub(1).unwrap_or(visible_count - 1); }
            }
            KeyCode::Char('s') | KeyCode::Down => {
                if visible_count > 0 { *selected = (*selected + 1) % visible_count; }
            }
            KeyCode::Char('/') if key.modifiers == KeyModifiers::NONE => {
                self.filter_active = true; self.filter.clear(); *selected = 0;
            }
            KeyCode::Tab => {
                self.filter.clear(); self.filter_active = false;
                self.context = TaskContext::Backlog; self.enter_task_list(None, None);
            }
            KeyCode::BackTab => {
                self.filter.clear(); self.filter_active = false;
                self.context = TaskContext::Personal; self.enter_task_list(None, None);
            }
            _ => {}
        }
    }

    fn handle_create_cake(&mut self, key: KeyEvent) {
        if is_ctrl_q(&key) { self.try_quit(); return; }
        let Screen::CreateCake { title } = &mut self.screen else { return };
        match key.code {
            KeyCode::Esc => { self.enter_planner_view(); }
            KeyCode::Enter if !title.is_empty() => {
                let t = title.clone();
                let owner = self.repo.as_ref().map(|r| r.info.username.clone());
                let msg = self.repo.as_ref().map(|r| match r.create_cake(NewCake { title: t, owner, ..Default::default() }) {
                    Ok(_)  => "Cake created.".to_string(),
                    Err(e) => e.to_string(),
                });
                let _ = msg;
                self.enter_planner_view();
            }
            KeyCode::Backspace => { title.pop(); }
            KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE => { title.push(c); }
            _ => {}
        }
    }

    fn handle_pick_cake(&mut self, key: KeyEvent) {
        let Screen::PickCake { task_id, cakes, selected, filter, from_create, create_task_type, create_assignee } = &mut self.screen else { return };
        let fl = filter.to_lowercase();
        let filtered: Vec<_> = std::iter::once(None)
            .chain(cakes.iter().map(Some))
            .filter(|c| c.map(|c: &Cake| c.title.to_lowercase().contains(&fl)).unwrap_or(true))
            .collect();
        let filtered_len = filtered.len();
        match key.code {
            KeyCode::Up => { if *selected > 0 { *selected -= 1; } }
            KeyCode::Down => { if filtered_len > 0 && *selected < filtered_len - 1 { *selected += 1; } }
            KeyCode::Backspace => { filter.pop(); *selected = 0; }
            KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE => { filter.push(c); *selected = 0; }
            KeyCode::Enter => {
                let chosen = filtered.get(*selected).and_then(|c| *c).map(|c| c.id.clone());
                if *from_create {
                    let tt = create_task_type.clone().unwrap_or(TaskType::Task);
                    let asgn = create_assignee.clone().unwrap_or_default();
                    self.screen = Screen::Create { task_type: tt, assignee: asgn, cake_id: chosen, field: CreateField::Cake };
                } else {
                    let id = task_id.clone();
                    let patch = TaskPatch { cake_id: Some(chosen), ..Default::default() };
                    if let Some(t) = self.repo.as_ref().and_then(|r| r.update_task(&id, patch).ok()) {
                        self.screen = Screen::Detail { task: t, message: None, selected_field: DetailField::Cake, from_planner: false };
                    } else {
                        self.enter_task_list(None, None);
                    }
                }
            }
            KeyCode::Esc => {
                if *from_create {
                    let tt = create_task_type.clone().unwrap_or(TaskType::Task);
                    let asgn = create_assignee.clone().unwrap_or_default();
                    self.screen = Screen::Create { task_type: tt, assignee: asgn, cake_id: None, field: CreateField::Cake };
                } else {
                    let id = task_id.clone();
                    let task = self.repo.as_ref().and_then(|r| r.get_task(&id).ok());
                    if let Some(t) = task {
                        self.screen = Screen::Detail { task: t, message: None, selected_field: DetailField::Cake, from_planner: false };
                    } else {
                        self.enter_task_list(None, None);
                    }
                }
            }
            _ => {}
        }
    }

    fn try_quit(&mut self) {
        let has_changes = self.repo.as_ref()
            .and_then(|r| r.has_local_changes().ok())
            .unwrap_or(true); // treat error or no-repo as "maybe has changes" — safer to prompt
        if has_changes {
            self.screen = Screen::PushPrompt;
        } else {
            self.should_quit = true;
        }
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
        Some("Another gitcake session is already open for this repo".to_string())
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
    dirs::config_dir().map(|d| d.join("gitcake").join(format!("{key}.lock")))
}

/// Checks whether a process with the given PID is currently running.
/// On Linux this uses /proc; on other platforms the check is not available
/// so we return false, which causes any existing lock to appear stale.
fn process_running(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    { Path::new(&format!("/proc/{pid}")).exists() }
    #[cfg(not(target_os = "linux"))]
    { let _ = pid; false }
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
        format!("Sync failed: {err} · ^R to retry")
    }
}

/// Interprets a pull result into an ephemeral message and a persistent error.
/// Auth and network failures are classified so the user gets actionable text.
fn classify_pull_result(result: Result<String, gitcake_core::error::AppError>) -> (Option<String>, Option<String>) {
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

/// Sorts tasks into display order: in-progress → open → done, then high → normal → low within each group.
fn sort_for_display(mut tasks: Vec<Task>) -> Vec<Task> {
    tasks.sort_by_key(|t| {
        let status_rank = match &t.status {
            TaskStatus::InProgress => 0u8,
            TaskStatus::Open => 1,
            TaskStatus::Done => 2,
        };
        let priority_rank = match &t.priority {
            Priority::Urgent => 0u8,
            Priority::High   => 1,
            Priority::Normal => 2,
        };
        (status_rank, priority_rank)
    });
    tasks
}

/// Returns the indices of tasks matching the filter. Empty filter returns all indices.
fn apply_filter_indices(tasks: &[Task], filter: &str) -> Vec<usize> {
    if filter.is_empty() {
        return (0..tasks.len()).collect();
    }
    let f = filter.to_lowercase();
    tasks.iter().enumerate()
        .filter(|(_, t)| task_matches(t, &f))
        .map(|(i, _)| i)
        .collect()
}

fn next_task_type(t: TaskType) -> TaskType {
    match t {
        TaskType::Task => TaskType::Bug,
        TaskType::Bug => TaskType::Incident,
        TaskType::Incident => TaskType::Task,
    }
}

fn next_priority(p: Priority) -> Priority {
    match p {
        Priority::Urgent => Priority::Normal,
        Priority::High   => Priority::Urgent,
        Priority::Normal => Priority::High,
    }
}

pub(crate) fn task_matches(t: &Task, f: &str) -> bool {
    use gitcake_core::models::task::Priority;
    t.id.starts_with(f)
        || t.title.to_lowercase().contains(f)
        || t.owner.as_deref().map(|o| o.to_lowercase().contains(f)).unwrap_or(false)
        || type_str(&t.task_type).contains(f)
        || status_str(&t.status).contains(f)
        || (f == "ai_flagged" && t.ai_flagged)
        || (f == "urgent" && t.priority == Priority::Urgent)
        || (f == "high"   && t.priority == Priority::High)
}

fn type_str(t: &gitcake_core::models::task::TaskType) -> &'static str {
    use gitcake_core::models::task::TaskType;
    match t {
        TaskType::Task => "task",
        TaskType::Bug => "bug",
        TaskType::Incident => "incident",
    }
}

fn status_str(s: &TaskStatus) -> &'static str {
    match s {
        TaskStatus::Open => "open",
        TaskStatus::InProgress => "in-progress",
        TaskStatus::Done => "done",
    }
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

    let tmp_path = env::temp_dir().join(format!("gitcake-desc-{}.md", std::process::id()));
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
