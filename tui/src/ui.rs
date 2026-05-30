use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph},
    Frame,
};

use gitcake_core::models::task::{Task, TaskStatus, TaskType};

use crate::app::{App, CreateField, Screen, TaskContext};

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
            let (repo_name, username) = app.repo.as_ref()
                .map(|r| (r.info.name.as_str(), r.info.username.as_str()))
                .unwrap_or(("", ""));
            draw_task_list(f, TaskListParams {
                context: app.context,
                tasks,
                selected: *selected,
                message: message.as_deref(),
                pull_error: app.pull_error.as_deref(),
                lock_warning: app.lock_warning.as_deref(),
                filter: &app.filter,
                filter_active: app.filter_active,
                repo_name,
                username,
            })
        }
        Screen::Detail { task, message } => {
            draw_detail(f, app.context, task, message.as_deref());
        }
        Screen::Create { task_type, assignee, field } => {
            draw_create(f, task_type, assignee, field)
        }
        Screen::AssignTask { users, selected, filter, .. } => draw_assign_task(f, users, *selected, filter),
        Screen::PickAssignee { users, selected, filter, .. } => draw_pick_assignee(f, users, *selected, filter),

        Screen::DeleteConfirm { task_title, .. } => draw_delete_confirm(f, task_title, app.context),
        Screen::SyncConfirm => draw_sync_confirm(f, app.context),
        Screen::PushPrompt => draw_push_prompt(f),
        Screen::TeamView { tasks, selected } => draw_team_view(f, tasks, *selected),
    }
}

// ── setup ─────────────────────────────────────────────────────────────────────

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
    let inner_width = {
        let b = padded_block("");
        b.inner(area).width as usize
    };

    let app_title = match context {
        TaskContext::Personal => format!(" gitcake · {repo_name} · {username} "),
        TaskContext::Backlog => format!(" gitcake · {repo_name} · {username} · BACKLOG "),
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
        block = block.title_top(
            Line::from(vec![
                Span::styled(format!(" /{filter} "), Style::new().add_modifier(Modifier::DIM)),
                Span::styled(" Esc: clear ", Style::new().add_modifier(Modifier::DIM)),
            ])
            .left_aligned(),
        );
    }
    if let Some(err) = pull_error {
        block = block.title_bottom(
            Line::from(vec![
                Span::styled(format!(" ⚠ {err} "), Style::new().fg(Color::Yellow)),
                Span::styled(" Esc: dismiss ", Style::new().add_modifier(Modifier::DIM)),
            ])
            .left_aligned(),
        );
    }
    if let Some(warn) = lock_warning {
        block = block.title_bottom(
            Line::from(Span::styled(format!(" ⚠ {warn} "), Style::new().fg(Color::Yellow)))
                .right_aligned(),
        );
    }
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(1), // nav shortcuts
            Constraint::Length(1), // filter bar (always present, empty when not filtering)
            Constraint::Length(1), // ctrl shortcuts
        ])
        .split(inner);

    // Apply filter — selected is an index into the visible list
    use crate::app::apply_filter;
    let visible: Vec<&Task> = apply_filter(tasks, filter);
    let safe_selected = selected.min(visible.len().saturating_sub(1));

    let in_progress: Vec<(usize, &Task)> = visible.iter().enumerate()
        .filter(|(_, t)| t.status == TaskStatus::InProgress)
        .map(|(i, t)| (i, *t))
        .collect();
    let open: Vec<(usize, &Task)> = visible.iter().enumerate()
        .filter(|(_, t)| t.status == TaskStatus::Open)
        .map(|(i, t)| (i, *t))
        .collect();
    let done: Vec<(usize, &Task)> = visible.iter().enumerate()
        .filter(|(_, t)| t.status == TaskStatus::Done)
        .map(|(i, t)| (i, *t))
        .collect();

    let mut items: Vec<ListItem> = Vec::new();
    let mut index_map: Vec<usize> = Vec::new(); // maps list-item pos → visible index

    let add_section = |items: &mut Vec<ListItem>,
                       index_map: &mut Vec<usize>,
                       header: &str,
                       group: &[(usize, &Task)],
                       current_selected: usize,
                       section_color: Option<Color>| {
        if group.is_empty() { return; }
        let header_style = match section_color {
            Some(c) => Style::new().add_modifier(Modifier::BOLD).fg(c),
            None => Style::new().add_modifier(Modifier::BOLD | Modifier::DIM),
        };
        items.push(ListItem::new(Line::from(vec![Span::styled(format!(" {header}"), header_style)])));
        index_map.push(usize::MAX);

        for (vis_idx, task) in group {
            let is_sel = *vis_idx == current_selected;
            let cursor = if is_sel { "▶ " } else { "  " };
            let tl = format!("{:<8}", type_label(&task.task_type));
            let id_str = format!("{}  ", task.id);
            let type_str = format!("{tl}  ");
            let title_budget = inner_width.saturating_sub(22);
            let title_str = truncate_title(&task.title, title_budget);
            let base = if task.status == TaskStatus::Done {
                Style::new().add_modifier(Modifier::DIM)
            } else if task.status == TaskStatus::InProgress {
                Style::new().add_modifier(Modifier::BOLD).fg(Color::Yellow)
            } else {
                Style::new()
            };
            let cursor_style = if is_sel {
                Style::new().add_modifier(Modifier::BOLD).fg(Color::Cyan)
            } else {
                Style::new().add_modifier(Modifier::DIM)
            };
            let row_style = if is_sel { base.add_modifier(Modifier::REVERSED) } else { base };
            let line = Line::from(vec![
                Span::styled(cursor.to_string(), cursor_style),
                Span::styled(id_str, row_style.add_modifier(Modifier::DIM)),
                Span::styled(type_str, row_style.add_modifier(Modifier::DIM)),
                Span::styled(title_str, row_style),
            ]);
            items.push(ListItem::new(line));
            index_map.push(*vis_idx);
        }
        items.push(ListItem::new(Line::from("")));
        index_map.push(usize::MAX);
    };

    let hdr = |sym: &str, label: &str, n: usize| format!("{sym} {label} ({n})");
    add_section(&mut items, &mut index_map, &hdr("●", "IN PROGRESS", in_progress.len()), &in_progress, safe_selected, Some(Color::Yellow));
    add_section(&mut items, &mut index_map, &hdr("○", "OPEN", open.len()), &open, safe_selected, None);
    add_section(&mut items, &mut index_map, &hdr("✓", "DONE", done.len()), &done, safe_selected, None);

    if items.is_empty() {
        let empty_msg = if filter.is_empty() {
            "No tasks yet. Press c to create one."
        } else {
            "No tasks match the filter."
        };
        f.render_widget(
            Paragraph::new(empty_msg)
                .alignment(Alignment::Center)
                .style(Style::new().add_modifier(Modifier::DIM)),
            rows[0],
        );
    } else {
        let list_pos = index_map.iter().position(|&i| i == safe_selected);
        let mut state = ListState::default();
        state.select(list_pos);
        f.render_stateful_widget(List::new(items), rows[0], &mut state);
    }

    // Filter line — always present above the nav bar
    // 2 leading spaces align '/' with the 'W' in WASD
    let filter_widget = if filter_active {
        Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled("/ ", Style::new().add_modifier(Modifier::DIM)),
            Span::styled(filter.to_string(), Style::new().add_modifier(Modifier::BOLD)),
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
    };
    let nb = if context == TaskContext::Backlog { backlog_nav_bar() } else { nav_bar() };
    f.render_widget(Paragraph::new(nb), rows[1]);
    f.render_widget(filter_widget, rows[2]);
    f.render_widget(Paragraph::new(ctrl_bar(context)), rows[3]);
}

