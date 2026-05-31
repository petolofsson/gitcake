use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph},
    Frame,
};

use gitcake_core::models::task::{Priority, Task, TaskStatus, TaskType};

use crate::app::{App, CreateField, DetailField, Screen, TaskContext};

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    if area.width < 60 || area.height < 20 {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(1), Constraint::Fill(1)])
            .split(area);
        f.render_widget(
            Paragraph::new(format!(
                "Terminal too small ({}×{}) — resize to at least 60×20",
                area.width, area.height
            ))
            .alignment(Alignment::Center)
            .style(Style::new().add_modifier(Modifier::DIM)),
            rows[1],
        );
        return;
    }
    match &app.screen {
        Screen::Setup { input, error, can_cancel } => draw_setup(f, input, error.as_deref(), *can_cancel),
        Screen::InitRepo { path, name, error } => draw_init_repo(f, path, name, error.as_deref()),
        Screen::TaskList { tasks, selected, message } => {
            draw_task_list(f, task_list_params(app, tasks, *selected, message.as_deref()))
        }
        Screen::Detail { task, message, selected_field } => {
            draw_detail(f, app.context, task, message.as_deref(), *selected_field);
        }
        Screen::Create { task_type, assignee, field } => {
            draw_create(f, app.context, task_type, assignee, field)
        }
        Screen::AssignTask { users, selected, filter, .. } => draw_assign_task(f, users, *selected, filter),
        Screen::PickAssignee { users, selected, filter, .. } => draw_pick_assignee(f, users, *selected, filter),

        Screen::DeleteConfirm { task_title, .. } => draw_delete_confirm(f, task_title, app.context),
        Screen::SyncConfirm => draw_sync_confirm(f, app.context),
        Screen::PushPrompt => draw_push_prompt(f),
        Screen::TeamView { tasks, selected } => draw_team_view(f, app, tasks, *selected),
    }
}

// ── setup ─────────────────────────────────────────────────────────────────────

fn task_list_params<'a>(app: &'a App, tasks: &'a [Task], selected: usize, message: Option<&'a str>) -> TaskListParams<'a> {
    let (repo_name, username) = app.repo.as_ref()
        .map(|r| (r.info.name.as_str(), r.info.username.as_str()))
        .unwrap_or(("", ""));
    TaskListParams {
        context: app.context, tasks, selected, message,
        pull_error: app.pull_error.as_deref(), lock_warning: app.lock_warning.as_deref(),
        filter: &app.filter, filter_active: app.filter_active, repo_name, username,
    }
}

