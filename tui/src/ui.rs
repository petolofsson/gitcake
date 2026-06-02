use std::{collections::HashMap, str::FromStr};

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, List, ListItem, ListState, Padding, Paragraph},
    Frame,
};

use gitcake_core::models::{cake::Cake, task::{Priority, Task, TaskStatus, TaskType}};

use crate::app::{App, CreateField, DetailField, Screen, TaskContext};
use crate::config::ThemeConfig;

// ── style string parser ───────────────────────────────────────────────────────

fn resolve_color(s: &str, palette: &HashMap<String, String>) -> Option<Color> {
    if let Some(alias) = palette.get(s) {
        return Color::from_str(alias).ok();
    }
    Color::from_str(s).ok()
}

fn parse_style(s: &str, palette: &HashMap<String, String>) -> Style {
    let mut style = Style::new();
    for token in s.split_whitespace() {
        match token.to_lowercase().as_str() {
            "bold"                   => style = style.add_modifier(Modifier::BOLD),
            "dim"                    => style = style.add_modifier(Modifier::DIM),
            "italic"                 => style = style.add_modifier(Modifier::ITALIC),
            "underline"|"underlined" => style = style.add_modifier(Modifier::UNDERLINED),
            t if t.starts_with("fg:") => {
                if let Some(c) = resolve_color(&t[3..], palette) { style = style.fg(c); }
            }
            t if t.starts_with("bg:") => {
                if let Some(c) = resolve_color(&t[3..], palette) { style = style.bg(c); }
            }
            t => { // bare token → treat as fg color
                if let Some(c) = resolve_color(t, palette) { style = style.fg(c); }
            }
        }
    }
    style
}

// ── theme ─────────────────────────────────────────────────────────────────────

pub struct Theme {
    /// Full parsed style for selected rows (bg + fg + optional modifiers).
    pub highlight:      Style,
    /// Foreground color for the cursor symbol and active indicators.
    pub accent:         Color,
    pub warning:        Color,
    pub danger:         Color,
    pub success:        Color,
    pub text:           Color,
    pub bg:             Color,
    pub border:         Color,
    pub border_focused: Color,
    pub muted:          Color,
    pub cursor:       String,
    pub sym_high:     String,
    pub sym_low:      String,
    pub sym_blocked:  String,
    pub sym_done:     String,
    pub sym_open:     String,
    pub sym_progress: String,
    pub sym_arrow:    String,
    pub sym_dot:      String,
}

impl Theme {
    pub fn from_config(tc: &ThemeConfig) -> Self {
        let pal = &tc.palette;
        let s   = |src: &str| parse_style(src, pal);
        let c   = |src: &str, fb: Color| s(src).fg.unwrap_or(fb);
        Self {
            highlight:      s(&tc.highlight),
            accent:         c(&tc.accent,         Color::Cyan),
            warning:        c(&tc.warning,        Color::Yellow),
            danger:         c(&tc.danger,         Color::Red),
            success:        c(&tc.success,        Color::Green),
            text:           c(&tc.text,           Color::Reset),
            bg:             c(&tc.bg,             Color::Reset),
            border:         c(&tc.border,         Color::Reset),
            border_focused: c(&tc.border_focused, Color::Cyan),
            muted:          c(&tc.muted,          Color::DarkGray),
            cursor:       tc.cursor.clone(),
            sym_high:     tc.sym_high.clone(),
            sym_low:      tc.sym_low.clone(),
            sym_blocked:  tc.sym_blocked.clone(),
            sym_done:     tc.sym_done.clone(),
            sym_open:     tc.sym_open.clone(),
            sym_progress: tc.sym_progress.clone(),
            sym_arrow:    tc.sym_arrow.clone(),
            sym_dot:      tc.sym_dot.clone(),
        }
    }

    // ── style helpers ─────────────────────────────────────────────────────────

    fn cursor_style(&self) -> Style {
        let hl_bg = self.highlight.bg.unwrap_or(Color::Blue);
        Style::new().add_modifier(Modifier::BOLD).fg(self.accent).bg(hl_bg)
    }

    fn dim(&self) -> Style {
        Style::new().fg(self.muted)
    }

    fn bold_style(&self) -> Style {
        Style::new().add_modifier(Modifier::BOLD).fg(self.text)
    }

    fn border_style(&self) -> Style {
        Style::new().fg(self.border)
    }

    fn border_focused_style(&self) -> Style {
        Style::new().fg(self.border_focused)
    }

    fn success_style(&self) -> Style {
        Style::new().fg(self.success)
    }

    // navbar chip: bg=text color, fg=bg color (falls back to Black when bg=Reset)
    fn chip_style(&self) -> Style {
        let fg = if self.bg == Color::Reset { Color::Black } else { self.bg };
        Style::new().bg(self.text).fg(fg)
    }

    fn label_style(&self) -> Style {
        Style::new().fg(self.muted)
    }

    // ── block factories ───────────────────────────────────────────────────────

    fn block(&self, title: &str) -> Block<'static> {
        let sty = self.border_style();
        Block::default()
            .title(Span::styled(format!(" {title} "), sty))
    }

    fn padded_block(&self, title: &str) -> Block<'static> {
        self.block(title).padding(Padding::new(1, 1, 1, 1))
    }

    // ── bar builder ───────────────────────────────────────────────────────────

    fn bar_line<'a>(&self, items: &[(&'a str, &'a str)]) -> Line<'a> {
        let mut spans = vec![Span::raw(" ")];
        let chip = self.chip_style();
        let lbl  = self.label_style();
        for (key, label) in items {
            spans.push(Span::styled(format!(" {key} "), chip));
            spans.push(Span::styled(format!(" {label}  "), lbl));
        }
        Line::from(spans)
    }
}

