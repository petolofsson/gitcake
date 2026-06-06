use std::{fs, path::{Path, PathBuf}};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use gitcake_core::{
    models::{
        cake::{Cake, NewCake},
        task::{NewTask, Priority, Task, TaskPatch, TaskStatus, TaskType},
    },
    repo::TaskRepo,
};

use tui_input::Input;

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

#[derive(PartialEq, Clone, Copy)]
pub enum ViewMode {
    Table,
    Tree,
}

// ── screens ───────────────────────────────────────────────────────────────────

pub enum Screen {
    Setup {
        input: String,
        error: Option<String>,
        /// True when reached via Ctrl+O from task list; Esc returns instead of quitting.
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
        title:       Input,
        description: String,
        focus:       CreateFocus,
        task_type:   TaskType,
        priority:    Priority,
        users:       Vec<String>,
        user_filter: String,
        user_sel:    usize,
        cakes:       Vec<Cake>,
        cake_filter: String,
        cake_sel:    usize,
    },
    EditTask {
        task_id:      String,
        title:        Input,
        description:  String,
        context:      TaskContext,
        from_detail:  bool,
        from_planner: bool,
    },
    CreateCake {
        title: String,
    },
    PickCake {
        task_id: String,
        cakes: Vec<Cake>,
        selected: usize,
        filter: String,
    },
    AssignTask {
        task_id: String,
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
pub enum CreateFocus {
    Title,
    Description,
    Type,
    Priority,
    Assignee,
    Cake,
}

impl CreateFocus {
    pub fn next(self) -> Self {
        match self {
            Self::Title       => Self::Description,
            Self::Description => Self::Type,
            Self::Type        => Self::Priority,
            Self::Priority    => Self::Assignee,
            Self::Assignee    => Self::Cake,
            Self::Cake        => Self::Title,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Self::Title       => Self::Cake,
            Self::Description => Self::Title,
            Self::Type        => Self::Description,
            Self::Priority    => Self::Type,
            Self::Assignee    => Self::Priority,
            Self::Cake        => Self::Assignee,
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
    Assign,
}

impl DetailField {
    pub fn next(self) -> Self {
        match self {
            Self::Type      => Self::Status,
            Self::Status    => Self::Priority,
            Self::Priority  => Self::AiFlagged,
            Self::AiFlagged => Self::Cake,
            Self::Cake      => Self::Assign,
            Self::Assign    => Self::Type,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Self::Type      => Self::Assign,
            Self::Status    => Self::Type,
            Self::Priority  => Self::Status,
            Self::AiFlagged => Self::Priority,
            Self::Cake      => Self::AiFlagged,
            Self::Assign    => Self::Cake,
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
    pub exit_message: Option<String>,
    pub pull_error: Option<String>,
    pub lock_warning: Option<String>,
    pub lock_path: Option<PathBuf>,
    /// Persists across screen transitions — cleared only by Esc.
    pub filter: String,
    pub filter_active: bool,
    pub hide_done: bool,
    pub needs_clear: bool,
    pub view_mode: ViewMode,
    /// Cached cake list for detail view and pickers. Refreshed when entering planner.
    pub cached_cakes: Vec<Cake>,
}

impl App {
    pub fn new(config: Config) -> Self {
        if config.repo_path.is_none() {
            return Self {
                screen: Screen::Setup { input: String::new(), error: None, can_cancel: false },
                repo: None, config, context: TaskContext::Personal,
                should_quit: false, exit_message: None,
                pull_error: None, lock_warning: None, lock_path: None,
                filter: String::new(), filter_active: false, hide_done: false, needs_clear: false, view_mode: ViewMode::Table, cached_cakes: Vec::new(),
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
                repo: None, config, context: TaskContext::Personal,
                should_quit: false, exit_message: None,
                pull_error: None, lock_warning: None, lock_path: None,
                filter: String::new(), filter_active: false, hide_done: false, needs_clear: false, view_mode: ViewMode::Table, cached_cakes: Vec::new(),
            };
        }

        let (pull_msg, pull_error) = classify_pull_result(repo.as_ref().unwrap().pull());
        let repo_path = repo.as_ref().unwrap().info.path.clone();
        let (lock_path, lock_warning) = acquire_lock(&repo_path);
        let (raw_tasks, task_warnings) = repo.as_ref().unwrap().list_tasks().unwrap_or_default();
        let startup_msg = merge_messages(pull_msg, warn_summary(&task_warnings));
        let tasks = sort_for_display(raw_tasks);
        let screen = Screen::TaskList { tasks, selected: 0, message: startup_msg };
        Self {
            screen, repo, config, context: TaskContext::Personal,
            should_quit: false, exit_message: None,
            pull_error, lock_warning, lock_path,
            filter: String::new(), filter_active: false, hide_done: false, needs_clear: false, view_mode: ViewMode::Table, cached_cakes: Vec::new(),
        }
    }

    pub fn handle_event(&mut self, event: Event) {
        let Event::Key(key) = event else { return };
        if matches!(&self.screen, Screen::TaskList { .. } | Screen::PlannerView { .. }) {
            match key.code {
                KeyCode::F(1) => { self.view_mode = ViewMode::Table; return; }
                KeyCode::F(2) => { self.view_mode = ViewMode::Tree;  return; }
                _ => {}
            }
        }
        if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
            let input_screen = matches!(&self.screen,
                Screen::Setup { .. } | Screen::InitRepo { .. } | Screen::Create { .. }
                | Screen::EditTask { .. } | Screen::CreateCake { .. }
                | Screen::AssignTask { .. } | Screen::DeleteConfirm { .. }
                | Screen::SyncConfirm | Screen::PushPrompt | Screen::PickCake { .. }
            );
            if !input_screen { self.open_create_screen(); return; }
        }
        match &self.screen {
            Screen::Setup { .. }         => self.handle_setup(key),
            Screen::InitRepo { .. }      => self.handle_init_repo(key),
            Screen::TaskList { .. }      => self.handle_task_list(key),
            Screen::Detail { .. }        => self.handle_detail(key),
            Screen::Create { .. }        => self.handle_create(key),
            Screen::EditTask { .. }      => self.handle_edit_task(key),
            Screen::AssignTask { .. }    => self.handle_assign_task(key),
            Screen::DeleteConfirm { .. } => self.handle_delete_confirm(key),
            Screen::SyncConfirm          => self.handle_sync_confirm(key),
            Screen::PushPrompt           => self.handle_push_prompt(key),
            Screen::PlannerView { .. }   => self.handle_planner_view(key),
            Screen::CreateCake { .. }    => self.handle_create_cake(key),
            Screen::PickCake { .. }      => self.handle_pick_cake(key),
        }
    }

    fn open_create_screen(&mut self) {
        let info = self.repo.as_ref().map(|r| {
            let username = r.info.username.clone();
            let mut users = r.list_users().ok().unwrap_or_default();
            if !users.contains(&username) { users.insert(0, username.clone()); }
            let user_sel = users.iter().position(|u| u == &username).unwrap_or(0);
            (users, user_sel)
        });
        if let Some((users, user_sel)) = info {
            let cakes = self.cached_cakes.clone();
            self.screen = Screen::Create {
                title: Input::default(), description: String::new(), focus: CreateFocus::Title,
                task_type: TaskType::Task, priority: Priority::Normal,
                users, user_filter: String::new(), user_sel,
                cakes, cake_filter: String::new(), cake_sel: 0,
            };
        }
    }

    // ── setup ─────────────────────────────────────────────────────────────────

    fn handle_setup(&mut self, key: KeyEvent) {
        let Screen::Setup { input, error: _, can_cancel } = &mut self.screen else { return };
        let can_cancel = *can_cancel;
        if is_ctrl_q(&key) { self.should_quit = true; return; }
        match key.code {
            KeyCode::Char(c)   => input.push(c),
            KeyCode::Backspace => { input.pop(); }
            KeyCode::Enter => {
                let path = input.trim().to_string();
                self.evaluate_path(path, can_cancel);
            }
            KeyCode::Esc => {
                if can_cancel { self.enter_task_list(None, None); } else { self.should_quit = true; }
            }
            _ => {}
        }
    }

    // ── init repo ─────────────────────────────────────────────────────────────

    fn handle_init_repo(&mut self, key: KeyEvent) {
        let Screen::InitRepo { path, name, error: _ } = &mut self.screen else { return };
        match key.code {
            KeyCode::Char(c)   => name.push(c),
            KeyCode::Backspace => { name.pop(); }
            KeyCode::Enter => {
                let path = path.clone();
                let name = name.trim().to_string();
                if name.is_empty() { return; }
                match TaskRepo::init(&path, &name) {
                    Ok(repo) => {
                        self.config.repo_path = Some(path);
                        self.config.save();
                        self.repo = Some(repo);
                        self.enter_task_list(Some("Repo initialized.".into()), None);
                    }
                    Err(e) => {
                        self.screen = Screen::InitRepo { path, name, error: Some(e.to_string()) };
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
            self.handle_list_pull(); return;
        }
        let Screen::TaskList { tasks, selected, message } = &mut self.screen else { return };
        if key.code == KeyCode::Char('/') && key.modifiers == KeyModifiers::NONE {
            self.filter_active = true; self.filter.clear(); *selected = 0; return;
        }
        let no_mod = key.modifiers == KeyModifiers::NONE;
        if key.code == KeyCode::Char('H') && !key.modifiers.contains(KeyModifiers::CONTROL) {
            self.hide_done = !self.hide_done; *selected = 0; return;
        }
        let filter = self.filter.clone();
        let hide_done = self.hide_done;
        let visible: Vec<usize> = if self.view_mode == ViewMode::Tree {
            tree_visible_indices(tasks, &self.cached_cakes, &filter, hide_done)
        } else {
            apply_filter_indices(tasks, &filter)
                .into_iter()
                .filter(|&i| !hide_done || tasks[i].status != TaskStatus::Done)
                .collect()
        };
        let vc = visible.len();
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
                if let Some(t) = sel_task {
                    self.screen = Screen::Detail { task: t, message: None, selected_field: DetailField::Type, from_planner: false };
                }
            }
            KeyCode::Char(c) if c == km.edit.chars().next().unwrap_or('e') && km.edit.len() == 1 && no_mod => {
                if let Some(t) = sel_task {
                    self.enter_edit_task(t.id, t.title, t.description.unwrap_or_default(), false, false);
                }
            }
            KeyCode::Char(c) if c == km.status_cycle.chars().next().unwrap_or('f') && km.status_cycle.len() == 1 && no_mod && self.context == TaskContext::Personal => {
                if let Some(t) = sel_task { self.cycle_status(&t.id); }
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
                if let Some(t) = sel_task {
                    self.screen = Screen::DeleteConfirm { task_id: t.id, task_title: t.title };
                }
            }
            KeyCode::Char('d') if ctrl && self.context == TaskContext::Backlog => {
                if let Some(t) = sel_task {
                    self.screen = Screen::DeleteConfirm { task_id: t.id, task_title: t.title };
                }
            }
            _ => {}
        }
    }

    fn handle_filter_input(&mut self, key: KeyEvent) {
        let Screen::TaskList { selected, .. } = &mut self.screen else { return };
        match key.code {
            KeyCode::Esc       => { if self.filter.is_empty() { self.filter_active = false; } else { self.filter.clear(); *selected = 0; } }
            KeyCode::Enter     => { self.filter_active = false; }
            KeyCode::Backspace => { self.filter.pop(); *selected = 0; }
            KeyCode::Char(c)   => { self.filter.push(c); *selected = 0; }
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
            self.do_detail_pull(); return;
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
            self.enter_edit_task(task_id, title, desc, true, from_planner); return;
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

    fn do_detail_pull(&mut self) {
        let (task_id, from_planner) = if let Screen::Detail { task, from_planner, .. } = &self.screen {
            (task.id.clone(), *from_planner)
        } else { return };
        let (pull_msg, pull_err) = match self.repo.as_ref().map(|r| r.pull()) {
            Some(r) => classify_pull_result(r),
            None    => (None, Some("No repo connected.".to_string())),
        };
        self.pull_error = pull_err;
        if from_planner { self.enter_planner_view(); } else { self.enter_task_list(pull_msg, Some(&task_id)); }
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
                self.screen = Screen::PickCake { task_id, cakes, selected: 0, filter: String::new() };
                return;
            }
            DetailField::Assign => {
                let users = self.repo.as_ref().and_then(|r| r.list_users().ok()).unwrap_or_default();
                self.screen = Screen::AssignTask { task_id, users, selected: 0, filter: String::new() };
                return;
            }
        };
        if let (Some(t), Screen::Detail { task, .. }) = (updated, &mut self.screen) {
            *task = t;
        }
    }

    // ── edit task ─────────────────────────────────────────────────────────────

    fn enter_edit_task(&mut self, task_id: String, title: String, desc: String, from_detail: bool, from_planner: bool) {
        self.screen = Screen::EditTask {
            task_id,
            title: title.as_str().into(),
            description: desc,
            context: self.context,
            from_detail,
            from_planner,
        };
    }

    fn handle_edit_task(&mut self, key: KeyEvent) {
        if is_ctrl_q(&key) { self.try_quit(); return; }
        if key.code == KeyCode::Tab || key.code == KeyCode::BackTab {
            let current = match &self.screen {
                Screen::EditTask { description, .. } => description.clone(),
                _ => return,
            };
            if let Some(new_desc) = open_editor(&current) {
                if let Screen::EditTask { description, .. } = &mut self.screen {
                    *description = new_desc;
                }
            }
            self.needs_clear = true;
            return;
        }
        if key.code == KeyCode::Esc { self.edit_cancel(); return; }
        if key.code == KeyCode::Enter && key.modifiers == KeyModifiers::NONE {
            self.edit_submit(); return;
        }
        self.edit_title_key(key);
    }

    fn edit_title_key(&mut self, key: KeyEvent) {
        use tui_input::backend::crossterm::EventHandler;
        if let Screen::EditTask { title, .. } = &mut self.screen {
            title.handle_event(&crossterm::event::Event::Key(key));
        }
    }

    fn edit_cancel(&mut self) {
        let (from_detail, from_planner, task_id) = match &self.screen {
            Screen::EditTask { from_detail, from_planner, task_id, .. } =>
                (*from_detail, *from_planner, task_id.clone()),
            _ => return,
        };
        if from_detail {
            if let Some(t) = self.repo.as_ref().and_then(|r| r.get_task(&task_id).ok()) {
                self.screen = Screen::Detail { task: t, message: None, selected_field: DetailField::Type, from_planner };
                return;
            }
        }
        if from_planner { self.enter_planner_view(); } else { self.enter_task_list(None, Some(&task_id)); }
    }

    fn edit_submit(&mut self) {
        let (task_id, title_str, desc_str, context, from_detail, from_planner) = match &self.screen {
            Screen::EditTask { task_id, title, description, context, from_detail, from_planner, .. } => {
                let ts = title.value().trim().to_string();
                if ts.is_empty() { return; }
                let ds = description.trim().to_string();
                (task_id.clone(), ts, ds, *context, *from_detail, *from_planner)
            }
            _ => return,
        };
        let n = desc_str.chars().count();
        if n > DESCRIPTION_MAX_CHARS { self.enter_task_list(Some(desc_too_long(n)), Some(&task_id)); return; }
        let desc = if desc_str.is_empty() { None } else { Some(desc_str) };
        let patch = TaskPatch { title: Some(title_str), description: Some(desc), ..Default::default() };
        let result = self.repo.as_ref().map(|r| match context {
            TaskContext::Personal => r.update_task(&task_id, patch),
            TaskContext::Backlog  => r.update_backlog_task(&task_id, patch),
        });
        let msg = match result {
            Some(Ok(_))  => Some("Task updated.".to_string()),
            Some(Err(e)) => Some(e.to_string()),
            None         => None,
        };
        if from_detail {
            if let Some(t) = self.repo.as_ref().and_then(|r| r.get_task(&task_id).ok()) {
                self.screen = Screen::Detail { task: t, message: msg, selected_field: DetailField::Type, from_planner };
                return;
            }
        }
        if from_planner { self.enter_planner_view(); } else { self.enter_task_list(msg, Some(&task_id)); }
    }

    // ── create ────────────────────────────────────────────────────────────────

    fn handle_create(&mut self, key: KeyEvent) {
        if is_ctrl_q(&key) { self.should_quit = true; return; }
        if key.code == KeyCode::Esc { self.enter_task_list(None, None); return; }
        let focus = match &self.screen {
            Screen::Create { focus, .. } => *focus,
            _ => return,
        };
        if key.code == KeyCode::Tab    { self.create_next_field(false); return; }
        if key.code == KeyCode::BackTab { self.create_next_field(true); return; }
        if key.code == KeyCode::Enter && key.modifiers == KeyModifiers::NONE
            && focus != CreateFocus::Description
        {
            self.submit_create(); return;
        }
        match focus {
            CreateFocus::Title       => self.create_title_key(key),
            CreateFocus::Description => {
                if key.code == KeyCode::Enter && key.modifiers == KeyModifiers::NONE {
                    let current = match &self.screen {
                        Screen::Create { description, .. } => description.clone(),
                        _ => return,
                    };
                    if let Some(new_desc) = open_editor(&current) {
                        if let Screen::Create { description, focus, .. } = &mut self.screen {
                            *description = new_desc;
                            *focus = CreateFocus::Type;
                        }
                    }
                    self.needs_clear = true;
                }
            }
            CreateFocus::Type        => self.create_type_key(key),
            CreateFocus::Priority    => self.create_priority_key(key),
            CreateFocus::Assignee    => self.create_assignee_key(key),
            CreateFocus::Cake        => self.create_cake_key(key),
        }
    }

    fn create_next_field(&mut self, backward: bool) {
        if let Screen::Create { focus, user_filter, cake_filter, .. } = &mut self.screen {
            user_filter.clear();
            cake_filter.clear();
            *focus = if backward { focus.prev() } else { focus.next() };
        }
    }

    fn create_title_key(&mut self, key: KeyEvent) {
        use tui_input::backend::crossterm::EventHandler;
        if let Screen::Create { title, .. } = &mut self.screen {
            title.handle_event(&crossterm::event::Event::Key(key));
        }
    }

    fn create_type_key(&mut self, key: KeyEvent) {
        if key.modifiers != KeyModifiers::NONE { return; }
        let Screen::Create { task_type, .. } = &mut self.screen else { return };
        if matches!(key.code, KeyCode::Left | KeyCode::Right) {
            *task_type = match (&task_type, key.code == KeyCode::Right) {
                (TaskType::Task,     true)  => TaskType::Bug,
                (TaskType::Task,     false) => TaskType::Incident,
                (TaskType::Bug,      true)  => TaskType::Incident,
                (TaskType::Bug,      false) => TaskType::Task,
                (TaskType::Incident, true)  => TaskType::Task,
                (TaskType::Incident, false) => TaskType::Bug,
            };
        }
    }

    fn create_priority_key(&mut self, key: KeyEvent) {
        if key.modifiers != KeyModifiers::NONE { return; }
        let Screen::Create { priority, .. } = &mut self.screen else { return };
        if matches!(key.code, KeyCode::Left | KeyCode::Right) {
            *priority = match (&priority, key.code == KeyCode::Right) {
                (Priority::Normal, true)  => Priority::High,
                (Priority::Normal, false) => Priority::Urgent,
                (Priority::High,   true)  => Priority::Urgent,
                (Priority::High,   false) => Priority::Normal,
                (Priority::Urgent, true)  => Priority::Normal,
                (Priority::Urgent, false) => Priority::High,
            };
        }
    }

    fn create_assignee_key(&mut self, key: KeyEvent) {
        let no_mod = key.modifiers == KeyModifiers::NONE;
        let Screen::Create { users, user_filter, user_sel, .. } = &mut self.screen else { return };
        let f = user_filter.to_lowercase();
        let fc = users.iter().filter(|u| f.is_empty() || u.to_lowercase().contains(&f)).count();
        match key.code {
            KeyCode::Up    if no_mod => { *user_sel = user_sel.checked_sub(1).unwrap_or(fc.saturating_sub(1)); }
            KeyCode::Down  if no_mod => { if fc > 0 { *user_sel = (*user_sel + 1) % fc; } }
            KeyCode::Backspace       => { user_filter.pop(); *user_sel = 0; }
            KeyCode::Char(c) if no_mod => { user_filter.push(c); *user_sel = 0; }
            _ => {}
        }
    }

    fn create_cake_key(&mut self, key: KeyEvent) {
        let no_mod = key.modifiers == KeyModifiers::NONE;
        let Screen::Create { cakes, cake_filter, cake_sel, .. } = &mut self.screen else { return };
        let cf = cake_filter.to_lowercase();
        let fc = 1 + cakes.iter().filter(|c| cf.is_empty() || c.title.to_lowercase().contains(&cf)).count();
        match key.code {
            KeyCode::Up    if no_mod => { *cake_sel = cake_sel.checked_sub(1).unwrap_or(fc - 1); }
            KeyCode::Down  if no_mod => { *cake_sel = (*cake_sel + 1) % fc; }
            KeyCode::Backspace       => { cake_filter.pop(); *cake_sel = 0; }
            KeyCode::Char(c) if no_mod => { cake_filter.push(c); *cake_sel = 0; }
            _ => {}
        }
    }

    fn submit_create(&mut self) {
        let extracted = match &self.screen {
            Screen::Create { title, description, task_type, priority,
                             users, user_filter, user_sel, cakes, cake_sel, cake_filter, .. } => {
                let ts = title.value().trim().to_string();
                if ts.is_empty() { return; }
                let dt = description.trim().to_string();
                let desc = if dt.is_empty() { None } else { Some(dt) };
                Some((ts, desc, task_type.clone(), priority.clone(),
                      users.clone(), user_filter.clone(), *user_sel, cakes.clone(), *cake_sel, cake_filter.clone()))
            }
            _ => None,
        };
        let Some((title_str, desc, tt, priority,
                  users, user_filter, user_sel, cakes, cake_sel, cake_filter)) = extracted else { return };
        if let Some(ref d) = desc {
            let n = d.chars().count();
            if n > DESCRIPTION_MAX_CHARS { self.enter_task_list(Some(desc_too_long(n)), None); return; }
        }
        let ctx = self.context;
        let f = user_filter.to_lowercase();
        let filt_u: Vec<&String> = users.iter().filter(|u| f.is_empty() || u.to_lowercase().contains(&f)).collect();
        let assignee = filt_u.get(user_sel).map(|u| (*u).clone());
        let cf = cake_filter.to_lowercase();
        let filt_c: Vec<&Cake> = cakes.iter().filter(|c| cf.is_empty() || c.title.to_lowercase().contains(&cf)).collect();
        let cake_id = if cake_sel == 0 { None } else { filt_c.get(cake_sel - 1).map(|c| c.id.clone()) };
        let new_task = NewTask { title: title_str, description: desc, task_type: tt, priority, cake_id, ..Default::default() };
        let create_result = match ctx {
            TaskContext::Personal => self.repo.as_ref().map(|r| r.create_task(new_task)),
            TaskContext::Backlog  => self.repo.as_ref().map(|r| r.create_backlog_task(new_task)),
        };
        let msg = self.apply_create_extras(create_result, assignee, ctx);
        self.enter_task_list(Some(msg), None);
    }

    fn apply_create_extras(
        &self,
        create_result: Option<Result<Task, gitcake_core::error::AppError>>,
        assignee: Option<String>,
        _ctx: TaskContext,
    ) -> String {
        let task = match create_result {
            Some(Ok(t))  => t,
            Some(Err(e)) => return e.to_string(),
            None         => return "No repo.".to_string(),
        };
        let id = &task.id;
        if let Some(a) = assignee {
            match self.repo.as_ref().map(|r| r.assign_task(id, Some(a))) {
                Some(Err(e)) => return format!("Task created but assign failed: {e}"),
                _ => {}
            }
        }
        "Task created.".to_string()
    }

    // ── sync confirm ──────────────────────────────────────────────────────────

    fn handle_sync_confirm(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let result = self.repo.as_ref()
                    .map(|r| match self.context {
                        TaskContext::Personal => r.push(),
                        TaskContext::Backlog  => r.push_backlog(),
                    })
                    .unwrap_or(Err(gitcake_core::error::AppError::NoRepo));
                let msg = match result {
                    Ok(_)  => "Successfully pushed.".into(),
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
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.exit_message = Some(match self.repo.as_ref().map(|r| r.push()) {
                    Some(Ok(_))  => "Successfully pushed.".into(),
                    Some(Err(e)) => classify_push_error(&e.to_string()),
                    None         => "No repo connected.".into(),
                });
                self.should_quit = true;
            }
            KeyCode::Enter | KeyCode::Char('n') | KeyCode::Char('N') => {
                self.should_quit = true;
            }
            KeyCode::Esc => {
                self.enter_task_list(None, None);
            }
            _ => {}
        }
    }

    // ── planner ───────────────────────────────────────────────────────────────

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
        if key.code == KeyCode::Char('H') && !key.modifiers.contains(KeyModifiers::CONTROL) {
            self.hide_done = !self.hide_done;
            if let Screen::PlannerView { selected, .. } = &mut self.screen { *selected = 0; }
            return;
        }
        if key.code == KeyCode::Char('d') && key.modifiers == KeyModifiers::NONE {
            let f = if self.filter.is_empty() { String::new() } else { self.filter.to_lowercase() };
            let hd = self.hide_done;
            let tree = self.view_mode == ViewMode::Tree;
            let task = if let Screen::PlannerView { cakes, tasks, selected, .. } = &self.screen {
                planner_visible_tasks(cakes, tasks, &f, hd, tree)
                    .get(*selected).map(|(_, t)| t.clone())
            } else { None };
            if let Some(t) = task {
                self.screen = Screen::Detail { task: t, message: None, selected_field: DetailField::Type, from_planner: true };
            }
            return;
        }
        if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::NONE {
            self.screen = Screen::CreateCake { title: String::new() }; return;
        }
        if key.code == KeyCode::Char('b') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.do_planner_backlog(); return;
        }
        let f = if self.filter.is_empty() { String::new() } else { self.filter.to_lowercase() };
        let hd = self.hide_done;
        let tree = self.view_mode == ViewMode::Tree;
        let visible_count = if let Screen::PlannerView { cakes, tasks, .. } = &self.screen {
            planner_visible_tasks(cakes, tasks, &f, hd, tree).len()
        } else { return };
        let Screen::PlannerView { selected, .. } = &mut self.screen else { return };
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

    fn do_planner_backlog(&mut self) {
        let f = if self.filter.is_empty() { String::new() } else { self.filter.to_lowercase() };
        let hd = self.hide_done;
        let tree = self.view_mode == ViewMode::Tree;
        let task_id = if let Screen::PlannerView { cakes, tasks, selected, .. } = &self.screen {
            planner_visible_tasks(cakes, tasks, &f, hd, tree)
                .get(*selected).map(|(_, t)| t.id.clone())
        } else { None };
        let Some(id) = task_id else { return };
        if let Some(repo) = &self.repo {
            match repo.move_task_to_backlog(&id) {
                Ok(()) => {}
                Err(_) => {} // may not be a personal task; refresh shows current state
            }
        }
        self.enter_planner_view();
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
                let _ = msg; // not surfaced in planner flash row currently
                self.enter_planner_view();
            }
            KeyCode::Backspace => { title.pop(); }
            KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE => { title.push(c); }
            _ => {}
        }
    }

    fn handle_pick_cake(&mut self, key: KeyEvent) {
        let Screen::PickCake { task_id, cakes, selected, filter } = &mut self.screen else { return };
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
                let id = task_id.clone();
                let patch = TaskPatch { cake_id: Some(chosen), ..Default::default() };
                if let Some(t) = self.repo.as_ref().and_then(|r| r.update_task(&id, patch).ok()) {
                    self.screen = Screen::Detail { task: t, message: None, selected_field: DetailField::Cake, from_planner: false };
                } else {
                    self.enter_task_list(None, None);
                }
            }
            KeyCode::Esc => {
                let id = task_id.clone();
                let task = self.repo.as_ref().and_then(|r| r.get_task(&id).ok());
                if let Some(t) = task {
                    self.screen = Screen::Detail { task: t, message: None, selected_field: DetailField::Cake, from_planner: false };
                } else {
                    self.enter_task_list(None, None);
                }
            }
            _ => {}
        }
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    fn evaluate_path(&mut self, path: String, can_cancel: bool) {
        let path = expand_tilde(&path);
        let p = Path::new(&path);
        if !p.join(".git").exists() {
            self.screen = Screen::Setup { input: path, error: Some("Not a git repository.".into()), can_cancel };
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
            TaskContext::Backlog  => {
                let (all, w) = r.list_backlog_tasks().unwrap_or_default();
                let active = all.into_iter().filter(|t| t.status != TaskStatus::Done).collect();
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
                    sorted.iter().filter(|t| filter_matches(t, &f)).position(|t| t.id == id)
                }
            })
            .unwrap_or(0);
        self.cached_cakes = self.repo.as_ref().and_then(|r| r.list_cakes().ok()).unwrap_or_default();
        self.screen = Screen::TaskList { tasks: sorted, selected, message: msg };
    }