fn draw_setup(f: &mut Frame, input: &str, error: Option<&str>, can_cancel: bool) {
    let area = f.area();
    let block = padded_block("gitcake");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new("Enter the path to your gitcake repo:")
            .alignment(Alignment::Center),
        rows[1],
    );

    let input_line = Line::from(vec![
        Span::raw("> "),
        Span::styled(input, Style::new().add_modifier(Modifier::BOLD)),
        Span::styled("_", Style::new().add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(
        Paragraph::new(input_line).alignment(Alignment::Center),
        rows[2],
    );

    f.render_widget(
        Paragraph::new("Local path to a cloned git repo — e.g. ~/tasks or /home/you/my-tasks")
            .alignment(Alignment::Center)
            .style(Style::new().add_modifier(Modifier::DIM)),
        rows[3],
    );

    draw_error_line(f, rows[4], error);
    let bar = if can_cancel {
        action_bar(&[("Enter", "connect"), ("Esc", "cancel")])
    } else {
        action_bar(&[("Enter", "connect"), ("^Q", "quit")])
    };
    f.render_widget(Paragraph::new(bar), rows[6]);
}

// ── init repo ─────────────────────────────────────────────────────────────────

fn draw_init_repo(f: &mut Frame, path: &str, name: &str, error: Option<&str>) {
    let area = f.area();
    let block = padded_block("gitcake — initialize repo");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new("Empty git repo detected. Initialize as a gitcake repo?")
            .alignment(Alignment::Center),
        rows[1],
    );
    f.render_widget(
        Paragraph::new(path)
            .alignment(Alignment::Center)
            .style(Style::new().add_modifier(Modifier::DIM)),
        rows[2],
    );

    let name_line = Line::from(vec![
        Span::raw("Repo name: "),
        Span::styled(name, Style::new().add_modifier(Modifier::BOLD)),
        Span::styled("_", Style::new().add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(Paragraph::new(name_line).alignment(Alignment::Center), rows[3]);

    draw_error_line(f, rows[4], error);
    f.render_widget(Paragraph::new(action_bar(&[("Enter", "initialize"), ("Esc", "back")])), rows[6]);
}

// ── pull prompt ───────────────────────────────────────────────────────────────

// ── task list ─────────────────────────────────────────────────────────────────

struct TaskListParams<'a> {
    context: TaskContext,
    tasks: &'a [Task],
    selected: usize,
    message: Option<&'a str>,
    pull_error: Option<&'a str>,
    lock_warning: Option<&'a str>,
    filter: &'a str,
    filter_active: bool,
    repo_name: &'a str,
    username: &'a str,
}

fn draw_task_list(f: &mut Frame, p: TaskListParams<'_>) {
    let TaskListParams { context, tasks, selected, message, pull_error, lock_warning, filter, filter_active, repo_name, username } = p;
    let area = f.area();
    let inner_width = { let b = padded_block(""); b.inner(area).width as usize };
    let block = task_list_block(context, repo_name, username, message, filter, filter_active, pull_error, lock_warning);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(1), Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
        .split(inner);
    let (items, index_map, safe_selected) = build_task_items(tasks, filter, selected, inner_width);
    if items.is_empty() {
        let msg = if filter.is_empty() { "No tasks yet. Press c to create one." } else { "No tasks match the filter." };
        f.render_widget(Paragraph::new(msg).alignment(Alignment::Center).style(Style::new().add_modifier(Modifier::DIM)), rows[0]);
    } else {
        let mut state = ListState::default();
        state.select(index_map.iter().position(|&i| i == safe_selected));
        f.render_stateful_widget(List::new(items), rows[0], &mut state);
    }
    f.render_widget(filter_line_widget(filter, filter_active), rows[1]);
    f.render_widget(Paragraph::new(nav_bar(context)), rows[3]);
    f.render_widget(Paragraph::new(ctrl_bar()), rows[4]);
}

fn task_list_block<'a>(
    context: TaskContext, repo_name: &'a str, username: &'a str,
    message: Option<&'a str>, filter: &'a str, filter_active: bool,
    pull_error: Option<&'a str>, lock_warning: Option<&'a str>,
) -> Block<'a> {
    let app_title = match context {
        TaskContext::Personal => format!(" gitcake · {repo_name} · {username} "),
        TaskContext::Backlog  => format!(" gitcake · {repo_name} · {username} · BACKLOG "),
    };
    let mut block = Block::default()
        .title(app_title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .padding(Padding::new(1, 1, 1, 1));
    if let Some(msg) = message {
        block = block.title_top(Line::from(format!(" {msg} ")).right_aligned());
    }
    if !filter.is_empty() && !filter_active {
        block = block.title_top(Line::from(vec![
            Span::styled(format!(" /{filter} "), Style::new().add_modifier(Modifier::DIM)),
            Span::styled(" Esc: clear ", Style::new().add_modifier(Modifier::DIM)),
        ]).left_aligned());
    }
    if let Some(err) = pull_error {
        block = block.title_bottom(Line::from(vec![
            Span::styled(format!(" ⚠ {err} "), Style::new().fg(Color::Yellow)),
            Span::styled(" Esc: dismiss ", Style::new().add_modifier(Modifier::DIM)),
        ]).left_aligned());
    }
    if let Some(warn) = lock_warning {
        block = block.title_bottom(Line::from(Span::styled(format!(" ⚠ {warn} "), Style::new().fg(Color::Yellow))).right_aligned());
    }
    block
}

fn build_task_items(tasks: &[Task], filter: &str, selected: usize, inner_width: usize) -> (Vec<ListItem<'static>>, Vec<usize>, usize) {
    use crate::app::task_matches;
    let f_lower = if filter.is_empty() { String::new() } else { filter.to_lowercase() };
    let visible = |t: &Task| f_lower.is_empty() || task_matches(t, &f_lower);
    let (mut ip, mut op, mut dn) = (0usize, 0usize, 0usize);
    for t in tasks.iter().filter(|t| visible(t)) {
        match t.status { TaskStatus::InProgress => ip += 1, TaskStatus::Open => op += 1, TaskStatus::Done => dn += 1 }
    }
    let safe_selected = selected.min((ip + op + dn).saturating_sub(1));
    let mut items: Vec<ListItem<'static>> = Vec::new();
    let mut index_map: Vec<usize> = Vec::new();
    let mut prev_status: Option<TaskStatus> = None;
    for (vis_idx, task) in tasks.iter().filter(|t| visible(t)).enumerate() {
        if prev_status.as_ref() != Some(&task.status) {
            if prev_status.is_some() { items.push(ListItem::new(Line::from(""))); index_map.push(usize::MAX); }
            let (sym, label, count, color) = match task.status {
                TaskStatus::InProgress => ("●", "IN PROGRESS", ip, Some(Color::Yellow)),
                TaskStatus::Open       => ("○", "OPEN",        op, None),
                TaskStatus::Done       => ("✓", "DONE",        dn, None),
            };
            let hdr = color.map_or_else(|| Style::new().add_modifier(Modifier::BOLD | Modifier::DIM), |c| Style::new().add_modifier(Modifier::BOLD).fg(c));
            items.push(ListItem::new(Line::from(Span::styled(format!(" {sym} {label} ({count})"), hdr))));
            index_map.push(usize::MAX);
            prev_status = Some(task.status.clone());
        }
        let (row, idx) = task_row(task, vis_idx, safe_selected, inner_width);
        items.push(row);
        index_map.push(idx);
    }
    if prev_status.is_some() { items.push(ListItem::new(Line::from(""))); index_map.push(usize::MAX); }
    (items, index_map, safe_selected)
}

fn filter_line_widget<'a>(filter: &'a str, filter_active: bool) -> Paragraph<'a> {
    if filter_active {
        Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled("/ ", Style::new().add_modifier(Modifier::DIM)),
            Span::styled(filter, Style::new().add_modifier(Modifier::BOLD)),
            Span::styled("_", Style::new().add_modifier(Modifier::SLOW_BLINK)),
        ]))
    } else if !filter.is_empty() {
        Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("/{filter}"), Style::new().add_modifier(Modifier::DIM)),
            Span::styled("  Esc: clear", Style::new().add_modifier(Modifier::DIM)),
        ]))
    } else {
        Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled("Use '/' to filter", Style::new().add_modifier(Modifier::DIM)),
        ]))
    }
}

