use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph},
    Frame,
};

use git_task_core::models::task::{Task, TaskStatus, TaskType};

use crate::app::{App, CreateField, Screen, TaskContext};

pub fn draw(f: &mut Frame, app: &App) {
    match &app.screen {
        Screen::Setup { input, error } => draw_setup(f, input, error.as_deref()),
        Screen::InitRepo { path, name, error } => draw_init_repo(f, path, name, error.as_deref()),
        Screen::TaskList { tasks, selected, message } => {
            draw_task_list(f, app.context, tasks, *selected, message.as_deref())
        }
        Screen::Detail { task, message } => draw_detail(f, task, message.as_deref()),
        Screen::Create { title, task_type, description, field } => {
            draw_create(f, title, task_type, description, field)
        }
        Screen::AssignTask { users, selected, .. } => draw_assign_task(f, users, *selected),
        Screen::DeleteConfirm { task_title, .. } => draw_delete_confirm(f, task_title),
        Screen::SyncConfirm => draw_sync_confirm(f, app.context),
        Screen::PushPrompt => draw_push_prompt(f),
    }
}

// ── setup ─────────────────────────────────────────────────────────────────────

fn draw_setup(f: &mut Frame, input: &str, error: Option<&str>) {
    let area = f.area();
    let block = outer_block("git-task");
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
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new("Enter the path to your git-task repo:")
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

    if let Some(err) = error {
        f.render_widget(
            Paragraph::new(format!("✗ {err}"))
                .alignment(Alignment::Center)
                .style(Style::new().fg(Color::Red)),
            rows[4],
        );
    }

    render_help(f, area, "Enter: connect  Q: quit");
}

// ── init repo ─────────────────────────────────────────────────────────────────

fn draw_init_repo(f: &mut Frame, path: &str, name: &str, error: Option<&str>) {
    let area = f.area();
    let block = outer_block("git-task — initialize repo");
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
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new("Empty git repo detected. Initialize as a git-task repo?")
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

    if let Some(err) = error {
        f.render_widget(
            Paragraph::new(format!("✗ {err}"))
                .alignment(Alignment::Center)
                .style(Style::new().add_modifier(Modifier::DIM)),
            rows[4],
        );
    }

    render_help(f, area, "Enter: initialize  Q: back");
}

// ── pull prompt ───────────────────────────────────────────────────────────────

// ── task list ─────────────────────────────────────────────────────────────────