// ── entry point ───────────────────────────────────────────────────────────────

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
    let t = Theme::from_config(&app.config.theme);
    match &app.screen {
        Screen::Setup { input, error, can_cancel } =>
            draw_setup(f, input, error.as_deref(), *can_cancel, &t),
        Screen::InitRepo { path, name, error } =>
            draw_init_repo(f, path, name, error.as_deref(), &t),
        Screen::TaskList { tasks, selected, message } =>
            draw_task_list(f, task_list_params(app, tasks, *selected, message.as_deref(), &t)),
        Screen::Detail { task, message, selected_field, .. } =>
            draw_detail(f, app.context, task, message.as_deref(), *selected_field, &app.cached_cakes, &t),
        Screen::Create { task_type, assignee, cake_id, field } =>
            draw_create(f, app.context, task_type, assignee, cake_id.as_deref(), &app.cached_cakes, field, &t),
        Screen::CreateCake { title } => draw_create_cake(f, title, &t),
        Screen::PickCake { cakes, selected, filter, .. } =>
            draw_pick_cake(f, cakes, *selected, filter, &t),
        Screen::AssignTask { users, selected, filter, .. } =>
            draw_assign_task(f, users, *selected, filter, &t),
        Screen::PickAssignee { users, selected, filter, .. } =>
            draw_pick_assignee(f, users, *selected, filter, &t),
        Screen::DeleteConfirm { task_title, .. } =>
            draw_delete_confirm(f, task_title, app.context, &t),
        Screen::SyncConfirm => draw_sync_confirm(f, app.context, &t),
        Screen::PushPrompt  => draw_push_prompt(f, &t),
        Screen::PlannerView { cakes, tasks, selected } =>
            draw_planner_view(f, app, cakes, tasks, *selected, &t),
    }
}

// ── setup ─────────────────────────────────────────────────────────────────────

fn task_list_params<'a>(app: &'a App, tasks: &'a [Task], selected: usize, message: Option<&'a str>, theme: &'a Theme) -> TaskListParams<'a> {
    let (repo_name, username) = app.repo.as_ref()
        .map(|r| (r.info.name.as_str(), r.info.username.as_str()))
        .unwrap_or(("", ""));
    TaskListParams {
        context: app.context, tasks, selected, message,
        pull_error: app.pull_error.as_deref(), lock_warning: app.lock_warning.as_deref(),
        filter: &app.filter, filter_active: app.filter_active, repo_name, username, theme,
    }
}

const LOGO: &str = r#"           o8o      .                       oooo
            `"'    .o8                       `888
 .oooooooo oooo  .o888oo  .ooooo.   .oooo.    888  oooo   .ooooo.
888' `88b  `888    888   d88' `"Y8 `P  )88b   888 .8P'   d88' `88b
888   888   888    888   888        .oP"888   888888.    888ooo888
`88bod8P'   888    888 . 888   .o8 d8(  888   888 `88b.  888    .o
`8oooooo.  o888o   "888" `Y8bod8P' `Y888""8o o888o o888o `Y8bod8P'
d"     YD
"Y88888P'                                                          "#;

fn draw_setup(f: &mut Frame, input: &str, error: Option<&str>, can_cancel: bool, theme: &Theme) {
    let area = f.area();
    let block = theme.padded_block("gitcake - created by peter olofsson");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Fill(1),
        Constraint::Length(9), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ]).split(inner);
    let logo_col = Layout::default().direction(Direction::Horizontal).constraints([
        Constraint::Fill(1), Constraint::Length(67), Constraint::Fill(1),
    ]).split(rows[1]);
    f.render_widget(Paragraph::new(LOGO).style(Style::new().fg(theme.accent)), logo_col[1]);
    // rows[2] blank between logo and slogan
    f.render_widget(Paragraph::new("——————————— Everyone Deserves Cake ———————————").alignment(Alignment::Center).style(Style::new().fg(theme.warning)), rows[3]);
    // rows[4] blank between slogan and form
    let input_line = Line::from(vec![
        Span::raw("Connect your task repo ⇒ "),
        Span::styled(input, theme.bold_style()),
        Span::styled("_", Style::new().add_modifier(Modifier::SLOW_BLINK)),
    ]);
    f.render_widget(Paragraph::new(input_line).alignment(Alignment::Center), rows[5]);
    f.render_widget(
        Paragraph::new("(e.g. ~/tasks or /home/user/your-folder)")
            .alignment(Alignment::Center).style(theme.dim()),
        rows[6],
    );
    draw_error_line(f, rows[7], error, theme);
    let bar = if can_cancel {
        theme.bar_line(&[("Enter", "connect"), ("Esc", "cancel")])
    } else {
        theme.bar_line(&[("Enter", "connect"), ("^Q", "quit")])
    };
    f.render_widget(Paragraph::new(bar), rows[9]);
}

// ── init repo ─────────────────────────────────────────────────────────────────