// ── task row helper ───────────────────────────────────────────────────────────

fn task_row(task: &Task, vis_idx: usize, safe_selected: usize, inner_width: usize) -> (ListItem<'static>, usize) {
    let is_sel = vis_idx == safe_selected;
    let cursor = if is_sel { "▶ " } else { "  " };
    let tl = format!("{:<8}", type_label(&task.task_type));
    let id_str = format!("{}  ", task.id);
    let type_str = format!("{tl}  ");
    let title_str = truncate_title(&task.title, inner_width.saturating_sub(24));
    let base = match task.status {
        TaskStatus::Done       => Style::new().add_modifier(Modifier::DIM),
        TaskStatus::InProgress => Style::new().add_modifier(Modifier::BOLD).fg(Color::Yellow),
        TaskStatus::Open       => Style::new(),
    };
    let cursor_style = if is_sel {
        Style::new().add_modifier(Modifier::BOLD).fg(Color::Cyan)
    } else {
        Style::new().add_modifier(Modifier::DIM)
    };
    let row_style = if is_sel { base.add_modifier(Modifier::REVERSED) } else { base };
    let (flag_char, flag_style) = if task.status != TaskStatus::Done {
        if task.blocked {
            ("! ", Style::new().fg(Color::Red).add_modifier(Modifier::BOLD))
        } else {
            match task.priority {
                Priority::High   => ("^ ", Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Priority::Low    => ("v ", Style::new().add_modifier(Modifier::DIM)),
                Priority::Normal => ("  ", Style::new()),
            }
        }
    } else {
        ("  ", Style::new())
    };
    let line = Line::from(vec![
        Span::styled(cursor.to_string(), cursor_style),
        Span::styled(flag_char, if is_sel { flag_style.add_modifier(Modifier::REVERSED) } else { flag_style }),
        Span::styled(id_str, row_style.add_modifier(Modifier::DIM)),
        Span::styled(type_str, row_style.add_modifier(Modifier::DIM)),
        Span::styled(title_str, row_style),
    ]);
    (ListItem::new(line), vis_idx)
}

// ── detail ────────────────────────────────────────────────────────────────────

fn draw_detail(f: &mut Frame, _context: TaskContext, task: &Task, _message: Option<&str>, selected: DetailField) {
    let area = f.area();
    let block = Block::default()
        .title(Line::from(vec![Span::raw(" Task "), Span::styled(task.id.clone(), Style::new().add_modifier(Modifier::BOLD)), Span::raw(" ")]))
        .borders(Borders::ALL).border_type(BorderType::Rounded).padding(Padding::new(1, 1, 1, 1));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Fill(1),   Constraint::Length(1), Constraint::Length(1),
    ]).split(inner);
    render_detail_fields(f, &rows, task, selected);
    render_detail_desc(f, rows[10], task);
    f.render_widget(Paragraph::new(detail_nav_bar()), rows[11]);
    f.render_widget(Paragraph::new(ctrl_bar()), rows[12]);
}