    fn cycle_status(&mut self, task_id: &str) {
        let Some(repo) = &self.repo else { return };
        let ctx = self.context;
        let Ok(task) = repo.get_task(task_id) else { return };
        let result = match (ctx, &task.status) {
            (TaskContext::Personal, TaskStatus::Open)       => repo.set_task_in_progress(task_id),
            (TaskContext::Personal, TaskStatus::InProgress) => repo.mark_task_done(task_id),
            (TaskContext::Personal, TaskStatus::Done)       => repo.set_task_in_progress(task_id),
            (TaskContext::Backlog,  TaskStatus::Open)       => repo.set_backlog_task_in_progress(task_id),
            (TaskContext::Backlog,  TaskStatus::InProgress) => repo.mark_backlog_task_done(task_id),
            (TaskContext::Backlog,  TaskStatus::Done)       => repo.set_backlog_task_in_progress(task_id),
        };
        let msg = match result {
            Ok(t) => match t.status {
                TaskStatus::InProgress => Some("Marked in-progress.".into()),
                TaskStatus::Done       => Some("Marked done.".into()),
                TaskStatus::Open       => Some("Marked open.".into()),
            },
            Err(e) => Some(e.to_string()),
        };
        if let Screen::TaskList { .. } = &self.screen {
            self.enter_task_list(msg, Some(task_id));
        }
    }