fn draw_init_repo(f: &mut Frame, path: &str, name: &str, error: Option<&str>, theme: &Theme) {
    let area = f.area();
    let block = theme.padded_block("gitcake — initialize repo");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Fill(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1),
        Constraint::Length(1),
    ]).split(inner);
    f.render_widget(
        Paragraph::new("Empty git repo detected. Initialize as a gitcake repo?").alignment(Alignment::Center),
        rows[1],
    );
    f.render_widget(Paragraph::new(path).alignment(Alignment::Center).style(theme.dim()), rows[2]);
    let name_line = Line::from(vec![
        Span::raw("Repo name: "),
        Span::styled(name, theme.bold_style()),
        Span::styled("_", Style::new().add_modifier(Modifier::SLOW_BLINK)),
    ]);
    f.render_widget(Paragraph::new(name_line).alignment(Alignment::Center), rows[3]);
    draw_error_line(f, rows[4], error, theme);
    f.render_widget(Paragraph::new(theme.bar_line(&[("Enter", "initialize"), ("Esc", "back")])), rows[6]);
}

// ── task list ─────────────────────────────────────────────────────────────────

struct TaskListParams<'a> {
    context:      TaskContext,
    tasks:        &'a [Task],
    selected:     usize,
    message:      Option<&'a str>,
    pull_error:   Option<&'a str>,
    lock_warning: Option<&'a str>,
    filter:       &'a str,
    filter_active: bool,
    repo_name:    &'a str,
    username:     &'a str,
    theme:        &'a Theme,
}

fn draw_task_list(f: &mut Frame, p: TaskListParams<'_>) {
    let TaskListParams { context, tasks, selected, message, pull_error, lock_warning,
                         filter, filter_active, repo_name, username, theme } = p;
    let area = f.area();
    let inner_width = theme.padded_block("").inner(area).width as usize;
    let block = task_list_block(context, message, filter, filter_active, pull_error, lock_warning, theme);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Fill(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
    ]).split(inner);
    let (items, index_map, safe_sel) = build_task_items(tasks, filter, selected, inner_width, theme);
    if items.is_empty() {
        let msg = if filter.is_empty() { "No tasks yet. ^C to create one." } else { "No tasks match the filter." };
        f.render_widget(Paragraph::new(msg).alignment(Alignment::Center).style(theme.dim()), rows[0]);
    } else {
        let mut state = ListState::default();
        state.select(index_map.iter().position(|&i| i == safe_sel));
        f.render_stateful_widget(List::new(items), rows[0], &mut state);
    }
    f.render_widget(filter_line_widget(filter, filter_active), rows[1]);
    f.render_widget(Paragraph::new(info_bar(repo_name, username, theme)), rows[3]);
    let active = if context == TaskContext::Backlog { ActiveView::Backlog } else { ActiveView::Personal };
    f.render_widget(Paragraph::new(tab_strip(active, theme)), rows[4]);
    f.render_widget(Paragraph::new(hint_bar(context, theme)), rows[5]);
}

fn task_list_block(
    context: TaskContext,
    message: Option<&str>, filter: &str, filter_active: bool,
    pull_error: Option<&str>, lock_warning: Option<&str>,
    theme: &Theme,
) -> Block<'static> {
    let title = match context {
        TaskContext::Personal => " gitcake · PERSONAL VIEW ",
        TaskContext::Backlog  => " gitcake · BACKLOG VIEW ",
    };
    let title_sty = Style::new().fg(theme.warning).add_modifier(Modifier::BOLD);
    let mut block = Block::default()
        .title(Span::styled(title, title_sty))
        .padding(Padding::new(1, 1, 1, 1));
    if let Some(msg) = message {
        block = block.title_top(Line::from(format!(" {msg} ")).right_aligned());
    }
    if !filter.is_empty() && !filter_active {
        block = block.title_top(Line::from(vec![
            Span::styled(format!(" /{filter} "), theme.dim()),
            Span::styled(" Esc: clear ", theme.dim()),
        ]).left_aligned());
    }
    if let Some(err) = pull_error {
        block = block.title_bottom(Line::from(vec![
            Span::styled(format!(" ⚠ {err} "), Style::new().fg(theme.warning)),
            Span::styled(" Esc: dismiss ", theme.dim()),
        ]).left_aligned());
    }
    if let Some(warn) = lock_warning {
        block = block.title_bottom(
            Line::from(Span::styled(format!(" ⚠ {warn} "), Style::new().fg(theme.warning))).right_aligned()
        );
    }
    block
}