fn render_detail_fields(f: &mut Frame, rows: &[Rect], task: &Task, selected: DetailField) {
    let dim  = Style::new().add_modifier(Modifier::DIM);
    let bold = Style::new().add_modifier(Modifier::BOLD);
    let cyan = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let render_field = |f: &mut Frame, row: Rect, field: DetailField, label: &str, value: &str, value_style: Style| {
        let is_sel = selected == field;
        let cursor_style = if is_sel { cyan } else { dim };
        f.render_widget(Paragraph::new(Line::from(vec![
            Span::styled(if is_sel { "▶ " } else { "  " }, cursor_style),
            Span::styled(format!("{:<10}", label), bold),
            Span::styled(value.to_string(), value_style),
        ])), row);
    };
    let status_style = match task.status {
        TaskStatus::InProgress => Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        TaskStatus::Done => dim, TaskStatus::Open => bold,
    };
    let priority_style = match task.priority {
        Priority::High => Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        Priority::Normal => Style::new(), Priority::Low => dim,
    };
    let blocked_style = if task.blocked { Style::new().fg(Color::Red).add_modifier(Modifier::BOLD) } else { dim };
    render_field(f, rows[1], DetailField::Type,     "TYPE:",     type_label(&task.task_type), bold);
    render_field(f, rows[2], DetailField::Status,   "STATUS:",   status_label(&task.status),  status_style);
    render_field(f, rows[3], DetailField::Priority, "PRIORITY:", priority_label(&task.priority), priority_style);
    render_field(f, rows[4], DetailField::Blocked,  "BLOCKED:",  if task.blocked { "yes" } else { "no" }, blocked_style);
    let file_path = format!("{}s/{}.md", type_label(&task.task_type), task.id);
    let ro = |label: &str, value: String| Paragraph::new(Line::from(vec![
        Span::raw("  "), Span::styled(format!("{:<10}", label), bold), Span::styled(value, dim),
    ]));
    f.render_widget(ro("FILE:",    file_path), rows[5]);
    f.render_widget(ro("CREATED:", task.created.format("%Y-%m-%d %H:%M").to_string()), rows[6]);
    f.render_widget(ro("TITLE:",   task.title.clone()), rows[8]);
}