fn draw_task_list(f: &mut Frame, context: TaskContext, tasks: &[Task], selected: usize, message: Option<&str>) {
    let area = f.area();

    let app_title = match context {
        TaskContext::Personal => " git-task ",
        TaskContext::Backlog => " git-task · BACKLOG ",
    };
    let mut block = Block::default()
        .title(app_title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .padding(Padding::new(1, 1, 1, 1));
    if let Some(msg) = message {
        block = block.title_top(Line::from(format!(" {msg} ")).right_aligned());
    }
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(1), // regular shortcuts
            Constraint::Length(1), // ctrl shortcuts (reversed)
        ])
        .split(inner);

    // Build grouped list items
    let mut items: Vec<ListItem> = Vec::new();
    let mut index_map: Vec<usize> = Vec::new(); // maps list index → task index

    let in_progress: Vec<(usize, &Task)> = tasks
        .iter()
        .enumerate()
        .filter(|(_, t)| t.status == TaskStatus::InProgress && !t.is_completed)
        .collect();
    let open: Vec<(usize, &Task)> = tasks
        .iter()
        .enumerate()
        .filter(|(_, t)| t.status == TaskStatus::Open && !t.is_completed)
        .collect();
    let done: Vec<(usize, &Task)> = tasks
        .iter()
        .enumerate()
        .filter(|(_, t)| t.status == TaskStatus::Done || t.is_completed)
        .collect();

    let add_section = |items: &mut Vec<ListItem>,
                       index_map: &mut Vec<usize>,
                       header: &str,
                       group: &[(usize, &Task)],
                       current_selected: usize,
                       section_color: Option<Color>| {
        if group.is_empty() {
            return;
        }
        let header_style = match section_color {
            Some(c) => Style::new().add_modifier(Modifier::BOLD).fg(c),
            None => Style::new().add_modifier(Modifier::BOLD | Modifier::DIM),
        };
        items.push(ListItem::new(Line::from(vec![Span::styled(
            format!(" {header}"),
            header_style,
        )])));
        index_map.push(usize::MAX);

        for (task_idx, task) in group {
            let is_sel = *task_idx == current_selected;
            let cursor = if is_sel { "▶ " } else { "  " };

            let type_label = match task.task_type {
                TaskType::Task => "task    ",
                TaskType::Bug => "bug     ",
                TaskType::Incident => "incident",
            };

            let id_str = format!("{:>3}  ", task.id);
            let type_str = format!("{type_label}  ");
            let title_str = task.title.clone();

            let base = if task.status == TaskStatus::Done || task.is_completed {
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

            let row_style = if is_sel {
                base.add_modifier(Modifier::REVERSED)
            } else {
                base
            };

            let assignee_str = task.assignee.as_deref()
                .map(|a| format!("  → {a}"))
                .unwrap_or_default();

            let line = Line::from(vec![
                Span::styled(cursor.to_string(), cursor_style),
                Span::styled(id_str, row_style.add_modifier(Modifier::DIM)),
                Span::styled(type_str, row_style.add_modifier(Modifier::DIM)),
                Span::styled(title_str, row_style),
                Span::styled(assignee_str, row_style.add_modifier(Modifier::DIM)),
            ]);

            items.push(ListItem::new(line));
            index_map.push(*task_idx);
        }

        items.push(ListItem::new(Line::from("")));
        index_map.push(usize::MAX);
    };

    add_section(&mut items, &mut index_map, "● IN PROGRESS", &in_progress, selected, Some(Color::Yellow));
    add_section(&mut items, &mut index_map, "○ OPEN", &open, selected, None);
    add_section(&mut items, &mut index_map, "✓ DONE (local)", &done, selected, None);

    if items.is_empty() {
        f.render_widget(
            Paragraph::new("No tasks yet. Press c to create one.")
                .alignment(Alignment::Center)
                .style(Style::new().add_modifier(Modifier::DIM)),
            rows[0],
        );
    } else {
        // Find the list position of the selected task
        let list_pos = index_map.iter().position(|&i| i == selected);
        let mut state = ListState::default();
        state.select(list_pos);

        f.render_stateful_widget(List::new(items), rows[0], &mut state);
    }

    f.render_widget(
        Paragraph::new(nav_bar()).style(Style::new().bg(Color::DarkGray)),
        rows[1],
    );

    f.render_widget(
        Paragraph::new(ctrl_bar()).style(Style::new().bg(Color::DarkGray)),
        rows[2],
    );
}

// ── detail ────────────────────────────────────────────────────────────────────