fn build_task_items(tasks: &[Task], filter: &str, selected: usize, inner_width: usize, theme: &Theme) -> (Vec<ListItem<'static>>, Vec<usize>, usize) {
    use crate::app::task_matches;
    let f_lower = if filter.is_empty() { String::new() } else { filter.to_lowercase() };
    let visible = |t: &Task| f_lower.is_empty() || task_matches(t, &f_lower);
    let (mut ip, mut op, mut dn) = (0usize, 0usize, 0usize);
    for t in tasks.iter().filter(|t| visible(t)) {
        match t.status { TaskStatus::InProgress => ip += 1, TaskStatus::Open => op += 1, TaskStatus::Done => dn += 1 }
    }
    let safe_sel = selected.min((ip + op + dn).saturating_sub(1));
    let mut items: Vec<ListItem<'static>> = Vec::new();
    let mut index_map: Vec<usize> = Vec::new();
    let mut prev_status: Option<TaskStatus> = None;
    for (vis_idx, task) in tasks.iter().filter(|t| visible(t)).enumerate() {
        if prev_status.as_ref() != Some(&task.status) {
            if prev_status.is_some() { items.push(ListItem::new(Line::from(""))); index_map.push(usize::MAX); }
            let (sym, label, count, color) = match task.status {
                TaskStatus::InProgress => (theme.sym_progress.as_str(), "IN PROGRESS", ip, Some(theme.warning)),
                TaskStatus::Open       => (theme.sym_open.as_str(),     "OPEN",        op, None),
                TaskStatus::Done       => (theme.sym_done.as_str(),     "DONE",        dn, None),
            };
            let hdr = color.map_or_else(
                || Style::new().add_modifier(Modifier::BOLD | Modifier::DIM),
                |c| Style::new().add_modifier(Modifier::BOLD).fg(c),
            );
            items.push(ListItem::new(Line::from(Span::styled(format!(" {sym} {label} ({count})"), hdr))));
            index_map.push(usize::MAX);
            prev_status = Some(task.status.clone());
        }
        let (row, idx) = task_row(task, vis_idx, safe_sel, inner_width, theme);
        items.push(row);
        index_map.push(idx);
    }
    if prev_status.is_some() { items.push(ListItem::new(Line::from(""))); index_map.push(usize::MAX); }
    (items, index_map, safe_sel)
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

// ── task row ──────────────────────────────────────────────────────────────────

fn task_row(task: &Task, vis_idx: usize, safe_sel: usize, inner_width: usize, theme: &Theme) -> (ListItem<'static>, usize) {
    let is_sel  = vis_idx == safe_sel;
    let base = match task.status {
        TaskStatus::Done       => Style::new().add_modifier(Modifier::DIM),
        TaskStatus::InProgress => Style::new().add_modifier(Modifier::BOLD).fg(theme.warning),
        TaskStatus::Open       => Style::new(),
    };
    let row_sty    = if is_sel { theme.highlight } else { base };
    let sp         = Style::new();
    let cursor_sty = if is_sel { Style::new().fg(theme.accent).add_modifier(Modifier::BOLD) } else { sp.add_modifier(Modifier::DIM) };
    let (flag_str, flag_sty) = task_flag(task, theme);
    let bite_prog  = bite_progress(&task.bites);
    let prog_width = bite_prog.as_ref().map(|s| s.len() + 2).unwrap_or(0);
    let title_str  = truncate_title(&task.title, inner_width.saturating_sub(24 + prog_width));
    let cursor_char = if is_sel { theme.cursor.clone() } else { " ".to_string() };
    let mut spans = vec![
        Span::styled(cursor_char,                              cursor_sty),
        Span::styled(" ",                                      sp),
        Span::styled(flag_str,                                 flag_sty),
        Span::styled(task.id.clone(),                          row_sty.add_modifier(Modifier::DIM)),
        Span::styled("  ",                                     sp),
        Span::styled(format!("{:<8}", type_label(&task.task_type)), row_sty.add_modifier(Modifier::DIM)),
        Span::styled("  ",                                     sp),
        Span::styled(title_str,                                row_sty),
    ];
    if let Some(p) = bite_prog {
        spans.push(Span::styled("  ", sp));
        spans.push(Span::styled(p, Style::new().add_modifier(Modifier::DIM)));
    }
    (ListItem::new(Line::from(spans)), vis_idx)
}

fn task_flag(task: &Task, theme: &Theme) -> (String, Style) {
    if task.status == TaskStatus::Done {
        return ("  ".to_string(), Style::new());
    }
    if task.blocked {
        return (format!("{} ", theme.sym_blocked), Style::new().fg(theme.danger).add_modifier(Modifier::BOLD));
    }
    match task.priority {
        Priority::High   => (format!("{} ", theme.sym_high),    Style::new().fg(theme.warning).add_modifier(Modifier::BOLD)),
        Priority::Low    => (format!("{} ", theme.sym_low),     Style::new().add_modifier(Modifier::DIM)),
        Priority::Normal => ("  ".to_string(),                  Style::new()),
    }
}

fn bite_progress(bites: &[gitcake_core::models::task::Bite]) -> Option<String> {
    if bites.is_empty() { return None; }
    let done = bites.iter().filter(|b| b.done).count();
    Some(format!("{done}/{}", bites.len()))
}

// ── detail ────────────────────────────────────────────────────────────────────

fn draw_detail(f: &mut Frame, _context: TaskContext, task: &Task, _message: Option<&str>, selected: DetailField, cakes: &[Cake], theme: &Theme) {
    let area = f.area();
    let sty = theme.border_style();
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" Task ", sty),
            Span::styled(task.id.clone(), sty.add_modifier(Modifier::BOLD)),
            Span::styled(" ", sty),
        ]))
        .padding(Padding::new(1, 1, 1, 1));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1),
        Constraint::Length(1),
    ]).split(inner);
    render_detail_fields(f, &rows, task, selected, cakes, theme);
    render_detail_desc(f, rows[11], task, theme);
    f.render_widget(Paragraph::new(detail_nav_bar(theme)), rows[12]);
}