fn render_detail_desc(f: &mut Frame, area: Rect, task: &Task) {
    let dim = Style::new().add_modifier(Modifier::DIM);
    let mut text = task.description.as_deref().unwrap_or_default().to_string();
    if task.order.is_some() || task.parent_id.is_some() {
        if !text.is_empty() { text.push('\n'); }
        if let Some(o) = task.order      { text.push_str(&format!("order: {o}  ")); }
        if let Some(p) = &task.parent_id { text.push_str(&format!("parent: {p}")); }
    }
    let lines: Vec<Line> = text.lines()
        .map(|l| Line::from(vec![Span::raw("  "), Span::styled(l.to_string(), dim)]))
        .collect();
    f.render_widget(Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false }), area);
}

// ── create ────────────────────────────────────────────────────────────────────

fn draw_create(f: &mut Frame, context: TaskContext, task_type: &TaskType, assignee: &str, field: &CreateField) {
    let area = f.area();
    let block = padded_block("New task");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Fill(1),
        Constraint::Length(1), Constraint::Length(1),
    ]).split(inner);
    draw_create_type_field(f, rows[0], rows[1], task_type, *field == CreateField::Type);
    draw_create_assign_field(f, rows[3], rows[4], assignee, *field == CreateField::Assignee);
    let confirm_active = *field == CreateField::Confirm;
    let confirm_label = if confirm_active {
        "↵ CREATE TASK  Enter: open editor — write '# Title' on the first line"
    } else {
        "↵ CREATE TASK  (Tab to reach, Enter to open editor)"
    };
    draw_field_label(f, rows[6], confirm_label, confirm_active);
    f.render_widget(Paragraph::new(nav_bar(context)), rows[8]);
    f.render_widget(Paragraph::new(ctrl_bar()), rows[9]);
}

fn draw_create_type_field(f: &mut Frame, label_row: Rect, val_row: Rect, task_type: &TaskType, active: bool) {
    draw_field_label(f, label_row, "TYPE:", active);
    let style = if active { Style::new().add_modifier(Modifier::BOLD).fg(Color::Cyan) } else { Style::new().add_modifier(Modifier::DIM) };
    f.render_widget(Paragraph::new(format!("[ {} ]  Space to cycle", type_label(task_type))).style(style), val_row);
}

fn draw_create_assign_field(f: &mut Frame, label_row: Rect, val_row: Rect, assignee: &str, active: bool) {
    draw_field_label(f, label_row, "ASSIGN TO:", active);
    let text = if active { format!("{assignee}  ← Enter to pick") } else { assignee.to_string() };
    let style = if active { Style::new().fg(Color::Cyan) } else { Style::new().add_modifier(Modifier::DIM) };
    f.render_widget(Paragraph::new(text).style(style), val_row);
}

// ── team view ─────────────────────────────────────────────────────────────────

fn draw_team_view(f: &mut Frame, app: &App, tasks: &[(String, Task)], selected: usize) {
    let area = f.area();
    let block = padded_block(" gitcake · TEAM ");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Fill(1), Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
    ]).split(inner);
    let inner_width = inner.width as usize;
    let (items, index_map) = build_team_items(tasks, selected, &app.filter, inner_width);
    if items.is_empty() {
        let msg = if app.filter.is_empty() { "No active tasks from any team member." } else { "No tasks match the filter." };
        f.render_widget(Paragraph::new(msg).alignment(Alignment::Center).style(Style::new().add_modifier(Modifier::DIM)), rows[0]);
    } else {
        let mut state = ListState::default();
        state.select(index_map.iter().position(|e| *e == Some(selected)));
        f.render_stateful_widget(List::new(items), rows[0], &mut state);
    }
    f.render_widget(filter_line_widget(&app.filter, app.filter_active), rows[1]);
    f.render_widget(Paragraph::new(bar_line(&[("WS", "navigate"), ("T", "back"), ("/", "filter")])), rows[2]);
    f.render_widget(Paragraph::new(ctrl_bar()), rows[3]);
}

