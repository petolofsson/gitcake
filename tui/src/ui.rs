use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph},
    Frame,
};

use git_task_core::models::task::{Task, TaskStatus, TaskType};

use crate::app::{App, CreateField, EditField, Screen};

pub fn draw(f: &mut Frame, app: &App) {
    match &app.screen {
        Screen::Setup { input, error } => draw_setup(f, input, error.as_deref()),
        Screen::PullPrompt => draw_pull_prompt(f),
        Screen::TaskList { tasks, selected, message } => {
            draw_task_list(f, tasks, *selected, message.as_deref())
        }
        Screen::Detail { task, message } => draw_detail(f, task, message.as_deref()),
        Screen::Create { title, task_type, description, field } => {
            draw_create(f, title, task_type, description, field)
        }
        Screen::Edit { title, description, field, .. } => {
            draw_edit(f, title, description, field)
        }
        Screen::InitRepo { path, name, error } => draw_init_repo(f, path, name, error.as_deref()),
        Screen::SyncConfirm => draw_sync_confirm(f),
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

fn draw_pull_prompt(f: &mut Frame) {
    draw_yes_no_prompt(f, "Pull latest changes?", "y: yes  n/q: skip");
}

// ── task list ─────────────────────────────────────────────────────────────────

fn draw_task_list(f: &mut Frame, tasks: &[Task], selected: usize, message: Option<&str>) {
    let area = f.area();

    let block = outer_block("git-task");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(1)])
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

            let line = Line::from(vec![
                Span::styled(cursor.to_string(), cursor_style),
                Span::styled(id_str, row_style.add_modifier(Modifier::DIM)),
                Span::styled(type_str, row_style.add_modifier(Modifier::DIM)),
                Span::styled(title_str, row_style),
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

    let msg_text = message.unwrap_or("");
    let help = "w/s:move  d:detail  c:new  e:edit  f:cycle  Ctrl+R:sync  Ctrl+Q:quit".to_string();
    let bottom_text = if msg_text.is_empty() {
        help
    } else {
        format!("{msg_text}  ·  {help}")
    };
    f.render_widget(
        Paragraph::new(bottom_text)
            .style(Style::new().add_modifier(Modifier::DIM)),
        rows[1],
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
            Constraint::Length(1), // TITLE: label
            Constraint::Length(1), // title input
            Constraint::Length(1), // blank
            Constraint::Length(1), // TYPE: label
            Constraint::Length(1), // type selector
            Constraint::Length(1), // blank
            Constraint::Length(1), // DESCRIPTION: label
            Constraint::Length(2), // description input
            Constraint::Fill(1),   // spacer
            Constraint::Length(1), // commands
        ])
        .split(inner);

    draw_field_label(f, rows[0], "TITLE:", *field == CreateField::Title);
    draw_field_input(f, rows[1], title, *field == CreateField::Title, false);

    draw_field_label(f, rows[3], "TYPE:", *field == CreateField::Type);
    let type_str = match task_type {
        TaskType::Task => "task",
        TaskType::Bug => "bug",
        TaskType::Incident => "incident",
    };
    let active_type = *field == CreateField::Type;
    let type_style = if active_type {
        Style::new().add_modifier(Modifier::BOLD).fg(Color::Cyan)
    } else {
        Style::new().add_modifier(Modifier::DIM)
    };
    f.render_widget(
        Paragraph::new(format!("[ {type_str} ]  Space to cycle")).style(type_style),
        rows[4],
    );

    draw_field_label(f, rows[6], "DESCRIPTION: (markdown)", *field == CreateField::Description);
    draw_field_input(f, rows[7], description, *field == CreateField::Description, true);

    f.render_widget(
        Paragraph::new("Tab/Enter: next field  ·  Shift+Enter: new line  ·  Ctrl+S: save  ·  Esc: cancel")
            .style(Style::new().add_modifier(Modifier::DIM)),
        rows[9],
    );
}

// ── edit ──────────────────────────────────────────────────────────────────────

fn draw_edit(f: &mut Frame, title: &str, description: &str, field: &EditField) {
    let area = f.area();
    let block = padded_block("Edit task");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // TITLE: label
            Constraint::Length(1), // title input
            Constraint::Length(1), // blank
            Constraint::Length(1), // DESCRIPTION: label
            Constraint::Length(2), // description input
            Constraint::Fill(1),   // spacer
            Constraint::Length(1), // commands
        ])
        .split(inner);

    draw_field_label(f, rows[0], "TITLE:", *field == EditField::Title);
    draw_field_input(f, rows[1], title, *field == EditField::Title, false);

    draw_field_label(f, rows[3], "DESCRIPTION: (markdown)", *field == EditField::Description);
    draw_field_input(f, rows[4], description, *field == EditField::Description, true);

    f.render_widget(
        Paragraph::new("Tab/Enter: next field  ·  Shift+Enter: new line  ·  Ctrl+S: save  ·  Esc: cancel")
            .style(Style::new().add_modifier(Modifier::DIM)),
        rows[6],
    );
}

// ── sync confirm ──────────────────────────────────────────────────────────────

fn draw_sync_confirm(f: &mut Frame) {
    let area = f.area();
    let popup = centered_rect(50, 7, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Sync ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    let inner = block.inner(popup);
    f.render_widget(block, popup);

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
        Paragraph::new("Push local changes to remote?").alignment(Alignment::Center),
        rows[1],
    );
    f.render_widget(
        Paragraph::new("This will commit and push your task folder.")
            .alignment(Alignment::Center)
            .style(Style::new().add_modifier(Modifier::DIM)),
        rows[2],
    );

    render_help(f, popup, "Enter/y: yes  q/n: cancel");
}

// ── push prompt ───────────────────────────────────────────────────────────────

fn draw_push_prompt(f: &mut Frame) {
    draw_yes_no_prompt(f, "Push before exiting?", "y/Enter: push  n/q: quit without pushing");
}

// ── error ─────────────────────────────────────────────────────────────────────

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

// wrap=true for multi-line areas (description)
fn draw_field_input(f: &mut Frame, area: Rect, value: &str, active: bool, wrap: bool) {
    let style = if active {
        Style::new().add_modifier(Modifier::UNDERLINED)
    } else {
        Style::new().add_modifier(Modifier::DIM)
    };
    let display = format!("{value}_");
    let para = Paragraph::new(display).style(style);
    if wrap {
        f.render_widget(para.wrap(ratatui::widgets::Wrap { trim: false }), area);
    } else {
        f.render_widget(para, area);
    }
}

fn draw_yes_no_prompt(f: &mut Frame, question: &str, help: &str) {
    let area = f.area();
    let block = outer_block("git-task");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(1), Constraint::Fill(1)])
        .split(inner);

    f.render_widget(
        Paragraph::new(question).alignment(Alignment::Center),
        rows[1],
    );

    render_help(f, area, help);
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