fn render_detail_fields(f: &mut Frame, rows: &[Rect], task: &Task, selected: DetailField, cakes: &[Cake], theme: &Theme) {
    let render_field = |f: &mut Frame, row: Rect, field: DetailField, label: &str, value: &str| {
        let is_sel = selected == field;
        let cur = if is_sel { format!("{} ", theme.cursor) } else { "  ".to_string() };
        let (cur_sty, lbl_sty, val_sty) = if is_sel {
            (
                Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
                Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
                theme.highlight,
            )
        } else {
            (
                theme.dim(),
                Style::new().fg(theme.muted).add_modifier(Modifier::BOLD),
                theme.dim(),
            )
        };
        f.render_widget(Paragraph::new(Line::from(vec![
            Span::styled(cur, cur_sty),
            Span::styled(format!("{:<10}", label), lbl_sty),
            Span::styled(value.to_string(), val_sty),
        ])), row);
    };
    let cake_title = task.cake_id.as_deref()
        .and_then(|id| cakes.iter().find(|c| c.id == id))
        .map(|c| c.title.as_str())
        .unwrap_or("none");
    render_field(f, rows[1], DetailField::Type,     "TYPE:",     type_label(&task.task_type));
    render_field(f, rows[2], DetailField::Status,   "STATUS:",   status_label(&task.status));
    render_field(f, rows[3], DetailField::Priority, "PRIORITY:", priority_label(&task.priority));
    render_field(f, rows[4], DetailField::Blocked,  "BLOCKED:",  if task.blocked { "yes" } else { "no" });
    render_field(f, rows[5], DetailField::Cake,     "CAKE:",     cake_title);
    let file_path = format!("{}s/{}.md", type_label(&task.task_type), task.id);
    let ro = |label: &str, value: String| Paragraph::new(Line::from(vec![
        Span::raw("  "),
        Span::styled(format!("{:<10}", label), theme.bold_style()),
        Span::styled(value, theme.dim()),
    ]));
    f.render_widget(ro("FILE:",    file_path), rows[6]);
    f.render_widget(ro("CREATED:", task.created.format("%Y-%m-%d %H:%M").to_string()), rows[7]);
    f.render_widget(ro("TITLE:",   task.title.clone()), rows[9]);
}

fn render_detail_desc(f: &mut Frame, area: Rect, task: &Task, theme: &Theme) {
    let mut lines: Vec<Line> = task.description.as_deref().unwrap_or_default()
        .lines().map(|l| desc_line_render(l, theme)).collect();
    if task.order.is_some() || task.parent_id.is_some() {
        let mut meta = String::new();
        if let Some(o) = task.order      { meta.push_str(&format!("order: {o}  ")); }
        if let Some(p) = &task.parent_id { meta.push_str(&format!("parent: {p}")); }
        lines.push(desc_line_render(&meta, theme));
    }
    f.render_widget(Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false }), area);
}

fn desc_line_render(line: &str, theme: &Theme) -> Line<'static> {
    if let Some(t) = line.strip_prefix("!!bite ") {
        Line::from(vec![
            Span::styled(format!("  {} ", theme.sym_done), theme.dim()),
            Span::styled(t.to_string(), theme.dim()),
        ])
    } else if let Some(t) = line.strip_prefix("!bite ") {
        Line::from(vec![
            Span::styled(format!("  {} ", theme.sym_open), theme.bold_style()),
            Span::styled(t.to_string(), theme.bold_style()),
        ])
    } else if let Some(t) = line.strip_prefix("!!crumb ").or_else(|| line.strip_prefix("!crumb ")) {
        Line::from(vec![
            Span::styled(format!("  {} ", theme.sym_dot), theme.dim()),
            Span::styled(t.to_string(), theme.dim()),
        ])
    } else {
        Line::from(vec![Span::raw("  "), Span::styled(line.to_string(), theme.dim())])
    }
}

// ── create ────────────────────────────────────────────────────────────────────

fn draw_create(f: &mut Frame, context: TaskContext, task_type: &TaskType, assignee: &str, cake_id: Option<&str>, cakes: &[Cake], field: &CreateField, theme: &Theme) {
    let area = f.area();
    let block = theme.padded_block("New task");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1),
        Constraint::Length(1),
    ]).split(inner);
    draw_create_type_field(f, rows[0], rows[1], task_type, *field == CreateField::Type, theme);
    draw_create_assign_field(f, rows[3], rows[4], assignee, *field == CreateField::Assignee, theme);
    draw_create_cake_field(f, rows[5], rows[6], cake_id, cakes, *field == CreateField::Cake, theme);
    let confirm_active = *field == CreateField::Confirm;
    let confirm_label = if confirm_active {
        "↵ CREATE TASK  F or Enter: open editor — write '# Title' on the first line"
    } else {
        "↵ CREATE TASK  (S to reach, F or Enter to open editor)"
    };
    draw_field_label(f, rows[7], confirm_label, confirm_active, theme);
    f.render_widget(Paragraph::new(hint_bar(context, theme)), rows[9]);
}

fn draw_create_type_field(f: &mut Frame, label_row: Rect, val_row: Rect, task_type: &TaskType, active: bool, theme: &Theme) {
    draw_field_label(f, label_row, "TYPE:", active, theme);
    let style = if active { theme.highlight.add_modifier(Modifier::BOLD) } else { theme.dim() };
    f.render_widget(Paragraph::new(format!("  [ {} ]  F to cycle", type_label(task_type))).style(style), val_row);
}

fn draw_create_assign_field(f: &mut Frame, label_row: Rect, val_row: Rect, assignee: &str, active: bool, theme: &Theme) {
    draw_field_label(f, label_row, "ASSIGN TO:", active, theme);
    let text = if active { format!("  {assignee}  ← F or Enter to pick") } else { format!("  {assignee}") };
    let style = if active { theme.highlight } else { theme.dim() };
    f.render_widget(Paragraph::new(text).style(style), val_row);
}

fn draw_create_cake_field(f: &mut Frame, label_row: Rect, val_row: Rect, cake_id: Option<&str>, cakes: &[Cake], active: bool, theme: &Theme) {
    draw_field_label(f, label_row, "CAKE:", active, theme);
    let title = cake_id
        .and_then(|id| cakes.iter().find(|c| c.id == id))
        .map(|c| c.title.as_str())
        .unwrap_or("none");
    let text  = if active { format!("  {title}  ← F or Enter to pick") } else { format!("  {title}") };
    let style = if active { theme.highlight } else { theme.dim() };
    f.render_widget(Paragraph::new(text).style(style), val_row);
}