fn build_team_items(tasks: &[(String, Task)], selected: usize, filter: &str, inner_width: usize) -> (Vec<ListItem<'static>>, Vec<Option<usize>>) {
    use crate::app::task_matches;
    let f = if filter.is_empty() { String::new() } else { filter.to_lowercase() };
    let is_vis = |t: &Task| f.is_empty() || task_matches(t, &f);
    let mut items: Vec<ListItem<'static>> = Vec::new();
    let mut index_map: Vec<Option<usize>> = Vec::new();
    let mut seen_users: Vec<&str> = Vec::new();
    for (user, task) in tasks {
        if is_vis(task) && !seen_users.contains(&user.as_str()) { seen_users.push(user.as_str()); }
    }
    let mut vis_idx = 0usize;
    for user in seen_users {
        let user_tasks: Vec<_> = tasks.iter().filter(|(u, t)| u.as_str() == user && is_vis(t)).collect();
        if user_tasks.is_empty() { continue; }
        items.push(ListItem::new(Line::from(Span::styled(format!(" {user}"), Style::new().add_modifier(Modifier::BOLD | Modifier::DIM)))));
        index_map.push(None);
        for (_, task) in user_tasks {
            let is_sel = vis_idx == selected;
            let base = if task.status == TaskStatus::InProgress { Style::new().add_modifier(Modifier::BOLD).fg(Color::Yellow) } else { Style::new() };
            let cursor_style = if is_sel { Style::new().add_modifier(Modifier::BOLD).fg(Color::Cyan) } else { Style::new().add_modifier(Modifier::DIM) };
            let row_style = if is_sel { base.add_modifier(Modifier::REVERSED) } else { base };
            items.push(ListItem::new(Line::from(vec![
                Span::styled(if is_sel { "▶ " } else { "  " }, cursor_style),
                Span::styled(format!("{}  ", task.id), row_style.add_modifier(Modifier::DIM)),
                Span::styled(format!("{:<8}  ", type_label(&task.task_type)), row_style.add_modifier(Modifier::DIM)),
                Span::styled(truncate_title(&task.title, inner_width.saturating_sub(22)), row_style),
            ])));
            index_map.push(Some(vis_idx));
            vis_idx += 1;
        }
        items.push(ListItem::new(Line::from("")));
        index_map.push(None);
    }
    (items, index_map)
}

// ── sync confirm ──────────────────────────────────────────────────────────────

fn draw_sync_confirm(f: &mut Frame, context: TaskContext) {
    let inner = render_popup(f, "Push", 54, 7);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let what = match context {
        TaskContext::Personal => "This will commit and push your tasks.",
        TaskContext::Backlog => "This will commit and push the shared backlog.",
    };
    f.render_widget(
        Paragraph::new(what).style(Style::new().add_modifier(Modifier::DIM)),
        rows[0],
    );
    f.render_widget(
        Paragraph::new("Continue? [y/N]"),
        rows[1],
    );
    f.render_widget(
        Paragraph::new("y: push  Enter/n/q: cancel")
            .style(Style::new().add_modifier(Modifier::DIM)),
        rows[3],
    );
}

// ── push prompt ───────────────────────────────────────────────────────────────

fn draw_push_prompt(f: &mut Frame) {
    let area = f.area();
    let block = padded_block("gitcake");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new("You are closing gitcake. Do you want to push all task states?")
            .alignment(Alignment::Center),
        rows[1],
    );
    f.render_widget(
        Paragraph::new("y: push and quit  Enter/n: quit without pushing  Esc: go back")
            .alignment(Alignment::Center)
            .style(Style::new().add_modifier(Modifier::DIM)),
        rows[2],
    );
}

// ── error ─────────────────────────────────────────────────────────────────────

// ── assign task ───────────────────────────────────────────────────────────────

fn draw_pick_assignee(f: &mut Frame, users: &[String], selected: usize, filter: &str) {
    draw_user_picker(f, " Pick assignee ", users, selected, filter);
}