fn draw_detail(f: &mut Frame, task: &Task, message: Option<&str>) {
    let area = f.area();
    let title = format!("Task {}", task.id);
    let block = outer_block(&title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let status_str = match task.status {
        TaskStatus::Open => "open",
        TaskStatus::InProgress => "in-progress",
        TaskStatus::Done => "done",
    };
    let type_str = match task.task_type {
        TaskType::Task => "task",
        TaskType::Bug => "bug",
        TaskType::Incident => "incident",
    };

    f.render_widget(
        Paragraph::new(task.title.clone()).style(Style::new().add_modifier(Modifier::BOLD)),
        rows[0],
    );
    f.render_widget(
        Paragraph::new(format!("{type_str}  ·  {status_str}"))
            .style(Style::new().add_modifier(Modifier::DIM)),
        rows[1],
    );
    f.render_widget(
        Paragraph::new(format!("created: {}", task.created.format("%Y-%m-%d %H:%M")))
            .style(Style::new().add_modifier(Modifier::DIM)),
        rows[2],
    );
    if let Some(done_at) = task.done {
        f.render_widget(
            Paragraph::new(format!("done:    {}", done_at.format("%Y-%m-%d %H:%M")))
                .style(Style::new().add_modifier(Modifier::DIM)),
            rows[3],
        );
    }

    let desc = task.description.as_deref().unwrap_or("No description.");
    f.render_widget(
        Paragraph::new(desc).wrap(ratatui::widgets::Wrap { trim: false }),
        rows[4],
    );

    let msg_prefix = message.map(|m| format!("{m}  ·  ")).unwrap_or_default();
    f.render_widget(
        Paragraph::new(format!("{msg_prefix}a/q:back  e:edit  f:cycle  Ctrl+Q:quit"))
            .style(Style::new().add_modifier(Modifier::DIM)),
        rows[5],
    );
}

// ── create ────────────────────────────────────────────────────────────────────

fn draw_create(
    f: &mut Frame,
    title: &str,
    task_type: &TaskType,
    description: &str,
    field: &CreateField,
) {
    let area = f.area();
    let block = padded_block("New task");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // editor hint (top)
            Constraint::Length(1), // blank
            Constraint::Length(1), // TITLE: label
            Constraint::Length(1), // title input
            Constraint::Length(1), // blank
            Constraint::Length(1), // TYPE: label
            Constraint::Length(1), // type selector
            Constraint::Length(1), // blank
            Constraint::Length(1), // DESCRIPTION: label
            Constraint::Fill(1),   // description preview — all remaining space
            Constraint::Length(1), // commands
        ])
        .split(inner);

    draw_editor_hint(f, rows[0], *field == CreateField::Description);

    draw_field_label(f, rows[2], "TITLE:", *field == CreateField::Title);
    draw_field_input(f, rows[3], title, *field == CreateField::Title, false);

    draw_field_label(f, rows[5], "TYPE:", *field == CreateField::Type);
    let type_str = match task_type {
        TaskType::Task => "task",
        TaskType::Bug => "bug",
        TaskType::Incident => "incident",
    };
    let type_style = if *field == CreateField::Type {
        Style::new().add_modifier(Modifier::BOLD).fg(Color::Cyan)
    } else {
        Style::new().add_modifier(Modifier::DIM)
    };
    f.render_widget(
        Paragraph::new(format!("[ {type_str} ]  Space to cycle")).style(type_style),
        rows[6],
    );

    draw_field_label(f, rows[8], "DESCRIPTION: (markdown)", *field == CreateField::Description);
    draw_description_preview(f, rows[9], description, *field == CreateField::Description);

    f.render_widget(
        Paragraph::new("Tab/Enter: next field  ·  Ctrl+S: save  ·  Esc: cancel")
            .style(Style::new().add_modifier(Modifier::DIM)),
        rows[10],
    );
}

// ── sync confirm ──────────────────────────────────────────────────────────────

fn draw_sync_confirm(f: &mut Frame, context: TaskContext) {
    let area = f.area();
    let popup = centered_rect(54, 7, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Task Sync ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .padding(Padding::new(1, 1, 1, 1));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

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
        Paragraph::new("y: sync  Enter/n/q: cancel")
            .style(Style::new().add_modifier(Modifier::DIM)),
        rows[3],
    );
}

// ── push prompt ───────────────────────────────────────────────────────────────

fn draw_push_prompt(f: &mut Frame) {
    let area = f.area();
    let block = outer_block("git-task");
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
        Paragraph::new("You are closing git-task. Do you want to push all task states?")
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

fn draw_assign_task(f: &mut Frame, users: &[String], selected: usize) {
    let area = f.area();
    let popup = centered_rect(50, (users.len() as u16 + 6).min(area.height), area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Assign to ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .padding(Padding::new(1, 1, 1, 1));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    if users.is_empty() {
        f.render_widget(
            Paragraph::new("No other users found in this repo.")
                .style(Style::new().add_modifier(Modifier::DIM)),
            inner,
        );
        return;
    }

    let items: Vec<ListItem> = users
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
                Span::styled(u.clone(), style),
            ]))
        })
        .collect();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(1)])
        .split(inner);

    f.render_widget(List::new(items), rows[0]);
    f.render_widget(
        Paragraph::new("Enter: assign  Esc/q: cancel")
            .style(Style::new().add_modifier(Modifier::DIM)),
        rows[1],
    );
}

// ── delete confirm ────────────────────────────────────────────────────────────