fn draw_create_cake(f: &mut Frame, title: &str, theme: &Theme) {
    let area = f.area();
    let block = theme.padded_block("New cake");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1), Constraint::Length(1),
    ]).split(inner);
    f.render_widget(Paragraph::new(Line::from(Span::styled("TITLE:", theme.bold_style()))), rows[0]);
    f.render_widget(Paragraph::new(Line::from(vec![
        Span::raw(title),
        Span::styled("_", Style::new().add_modifier(Modifier::SLOW_BLINK)),
    ])), rows[1]);
    f.render_widget(Paragraph::new(Line::from(vec![
        Span::styled("  Enter: create  ", theme.dim()),
        Span::styled("  Esc: cancel",     theme.dim()),
    ])), rows[3]);
}

// ── pickers ───────────────────────────────────────────────────────────────────

fn draw_pick_cake(f: &mut Frame, cakes: &[Cake], selected: usize, filter: &str, theme: &Theme) {
    let items: Vec<String> = std::iter::once("none".to_string())
        .chain(cakes.iter().map(|c| c.title.clone()))
        .filter(|t| filter.is_empty() || t.to_lowercase().contains(&filter.to_lowercase()))
        .collect();
    draw_user_picker(f, "Pick cake", &items, selected, filter, theme);
}

fn draw_pick_assignee(f: &mut Frame, users: &[String], selected: usize, filter: &str, theme: &Theme) {
    draw_user_picker(f, " Pick assignee ", users, selected, filter, theme);
}

fn draw_assign_task(f: &mut Frame, users: &[String], selected: usize, filter: &str, theme: &Theme) {
    draw_user_picker(f, " Assign to ", users, selected, filter, theme);
}

fn draw_user_picker(f: &mut Frame, title: &str, users: &[String], selected: usize, filter: &str, theme: &Theme) {
    let filtered: Vec<&String> = if filter.is_empty() {
        users.iter().collect()
    } else {
        users.iter().filter(|u| u.to_lowercase().starts_with(&filter.to_lowercase())).collect()
    };
    let height = (filtered.len() as u16 + 7).min(f.area().height);
    let inner  = render_popup(f, title, 50, height, theme);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1), Constraint::Length(1),
    ]).split(inner);
    f.render_widget(Paragraph::new(Line::from(vec![
        Span::styled("/ ", theme.dim()),
        Span::styled(filter, theme.bold_style()),
        Span::styled("_", Style::new().add_modifier(Modifier::SLOW_BLINK)),
    ])), rows[0]);
    if filtered.is_empty() {
        f.render_widget(Paragraph::new("No matching users.").style(theme.dim()), rows[2]);
    } else {
        let items = user_picker_items(&filtered, selected, theme);
        let mut state = ListState::default();
        state.select(Some(selected));
        f.render_stateful_widget(List::new(items), rows[2], &mut state);
    }
    f.render_widget(
        Paragraph::new("↑↓: navigate  Type to filter  Enter: select  Esc: cancel").style(theme.dim()),
        rows[3],
    );
}

fn user_picker_items(filtered: &[&String], selected: usize, theme: &Theme) -> Vec<ListItem<'static>> {
    filtered.iter().enumerate().map(|(i, u)| {
        if i == selected {
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", theme.cursor), theme.cursor_style()),
                Span::styled(u.to_string(), theme.highlight),
            ]))
        } else {
            ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(u.to_string(), Style::new()),
            ]))
        }
    }).collect()
}

// ── planner view ──────────────────────────────────────────────────────────────

fn draw_planner_view(f: &mut Frame, app: &App, cakes: &[Cake], tasks: &[(String, Task)], selected: usize, theme: &Theme) {
    let area = f.area();
    let (repo_name, username) = app.repo.as_ref()
        .map(|r| (r.info.name.as_str(), r.info.username.as_str()))
        .unwrap_or(("", ""));
    let title_sty = Style::new().fg(theme.warning).add_modifier(Modifier::BOLD);
    let block = Block::default()
        .title(Span::styled(" gitcake · PLANNER VIEW ", title_sty))
        .padding(Padding::new(1, 1, 1, 1));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Fill(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
    ]).split(inner);
    let inner_width = inner.width as usize;
    let (items, index_map) = build_planner_items(cakes, tasks, selected, &app.filter, inner_width, theme);
    if items.is_empty() {
        let msg = if app.filter.is_empty() { "No active tasks. ^C to create one." } else { "No tasks match the filter." };
        f.render_widget(Paragraph::new(msg).alignment(Alignment::Center).style(theme.dim()), rows[0]);
    } else {
        let mut state = ListState::default();
        state.select(index_map.iter().position(|e| *e == Some(selected)));
        f.render_stateful_widget(List::new(items), rows[0], &mut state);
    }
    f.render_widget(filter_line_widget(&app.filter, app.filter_active), rows[1]);
    f.render_widget(Paragraph::new(info_bar(repo_name, username, theme)), rows[3]);
    f.render_widget(Paragraph::new(tab_strip(ActiveView::Planner, theme)), rows[4]);
    f.render_widget(Paragraph::new(planner_hint_bar(theme)), rows[5]);
}