// ── detail ────────────────────────────────────────────────────────────────────

fn draw_detail(f: &mut Frame, _context: TaskContext, task: &Task, _message: Option<&str>) {
    let area = f.area();
    let block = Block::default()
        .title(Line::from(vec![
            Span::raw(" Task "),
            Span::styled(task.id.clone(), Style::new().add_modifier(Modifier::BOLD)),
            Span::raw(" "),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .padding(Padding::new(1, 1, 1, 1));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // blank
            Constraint::Length(1), // task: {status}
            Constraint::Length(1), // file: {path}
            Constraint::Length(1), // created: {timestamp}
            Constraint::Length(1), // blank
            Constraint::Length(1), // title: {title}
            Constraint::Length(1), // blank
            Constraint::Fill(1),   // description
            Constraint::Length(1), // nav bar
            Constraint::Length(1), // ctrl bar
        ])
        .split(inner);

    let file_path = {
        let type_folder = match task.task_type {
            TaskType::Task => "tasks",
            TaskType::Bug => "bugs",
            TaskType::Incident => "incidents",
        };
        format!("{type_folder}/{}.md", task.id)
    };

    let dim = Style::new().add_modifier(Modifier::DIM);
    let status_style = match task.status {
        TaskStatus::InProgress => Style::new().add_modifier(Modifier::BOLD).fg(Color::Yellow),
        TaskStatus::Done => Style::new().add_modifier(Modifier::DIM),
        TaskStatus::Open => Style::new().add_modifier(Modifier::BOLD),
    };
    let bold = Style::new().add_modifier(Modifier::BOLD);
    // rows[0] blank
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{}:", type_label(&task.task_type).to_uppercase()), bold),
            Span::raw("  "),
            Span::styled(status_label(&task.status), status_style),
        ])),
        rows[1],
    );
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled("FILE:", bold),
            Span::styled(format!("     {file_path}"), dim),
        ])),
        rows[2],
    );
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled("CREATED:", bold),
            Span::styled(format!("  {}", task.created.format("%Y-%m-%d %H:%M")), dim),
        ])),
        rows[3],
    );
    // rows[4] blank
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled("TITLE:", bold),
            Span::raw("    "),
            Span::styled(task.title.clone(), bold),
        ])),
        rows[5],
    );
    // rows[6] blank
    let desc = task.description.as_deref().unwrap_or_default();
    let desc_lines: Vec<Line> = desc.lines()
        .map(|l| Line::from(vec![Span::raw("  "), Span::raw(l.to_string())]))
        .collect();
    f.render_widget(
        Paragraph::new(desc_lines).wrap(ratatui::widgets::Wrap { trim: false }),
        rows[7],
    );

    f.render_widget(Paragraph::new(nav_bar()), rows[8]);
    f.render_widget(Paragraph::new(ctrl_bar(TaskContext::Personal)), rows[9]);
}