fn draw_assign_task(f: &mut Frame, users: &[String], selected: usize, filter: &str) {
    draw_user_picker(f, " Assign to ", users, selected, filter);
}

fn draw_user_picker(f: &mut Frame, title: &str, users: &[String], selected: usize, filter: &str) {
    let filtered: Vec<&String> = if filter.is_empty() {
        users.iter().collect()
    } else {
        users.iter().filter(|u| u.to_lowercase().starts_with(&filter.to_lowercase())).collect()
    };

    let height = (filtered.len() as u16 + 7).min(f.area().height);
    let inner = render_popup(f, title, 50, height);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // filter input
            Constraint::Length(1), // blank
            Constraint::Fill(1),   // list
            Constraint::Length(1), // hint
        ])
        .split(inner);

    // Filter input line
    let filter_line = Line::from(vec![
        Span::styled("/ ", Style::new().add_modifier(Modifier::DIM)),
        Span::styled(filter, Style::new().add_modifier(Modifier::BOLD)),
        Span::styled("_", Style::new().add_modifier(Modifier::SLOW_BLINK)),
    ]);
    f.render_widget(Paragraph::new(filter_line), rows[0]);

    if filtered.is_empty() {
        f.render_widget(Paragraph::new("No matching users.").style(Style::new().add_modifier(Modifier::DIM)), rows[2]);
    } else {
        let items = user_picker_items(&filtered, selected);
        let mut state = ListState::default();
        state.select(Some(selected));
        f.render_stateful_widget(List::new(items), rows[2], &mut state);
    }

    f.render_widget(
        Paragraph::new("↑↓: navigate  Type to filter  Enter: select  Esc: cancel")
            .style(Style::new().add_modifier(Modifier::DIM)),
        rows[3],
    );
}

fn user_picker_items(filtered: &[&String], selected: usize) -> Vec<ListItem<'static>> {
    filtered.iter().enumerate().map(|(i, u)| {
        let (prefix, style) = if i == selected {
            ("▶ ", Style::new().add_modifier(Modifier::BOLD).fg(Color::Cyan))
        } else {
            ("  ", Style::new())
        };
        ListItem::new(Line::from(vec![Span::styled(prefix, style), Span::styled(u.to_string(), style)]))
    }).collect()
}

// ── delete confirm ────────────────────────────────────────────────────────────

fn draw_delete_confirm(f: &mut Frame, task_title: &str, context: TaskContext) {
    let (title, subtext) = match context {
        TaskContext::Personal => ("Move to Backlog", "Assignee will be cleared."),
        TaskContext::Backlog => ("Delete", "This action cannot be undone."),
    };

    let inner = render_popup(f, title, 60, 8);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // question (may wrap)
            Constraint::Length(1), // spacer
            Constraint::Length(1), // subtext
            Constraint::Length(1), // spacer
            Constraint::Length(1), // [y/N]
        ])
        .split(inner);

    let question_line = match context {
        TaskContext::Personal => Line::from(vec![
            Span::raw("Move \""),
            Span::styled(task_title, Style::new().add_modifier(Modifier::BOLD)),
            Span::raw("\" to the shared backlog?"),
        ]),
        TaskContext::Backlog => Line::from(vec![
            Span::raw("Are you sure you want to delete \""),
            Span::styled(task_title, Style::new().add_modifier(Modifier::BOLD)),
            Span::raw("\"?"),
        ]),
    };
    f.render_widget(
        Paragraph::new(question_line).wrap(ratatui::widgets::Wrap { trim: true }),
        rows[0],
    );
    f.render_widget(
        Paragraph::new(subtext).style(Style::new().add_modifier(Modifier::DIM)),
        rows[2],
    );
    f.render_widget(
        Paragraph::new("[y/N]"),
        rows[4],
    );
}

// ── command bars ─────────────────────────────────────────────────────────────