fn planner_task_row(task: &Task, owner: &str, vis_idx: usize, selected: usize, inner_width: usize, theme: &Theme) -> ListItem<'static> {
    let is_sel   = vis_idx == selected;
    let in_prog  = task.status == TaskStatus::InProgress;
    let base     = if in_prog { Style::new().add_modifier(Modifier::BOLD).fg(theme.warning) } else { Style::new() };
    let cur_sty  = if is_sel { theme.cursor_style() } else { Style::new().add_modifier(Modifier::DIM) };
    let row_sty  = if is_sel { theme.highlight } else { base };
    let status_sty = if in_prog { Style::new().fg(theme.warning) } else { Style::new().add_modifier(Modifier::DIM) };
    let status_ind = if in_prog { format!("{} ", theme.sym_arrow) } else { format!("{} ", theme.sym_dot) };
    let owner_str  = format!("{:<12}  ", truncate_title(owner, 12));
    let title_bud  = inner_width.saturating_sub(2 + 14 + 2 + 10 + 2);
    ListItem::new(Line::from(vec![
        Span::styled(if is_sel { format!("{} ", theme.cursor) } else { "  ".to_string() }, cur_sty),
        Span::styled(owner_str, row_sty.add_modifier(Modifier::DIM)),
        Span::styled(status_ind, if is_sel { row_sty } else { status_sty }),
        Span::styled(format!("{:<8}  ", type_label(&task.task_type)), row_sty.add_modifier(Modifier::DIM)),
        Span::styled(truncate_title(&task.title, title_bud), row_sty),
    ]))
}

fn build_planner_items(cakes: &[Cake], tasks: &[(String, Task)], selected: usize, filter: &str, inner_width: usize, theme: &Theme) -> (Vec<ListItem<'static>>, Vec<Option<usize>>) {
    use crate::app::task_matches;
    let f = if filter.is_empty() { String::new() } else { filter.to_lowercase() };
    let is_vis = |t: &Task| f.is_empty() || task_matches(t, &f);
    let mut items: Vec<ListItem<'static>> = Vec::new();
    let mut index_map: Vec<Option<usize>> = Vec::new();
    let mut vis_idx = 0usize;
    let cake_hdr_sty = Style::new().add_modifier(Modifier::BOLD).fg(theme.accent);
    let section_hdr_sty = Style::new().add_modifier(Modifier::BOLD).fg(theme.muted);
    let section_hdr = |title: &str| ListItem::new(Line::from(Span::styled(format!(" {title}"), section_hdr_sty)));
    for cake in cakes {
        let cake_tasks: Vec<_> = tasks.iter().filter(|(_, t)| t.cake_id.as_deref() == Some(&cake.id) && is_vis(t)).collect();
        let total = tasks.iter().filter(|(_, t)| t.cake_id.as_deref() == Some(&cake.id)).count();
        let done  = tasks.iter().filter(|(_, t)| t.cake_id.as_deref() == Some(&cake.id) && t.status == TaskStatus::Done).count();
        let progress = if total > 0 { format!("  {}/{}", total - done, total) } else { String::new() };
        items.push(ListItem::new(Line::from(vec![
            Span::styled(format!(" {}", cake.title), cake_hdr_sty),
            Span::styled(progress, Style::new().fg(theme.muted)),
        ])));
        index_map.push(None);
        for (owner, task) in &cake_tasks {
            items.push(planner_task_row(task, owner, vis_idx, selected, inner_width, theme));
            index_map.push(Some(vis_idx));
            vis_idx += 1;
        }
        items.push(ListItem::new(Line::from("")));
        index_map.push(None);
    }
    let standalone: Vec<_> = tasks.iter().filter(|(_, t)| t.cake_id.is_none() && is_vis(t)).collect();
    if !standalone.is_empty() {
        items.push(section_hdr("STANDALONE"));
        index_map.push(None);
        for (owner, task) in standalone {
            items.push(planner_task_row(task, owner, vis_idx, selected, inner_width, theme));
            index_map.push(Some(vis_idx));
            vis_idx += 1;
        }
    }
    (items, index_map)
}

// ── confirm dialogs ───────────────────────────────────────────────────────────

fn draw_sync_confirm(f: &mut Frame, context: TaskContext, theme: &Theme) {
    let inner = render_popup(f, "Push", 54, 7, theme);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1), Constraint::Length(1),
    ]).split(inner);
    let what = match context {
        TaskContext::Personal => "This will commit and push your tasks.",
        TaskContext::Backlog  => "This will commit and push the shared backlog.",
    };
    f.render_widget(Paragraph::new(what).style(theme.dim()), rows[0]);
    f.render_widget(Paragraph::new("Continue? [y/N]"), rows[1]);
    f.render_widget(Paragraph::new("y: push  Enter/n/q: cancel").style(theme.dim()), rows[3]);
}

fn draw_delete_confirm(f: &mut Frame, task_title: &str, context: TaskContext, theme: &Theme) {
    let (title, subtext) = match context {
        TaskContext::Personal => ("Move to Backlog", "Assignee will be cleared."),
        TaskContext::Backlog  => ("Delete", "This action cannot be undone."),
    };
    let inner = render_popup(f, title, 60, 8, theme);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Min(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Length(1),
    ]).split(inner);
    let question_line = match context {
        TaskContext::Personal => Line::from(vec![
            Span::raw("Move \""),
            Span::styled(task_title, theme.bold_style()),
            Span::raw("\" to the shared backlog?"),
        ]),
        TaskContext::Backlog => Line::from(vec![
            Span::raw("Are you sure you want to delete \""),
            Span::styled(task_title, theme.bold_style()),
            Span::raw("\"?"),
        ]),
    };
    f.render_widget(Paragraph::new(question_line).wrap(ratatui::widgets::Wrap { trim: true }), rows[0]);
    f.render_widget(Paragraph::new(subtext).style(theme.dim()), rows[2]);
    f.render_widget(Paragraph::new("[y/N]"), rows[4]);
}