// ── create ────────────────────────────────────────────────────────────────────

fn draw_create(
    f: &mut Frame,
    task_type: &TaskType,
    assignee: &str,
    field: &CreateField,
) {
    let area = f.area();
    let block = padded_block("New task");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // TYPE: label
            Constraint::Length(1), // type selector
            Constraint::Length(1), // blank
            Constraint::Length(1), // ASSIGN TO: label
            Constraint::Length(1), // assignee display
            Constraint::Length(1), // blank
            Constraint::Length(1), // CREATE TASK row
            Constraint::Fill(1),   // breathing room
            Constraint::Length(1), // nav bar
            Constraint::Length(1), // ctrl bar
        ])
        .split(inner);

    draw_field_label(f, rows[0], "TYPE:", *field == CreateField::Type);
    let type_style = if *field == CreateField::Type {
        Style::new().add_modifier(Modifier::BOLD).fg(Color::Cyan)
    } else {
        Style::new().add_modifier(Modifier::DIM)
    };
    f.render_widget(
        Paragraph::new(format!("[ {} ]  Space to cycle", type_label(task_type))).style(type_style),
        rows[1],
    );

    let assign_active = *field == CreateField::Assignee;
    draw_field_label(f, rows[3], "ASSIGN TO:", assign_active);
    let assign_text = if assign_active {
        format!("{assignee}  ← Enter to pick")
    } else {
        assignee.to_string()
    };
    let assign_style = if assign_active {
        Style::new().fg(Color::Cyan)
    } else {
        Style::new().add_modifier(Modifier::DIM)
    };
    f.render_widget(Paragraph::new(assign_text).style(assign_style), rows[4]);

    let confirm_active = *field == CreateField::Confirm;
    let confirm_label = if confirm_active {
        "↵ CREATE TASK  Enter: open editor — write '# Title' on the first line"
    } else {
        "↵ CREATE TASK  (Tab to reach, Enter to open editor)"
    };
    draw_field_label(f, rows[6], confirm_label, confirm_active);

    f.render_widget(Paragraph::new(nav_bar()), rows[8]);
    f.render_widget(Paragraph::new(ctrl_bar(TaskContext::Personal)), rows[9]);
}

// ── team view ─────────────────────────────────────────────────────────────────