fn draw_delete_confirm(f: &mut Frame, task_title: &str) {
    let area = f.area();
    let popup = centered_rect(56, 7, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Delete task ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .padding(Padding::new(1, 1, 1, 1));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new(format!("Delete \"{}\"?", task_title)),
        rows[0],
    );
    f.render_widget(
        Paragraph::new("This cannot be undone.")
            .style(Style::new().add_modifier(Modifier::DIM)),
        rows[1],
    );
    f.render_widget(
        Paragraph::new("y: delete  Enter/n/Esc: cancel")
            .style(Style::new().add_modifier(Modifier::DIM)),
        rows[3],
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
    ];
    let mut spans = vec![Span::raw(" ")];
    for (key, label) in &items {
        spans.push(Span::styled(
            format!(" {key} "),
            Style::new().fg(Color::Black).bg(Color::White),
        ));
        spans.push(Span::styled(
            format!(" {label}  "),
            Style::new().fg(Color::White),
        ));
    }
    Line::from(spans)
}

fn ctrl_bar<'a>() -> Line<'a> {
    let items = [
        ("^A", "assign"),
        ("^R", "sync"),
        ("^D", "delete"),
        ("^Q", "quit"),
    ];
    let mut spans = vec![Span::raw(" ")];
    for (key, label) in &items {
        spans.push(Span::styled(
            format!(" {key} "),
            Style::new().fg(Color::Black).bg(Color::White),
        ));
        spans.push(Span::styled(
            format!(" {label}  "),
            Style::new().fg(Color::White),
        ));
    }
    Line::from(spans)
}

// ── shared helpers ────────────────────────────────────────────────────────────

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

fn draw_description_preview(f: &mut Frame, area: Rect, content: &str, active: bool) {
    let style = if active {
        Style::new()
    } else {
        Style::new().add_modifier(Modifier::DIM)
    };
    let text = if content.trim().is_empty() {
        "(no description)".to_string()
    } else {
        content.to_string()
    };
    f.render_widget(
        Paragraph::new(text)
            .style(style)
            .wrap(ratatui::widgets::Wrap { trim: false }),
        area,
    );
}

fn draw_editor_hint(f: &mut Frame, area: Rect, desc_active: bool) {
    let style = if desc_active {
        Style::new().add_modifier(Modifier::BOLD).fg(Color::Cyan)
    } else {
        Style::new().add_modifier(Modifier::DIM)
    };
    f.render_widget(
        Paragraph::new("Press Enter to open description in $EDITOR").style(style),
        area,
    );
}

// wrap=true for multi-line areas (description)
fn draw_field_input(f: &mut Frame, area: Rect, value: &str, active: bool, wrap: bool) {
    if active {
        // Split on newlines so the blinking cursor lands on the correct last line
        let cursor = Span::styled("_", Style::new().add_modifier(Modifier::SLOW_BLINK));
        let mut lines: Vec<Line> = value
            .split('\n')
            .map(|l| Line::from(l.to_string()))
            .collect();
        match lines.last_mut() {
            Some(last) => last.spans.push(cursor),
            None => lines.push(Line::from(vec![cursor])),
        }
        let para = Paragraph::new(Text::from(lines));
        if wrap {
            f.render_widget(para.wrap(ratatui::widgets::Wrap { trim: false }), area);
        } else {
            f.render_widget(para, area);
        }
    } else {
        let para = Paragraph::new(value).style(Style::new().add_modifier(Modifier::DIM));
        if wrap {
            f.render_widget(para.wrap(ratatui::widgets::Wrap { trim: false }), area);
        } else {
            f.render_widget(para, area);
        }
    }
}


fn render_help(f: &mut Frame, area: Rect, text: &str) {
    let height = area.height;
    if height == 0 {
        return;
    }
    let help_area = Rect {
        x: area.x + 1,
        y: area.y + height - 1,
        width: area.width.saturating_sub(2),
        height: 1,
    };
    f.render_widget(
        Paragraph::new(format!(" {text} "))
            .alignment(Alignment::Right)
            .style(Style::new().add_modifier(Modifier::DIM)),
        help_area,
    );
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let width = area.width * percent_x / 100;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect { x, y, width, height }
}