fn draw_push_prompt(f: &mut Frame, theme: &Theme) {
    let area = f.area();
    let block = theme.padded_block("gitcake");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Fill(1), Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1),
    ]).split(inner);
    f.render_widget(
        Paragraph::new("You are closing gitcake. Do you want to push all task states?").alignment(Alignment::Center),
        rows[1],
    );
    f.render_widget(
        Paragraph::new("y: push and quit  Enter/n: quit without pushing  Esc: go back")
            .alignment(Alignment::Center).style(theme.dim()),
        rows[2],
    );
}

// ── command bars / info bar ───────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum ActiveView { Personal, Planner, Backlog }

fn info_bar<'a>(repo_name: &'a str, username: &'a str, theme: &Theme) -> Line<'a> {
    Line::from(vec![
        Span::raw(" "),
        Span::styled(repo_name, theme.dim()),
        Span::styled(" · ", theme.dim()),
        Span::styled(username, theme.dim()),
    ])
}

fn tab_strip(active: ActiveView, theme: &Theme) -> Line<'static> {
    let tabs = [
        (ActiveView::Personal, "PERSONAL"),
        (ActiveView::Planner,  "PLANNER"),
        (ActiveView::Backlog,  "BACKLOG"),
    ];
    let fg = if theme.bg == Color::Reset { Color::Black } else { theme.bg };
    let inactive = Style::new().bg(theme.text).fg(fg);
    let active_sty = Style::new().bg(theme.accent).fg(fg).add_modifier(Modifier::BOLD);
    let mut spans = vec![Span::raw(" ")];
    for (view, label) in &tabs {
        let sty = if *view == active { active_sty } else { inactive };
        spans.push(Span::styled(format!(" {label} "), sty));
        spans.push(Span::raw(" "));
    }
    Line::from(spans)
}

fn hint_bar(context: TaskContext, theme: &Theme) -> Line<'static> {
    let f_label = if context == TaskContext::Backlog { "claim" } else { "cycle" };
    theme.bar_line(&[
        ("WASD", "nav"), ("D", "detail"), ("E", "edit"), ("F", f_label),
        ("^C", "create"), ("^A", "assign"), ("^R", "push"), ("^Q", "quit"),
    ])
}

fn planner_hint_bar(theme: &Theme) -> Line<'static> {
    theme.bar_line(&[
        ("WASD", "nav"), ("D", "detail"), ("C", "cake"),
        ("^C", "create"), ("^R", "push"), ("^Q", "quit"),
    ])
}

fn detail_nav_bar(theme: &Theme) -> Line<'static> {
    theme.bar_line(&[("WS", "nav"), ("F", "cycle"), ("E", "edit"), ("^A", "assign"), ("^R", "push"), ("^Q", "quit")])
}

// ── shared helpers ────────────────────────────────────────────────────────────

fn draw_field_label(f: &mut Frame, area: Rect, label: &str, active: bool, theme: &Theme) {
    if active {
        f.render_widget(Paragraph::new(Line::from(vec![
            Span::styled(format!("{} ", theme.cursor), Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled(label.to_string(), Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)),
        ])), area);
    } else {
        f.render_widget(Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled(label.to_string(), Style::new().fg(theme.muted).add_modifier(Modifier::BOLD)),
        ])), area);
    }
}

fn draw_error_line(f: &mut Frame, area: Rect, err: Option<&str>, theme: &Theme) {
    if let Some(e) = err {
        f.render_widget(
            Paragraph::new(format!("✗ {e}"))
                .alignment(Alignment::Center)
                .style(Style::new().fg(theme.danger)),
            area,
        );
    }
}

fn render_popup(f: &mut Frame, title: &str, percent_x: u16, height: u16, theme: &Theme) -> Rect {
    let area  = f.area();
    let popup = centered_rect(percent_x, height, area);
    f.render_widget(Clear, popup);
    let block = theme.padded_block(title);
    let inner = block.inner(popup);
    f.render_widget(block, popup);
    inner
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let width = area.width * percent_x / 100;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect { x, y, width, height }
}

// ── type / status / priority labels ──────────────────────────────────────────

fn type_label(t: &TaskType) -> &'static str {
    match t { TaskType::Task => "task", TaskType::Bug => "bug", TaskType::Incident => "incident" }
}

fn status_label(s: &TaskStatus) -> &'static str {
    match s { TaskStatus::Open => "open", TaskStatus::InProgress => "in-progress", TaskStatus::Done => "done" }
}

fn priority_label(p: &Priority) -> &'static str {
    match p { Priority::High => "high", Priority::Normal => "normal", Priority::Low => "low" }
}

// ── unicode title truncation ──────────────────────────────────────────────────

fn truncate_title(s: &str, max_width: usize) -> String {
    if max_width == 0 { return String::new(); }
    if UnicodeWidthStr::width(s) <= max_width { return s.to_string(); }
    let budget = max_width.saturating_sub(1);
    let mut used = 0usize;
    let mut end  = 0usize;
    for ch in s.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(1);
        if used + w > budget { break; }
        used += w;
        end  += ch.len_utf8();
    }
    format!("{}…", &s[..end])
}