fn draw_team_view(f: &mut Frame, tasks: &[(String, Task)], selected: usize) {
    let area = f.area();
    let block = padded_block(" gitcake · TEAM ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(1)])
        .split(inner);

    let inner_width = inner.width as usize;

    // index_map[list_row] = Some(tasks index) for task rows, None for headers/blanks
    let mut items: Vec<ListItem> = Vec::new();
    let mut index_map: Vec<Option<usize>> = Vec::new();

    let mut seen_users: Vec<&str> = Vec::new();
    for (user, _) in tasks {
        if !seen_users.contains(&user.as_str()) {
            seen_users.push(user.as_str());
        }
    }

    for user in seen_users {
        items.push(ListItem::new(Line::from(Span::styled(
            format!(" {user}"),
            Style::new().add_modifier(Modifier::BOLD | Modifier::DIM),
        ))));
        index_map.push(None);

        for (task_idx, (_, task)) in tasks.iter().enumerate().filter(|(_, (u, _))| u.as_str() == user) {
            let is_sel = task_idx == selected;
            let cursor = if is_sel { "▶ " } else { "  " };
            let base = if task.status == TaskStatus::InProgress {
                Style::new().add_modifier(Modifier::BOLD).fg(Color::Yellow)
            } else {
                Style::new()
            };
            let cursor_style = if is_sel {
                Style::new().add_modifier(Modifier::BOLD).fg(Color::Cyan)
            } else {
                Style::new().add_modifier(Modifier::DIM)
            };
            let row_style = if is_sel { base.add_modifier(Modifier::REVERSED) } else { base };
            let title_str = truncate_title(&task.title, inner_width.saturating_sub(22));

            items.push(ListItem::new(Line::from(vec![
                Span::styled(cursor.to_string(), cursor_style),
                Span::styled(format!("{}  ", task.id), row_style.add_modifier(Modifier::DIM)),
                Span::styled(format!("{:<8}  ", type_label(&task.task_type)), row_style.add_modifier(Modifier::DIM)),
                Span::styled(title_str, row_style),
            ])));
            index_map.push(Some(task_idx));
        }
        items.push(ListItem::new(Line::from("")));
        index_map.push(None);
    }

    if items.is_empty() {
        f.render_widget(
            Paragraph::new("No active tasks from any team member.")
                .alignment(Alignment::Center)
                .style(Style::new().add_modifier(Modifier::DIM)),
            rows[0],
        );
    } else {
        let list_pos = index_map.iter().position(|e| *e == Some(selected));
        let mut state = ListState::default();
        state.select(list_pos);
        f.render_stateful_widget(List::new(items), rows[0], &mut state);
    }

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(" WS ", Style::new().bg(Color::White).fg(Color::Black)),
            Span::styled(" navigate  ", Style::new().fg(Color::DarkGray)),
            Span::styled(" A ", Style::new().bg(Color::White).fg(Color::Black)),
            Span::styled(" back  ", Style::new().fg(Color::DarkGray)),
        ])),
        rows[1],
    );
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
        f.render_widget(
            Paragraph::new("No matching users.")
                .style(Style::new().add_modifier(Modifier::DIM)),
            rows[2],
        );
    } else {
        let items: Vec<ListItem> = filtered
            .iter()
            .enumerate()
            .map(|(i, u)| {
                let (prefix, style) = if i == selected {
                    ("▶ ", Style::new().add_modifier(Modifier::BOLD).fg(Color::Cyan))
                } else {
                    ("  ", Style::new())
                };
                ListItem::new(Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(u.to_string(), style),
                ]))
            })
            .collect();

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

fn nav_bar<'a>() -> Line<'a> {
    let items = [
        ("WASD", "navigate"),
        ("C", "create"),
        ("E", "edit"),
        ("F", "cycle"),
        ("B", "backlog"),
        ("T", "team"),
    ];
    let mut spans = vec![Span::raw(" ")];
    for (key, label) in &items {
        spans.push(Span::styled(
            format!(" {key} "),
            Style::new().bg(Color::White).fg(Color::Black),
        ));
        spans.push(Span::styled(
            format!(" {label}  "),
            Style::new().fg(Color::DarkGray),
        ));
    }
    Line::from(spans)
}

fn backlog_nav_bar<'a>() -> Line<'a> {
    let items = [
        ("WASD", "navigate"),
        ("C", "create"),
        ("E", "edit"),
        ("F", "claim"),
        ("B", "personal"),
        ("T", "team"),
    ];
    let mut spans = vec![Span::raw(" ")];
    for (key, label) in &items {
        spans.push(Span::styled(format!(" {key} "), Style::new().bg(Color::White).fg(Color::Black)));
        spans.push(Span::styled(format!(" {label}  "), Style::new().fg(Color::DarkGray)));
    }
    Line::from(spans)
}

fn ctrl_bar<'a>(_context: TaskContext) -> Line<'a> {
    let mut items: Vec<(&str, &str)> = Vec::new();
    items.push(("^A", "assign"));
    items.extend_from_slice(&[("⇧R", "pull"), ("^R", "push"), ("^D", "delete"), ("^Q", "quit")]);
    let mut spans = vec![Span::raw(" ")];
    for (key, label) in &items {
        spans.push(Span::styled(
            format!(" {key} "),
            Style::new().bg(Color::White).fg(Color::Black),
        ));
        spans.push(Span::styled(
            format!(" {label}  "),
            Style::new().fg(Color::DarkGray),
        ));
    }
    Line::from(spans)
}

/// Generic chip-style bar — same visual style as nav_bar / ctrl_bar.
fn action_bar<'a>(items: &[(&'a str, &'a str)]) -> Line<'a> {
    let mut spans = vec![Span::raw(" ")];
    for (key, label) in items {
        spans.push(Span::styled(
            format!(" {key} "),
            Style::new().bg(Color::White).fg(Color::Black),
        ));
        spans.push(Span::styled(
            format!(" {label}  "),
            Style::new().fg(Color::DarkGray),
        ));
    }
    Line::from(spans)
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