    fn handle_assign_task(&mut self, key: KeyEvent) {
        let Screen::AssignTask { task_id, users, selected, filter } = &mut self.screen else { return };
        let filtered_len = users.iter().filter(|u| u.to_lowercase().starts_with(&filter.to_lowercase())).count();
        match key.code {
            KeyCode::Up => { if *selected > 0 { *selected -= 1; } }
            KeyCode::Down => { if filtered_len > 0 && *selected < filtered_len - 1 { *selected += 1; } }
            KeyCode::Backspace => { filter.pop(); *selected = 0; }
            KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE => { filter.push(c); *selected = 0; }
            KeyCode::Enter => {
                let f = filter.to_lowercase();
                let owner = users.iter().filter(|u| u.to_lowercase().starts_with(&f)).nth(*selected).cloned();
                let id = task_id.clone();
                let result = self.repo.as_ref().map(|r| r.assign_task(&id, owner.clone()));
                let msg = match result {
                    Some(Ok(_))  => Some("Assigned.".into()),
                    Some(Err(e)) => Some(e.to_string()),
                    None         => None,
                };
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
                    TaskContext::Personal => (self.repo.as_ref().map(|r| r.move_task_to_backlog(&id)), "Moved to backlog."),
                    TaskContext::Backlog  => (self.repo.as_ref().map(|r| r.delete_backlog_task(&id)), "Deleted."),
                };
                let msg = match result {
                    Some(Ok(())) => Some(msg_ok.into()),
                    Some(Err(e)) => Some(e.to_string()),
                    None         => None,
                };
                self.enter_task_list(msg, None);
            }
            KeyCode::Enter | KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.enter_task_list(None, Some(&id));
            }
            _ => {}
        }
    }

    fn try_quit(&mut self) {
        let has_changes = self.repo.as_ref()
            .and_then(|r| r.has_local_changes().ok())
            .unwrap_or(true);
        if has_changes { self.screen = Screen::PushPrompt; } else { self.should_quit = true; }
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

fn acquire_lock(repo_path: &str) -> (Option<PathBuf>, Option<String>) {
    let Some(path) = lock_file_path(repo_path) else { return (None, None) };
    if let Some(parent) = path.parent() { let _ = fs::create_dir_all(parent); }
    let warn = if path.exists() { stale_lock_warning(&path) } else { None };
    let _ = fs::write(&path, std::process::id().to_string());
    (Some(path), warn)
}

fn stale_lock_warning(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let pid: u32 = content.trim().parse().ok()?;
    if pid == std::process::id() { return None; }
    if process_running(pid) {
        Some("Another gitcake session is already open for this repo".to_string())
    } else {
        Some("Last session ended without pushing — consider ^R to sync".to_string())
    }
}

fn lock_file_path(repo_path: &str) -> Option<PathBuf> {
    let key: String = repo_path.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect();
    dirs::config_dir().map(|d| d.join("gitcake").join(format!("{key}.lock")))
}

fn process_running(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    { Path::new(&format!("/proc/{pid}")).exists() }
    #[cfg(not(target_os = "linux"))]
    { let _ = pid; false }
}

fn warn_summary(warnings: &[String]) -> Option<String> {
    match warnings.len() {
        0 => None,
        1 => Some(format!("1 slice could not be read: {}", warnings[0])),
        n => Some(format!("{n} slices could not be read")),
    }
}

fn merge_messages(primary: Option<String>, secondary: Option<String>) -> Option<String> {
    match (primary, secondary) {
        (Some(a), Some(b)) => Some(format!("{a} · {b}")),
        (a, None) => a,
        (None, b) => b,
    }
}

fn classify_push_error(err: &str) -> String {
    let lower = err.to_lowercase();
    if lower.contains("rejected") || lower.contains("non-fast-forward")
        || lower.contains("fetch first") || lower.contains("updates were rejected")
    {
        "Push rejected: remote has new commits — run `git pull` in the repo, then ^R to retry".to_string()
    } else {
        format!("Sync failed: {err} · ^R to retry")
    }
}

fn classify_pull_result(result: Result<String, gitcake_core::error::AppError>) -> (Option<String>, Option<String>) {
    match result {
        Ok(out) if out.trim().is_empty() || out.contains("Already up to date") => (None, None),
        Ok(_) => (Some("Pulled latest changes.".to_string()), None),
        Err(e) => {
            let raw = e.to_string();
            let lower = raw.to_lowercase();
            let msg = if lower.contains("permission denied") || lower.contains("authentication failed")
                || lower.contains("could not read username") || lower.contains("access denied")
                || lower.contains("publickey") || lower.contains("invalid username or password")
            {
                "Pull failed: auth error — SSH key not loaded or credentials expired".to_string()
            } else if lower.contains("could not resolve host") || lower.contains("could not resolve hostname")
                || lower.contains("network is unreachable") || lower.contains("connection timed out")
                || lower.contains("no route to host") || lower.contains("unable to connect")
            {
                "Pull failed: no network — check VPN or connection".to_string()
            } else {
                format!("Pull failed: {raw}")
            };
            (None, Some(msg))
        }
    }
}

fn sort_for_display(mut tasks: Vec<Task>) -> Vec<Task> {
    tasks.sort_by_key(|t| {
        let status_rank = match &t.status {
            TaskStatus::InProgress => 0u8,
            TaskStatus::Open       => 1,
            TaskStatus::Done       => 2,
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

fn apply_filter_indices(tasks: &[Task], filter: &str) -> Vec<usize> {
    if filter.is_empty() { return (0..tasks.len()).collect(); }
    let f = filter.to_lowercase();
    tasks.iter().enumerate().filter(|(_, t)| filter_matches(t, &f)).map(|(i, _)| i).collect()
}

/// Returns task indices in cake-grouped tree traversal order (mirrors build_tree_items).
/// Used so Up/Down in tree mode selects tasks in the same order they appear on screen.
pub(crate) fn tree_visible_indices(tasks: &[Task], cakes: &[Cake], filter: &str, hide_done: bool) -> Vec<usize> {
    let f = if filter.is_empty() { String::new() } else { filter.to_lowercase() };
    let is_vis = |t: &Task| (f.is_empty() || filter_matches(t, &f))
                           && (!hide_done || t.status != TaskStatus::Done);

    fn process(result: &mut Vec<usize>, tasks: &[Task], indices: &[usize]) {
        let roots: Vec<usize> = indices.iter().copied()
            .filter(|&i| tasks[i].parent_id.as_ref()
                .map_or(true, |pid| !indices.iter().any(|&j| tasks[j].id == *pid)))
            .collect();
        for ri in roots {
            result.push(ri);
            for ci in indices.iter().copied()
                .filter(|&ci| tasks[ci].parent_id.as_deref() == Some(tasks[ri].id.as_str()))
            {
                result.push(ci);
            }
        }
    }

    let mut result = Vec::new();
    for cake in cakes {
        let group: Vec<usize> = tasks.iter().enumerate()
            .filter(|(_, t)| t.cake_id.as_deref() == Some(&cake.id) && is_vis(t))
            .map(|(i, _)| i).collect();
        if !group.is_empty() { process(&mut result, tasks, &group); }
    }
    let standalone: Vec<usize> = tasks.iter().enumerate()
        .filter(|(_, t)| t.cake_id.is_none() && is_vis(t))
        .map(|(i, _)| i).collect();
    if !standalone.is_empty() { process(&mut result, tasks, &standalone); }
    result
}

/// Returns planner tasks in the order the renderer shows them.
/// Table mode: cake-grouped flat order. Tree mode: cake-grouped tree traversal (roots then children).
/// Used so Up/Down and task selection agree with what's on screen.
pub(crate) fn planner_visible_tasks<'a>(
    cakes: &[Cake],
    tasks: &'a [(String, Task)],
    filter: &str,
    hide_done: bool,
    tree: bool,
) -> Vec<&'a (String, Task)> {
    let f = if filter.is_empty() { String::new() } else { filter.to_lowercase() };
    let is_vis = |t: &Task| (f.is_empty() || filter_matches(t, &f))
                           && (!hide_done || t.status != TaskStatus::Done);

    fn process_group<'a>(result: &mut Vec<&'a (String, Task)>, group: &[&'a (String, Task)], tree: bool) {
        if !tree { result.extend_from_slice(group); return; }
        let roots: Vec<&(String, Task)> = group.iter().copied()
            .filter(|(_, t)| t.parent_id.as_ref()
                .map_or(true, |pid| !group.iter().any(|(_, pt)| pt.id == *pid)))
            .collect();
        for (_, root_task) in &roots {
            let item = group.iter().copied().find(|(_, t)| t.id == root_task.id).unwrap();
            result.push(item);
            for child in group.iter().copied()
                .filter(|(_, t)| t.parent_id.as_deref() == Some(root_task.id.as_str()))
            { result.push(child); }
        }
    }

    let mut result = Vec::new();
    for cake in cakes {
        let group: Vec<&(String, Task)> = tasks.iter()
            .filter(|(_, t)| t.cake_id.as_deref() == Some(&cake.id) && is_vis(t))
            .collect();
        if !group.is_empty() { process_group(&mut result, &group, tree); }
    }
    let standalone: Vec<&(String, Task)> = tasks.iter()
        .filter(|(_, t)| t.cake_id.is_none() && is_vis(t))
        .collect();
    if !standalone.is_empty() { process_group(&mut result, &standalone, tree); }
    result
}

fn next_task_type(t: TaskType) -> TaskType {
    match t { TaskType::Task => TaskType::Bug, TaskType::Bug => TaskType::Incident, TaskType::Incident => TaskType::Task }
}

fn next_priority(p: Priority) -> Priority {
    match p { Priority::Urgent => Priority::Normal, Priority::High => Priority::Urgent, Priority::Normal => Priority::High }
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

pub(crate) fn filter_matches(t: &Task, filter: &str) -> bool {
    if filter.is_empty() { return true; }
    if let Some(neg) = filter.strip_prefix('!') { return !task_matches(t, neg); }
    if let Some(owner_q) = filter.strip_prefix('@') {
        return t.owner.as_deref().map(|o| o.to_lowercase().contains(owner_q)).unwrap_or(false);
    }
    task_matches(t, filter)
}

fn type_str(t: &gitcake_core::models::task::TaskType) -> &'static str {
    use gitcake_core::models::task::TaskType;
    match t { TaskType::Task => "task", TaskType::Bug => "bug", TaskType::Incident => "incident" }
}

fn status_str(s: &TaskStatus) -> &'static str {
    match s { TaskStatus::Open => "open", TaskStatus::InProgress => "in-progress", TaskStatus::Done => "done" }
}

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() { return home.join(rest).to_string_lossy().into_owned(); }
    } else if path == "~" {
        if let Some(home) = dirs::home_dir() { return home.to_string_lossy().into_owned(); }
    }
    path.to_string()
}

fn open_editor(initial: &str) -> Option<String> {
    let tmp_path = std::env::temp_dir().join(format!("gitcake-{}.md", std::process::id()));
    fs::write(&tmp_path, initial).ok()?;

    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);

    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());
    let _ = std::process::Command::new(&editor).arg(&tmp_path).status();

    let _ = crossterm::terminal::enable_raw_mode();
    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen);

    let content = fs::read_to_string(&tmp_path).unwrap_or_default();
    let _ = fs::remove_file(&tmp_path);
    Some(content.trim_end_matches('\n').to_string())
}

fn is_empty_repo(path: &Path) -> bool {
    let Ok(entries) = fs::read_dir(path) else { return false };
    entries.flatten().all(|entry| {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        matches!(name.as_str(), ".git" | "readme.md" | "readme" | "readme.txt" | ".gitignore" | ".gitattributes" | ".gitkeep")
    })
}