fn nav_bar<'a>(context: TaskContext) -> Line<'a> {
    let f_label = if context == TaskContext::Backlog { "claim" } else { "cycle" };
    let b_label = if context == TaskContext::Backlog { "personal" } else { "backlog" };
    let items: &[(&str, &str)] = &[
        ("WASD", "navigate"),
        ("C", "create"),
        ("E", "edit"),
        ("F", f_label),
        ("B", b_label),
        ("T", "team"),
    ];
    bar_line(items)
}

fn detail_nav_bar<'a>() -> Line<'a> {
    bar_line(&[("WS", "navigate"), ("F", "cycle"), ("E", "edit"), ("⇧R", "pull")])
}

fn ctrl_bar<'a>() -> Line<'a> {
    bar_line(&[("^A", "assign"), ("⇧R", "pull"), ("^R", "push"), ("^D", "delete"), ("^Q", "quit")])
}

fn bar_line<'a>(items: &[(&'a str, &'a str)]) -> Line<'a> {
    let mut spans = vec![Span::raw(" ")];
    for (key, label) in items {
        spans.push(Span::styled(format!(" {key} "), Style::new().bg(Color::White).fg(Color::Black)));
        spans.push(Span::styled(format!(" {label}  "), Style::new().fg(Color::DarkGray)));
    }
    Line::from(spans)
}

/// Generic chip-style bar — same visual style as nav_bar / ctrl_bar.
fn action_bar<'a>(items: &[(&'a str, &'a str)]) -> Line<'a> {
    bar_line(items)
}

// ── shared helpers ────────────────────────────────────────────────────────────

// ── type / status labels ──────────────────────────────────────────────────────

fn type_label(t: &TaskType) -> &'static str {
    match t {
        TaskType::Task => "task",
        TaskType::Bug => "bug",
        TaskType::Incident => "incident",
    }
}

fn status_label(s: &TaskStatus) -> &'static str {
    match s {
        TaskStatus::Open => "open",
        TaskStatus::InProgress => "in-progress",
        TaskStatus::Done => "done",
    }
}

fn priority_label(p: &Priority) -> &'static str {
    match p {
        Priority::High => "high",
        Priority::Normal => "normal",
        Priority::Low => "low",
    }
}

// ── popup helper ──────────────────────────────────────────────────────────────

/// Renders a centred popup block and returns the inner rect.
fn render_popup(f: &mut Frame, title: &str, percent_x: u16, height: u16) -> Rect {
    let area = f.area();
    let popup = centered_rect(percent_x, height, area);
    f.render_widget(Clear, popup);
    let block = padded_block(title);
    let inner = block.inner(popup);
    f.render_widget(block, popup);
    inner
}

// ── error line ────────────────────────────────────────────────────────────────

fn draw_error_line(f: &mut Frame, area: Rect, err: Option<&str>) {
    if let Some(e) = err {
        f.render_widget(
            Paragraph::new(format!("✗ {e}"))
                .alignment(Alignment::Center)
                .style(Style::new().fg(Color::Red)),
            area,
        );
    }
}

fn outer_block(title: &str) -> Block<'static> {
    Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
}

fn padded_block(title: &str) -> Block<'static> {
    outer_block(title).padding(Padding::new(1, 1, 1, 1))
}

fn draw_field_label(f: &mut Frame, area: Rect, label: &str, active: bool) {
    let style = if active {
        Style::new().add_modifier(Modifier::BOLD).fg(Color::Cyan)
    } else {
        Style::new().add_modifier(Modifier::DIM)
    };
    f.render_widget(Paragraph::new(label).style(style), area);
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let width = area.width * percent_x / 100;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect { x, y, width, height }
}

/// Truncates `s` to at most `max_width` display cells, appending `…` if
/// truncation occurred. Handles multi-cell characters (emoji, CJK) correctly.
fn truncate_title(s: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if UnicodeWidthStr::width(s) <= max_width {
        return s.to_string();
    }
    let budget = max_width.saturating_sub(1); // reserve one cell for '…'
    let mut used = 0usize;
    let mut end = 0usize;
    for ch in s.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(1);
        if used + w > budget {
            break;
        }
        used += w;
        end += ch.len_utf8();
    }
    format!("{}…", &s[..end])
}
