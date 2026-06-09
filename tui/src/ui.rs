use std::{collections::HashMap, str::FromStr};

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Cell, Clear, List, ListItem, ListState, Padding, Paragraph, Row, Table, TableState},
    Frame,
};

use gitcake_core::models::{cake::Cake, task::{Priority, Task, TaskStatus, TaskType}};

use tui_input::Input;

use crate::app::{App, CreateFocus, DetailField, Screen, TaskContext};
use crate::config::ThemeConfig;

// ── style string parser ───────────────────────────────────────────────────────

fn resolve_color(s: &str, palette: &HashMap<String, String>) -> Option<Color> {
    if let Some(alias) = palette.get(s) { return Color::from_str(alias).ok(); }
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
            "reversed"|"reverse"     => style = style.add_modifier(Modifier::REVERSED),
            t if t.starts_with("fg:") => {
                if let Some(c) = resolve_color(&t[3..], palette) { style = style.fg(c); }
            }
            t if t.starts_with("bg:") => {
                if let Some(c) = resolve_color(&t[3..], palette) { style = style.bg(c); }
            }
            t => { if let Some(c) = resolve_color(t, palette) { style = style.fg(c); } }
        }
    }
    style
}

// ── theme ─────────────────────────────────────────────────────────────────────

pub struct Theme {
    pub highlight:    Style,
    pub accent:       Color,
    pub warning:      Color,
    pub danger:       Color,
    pub text:         Color,
    pub bg:           Color,
    pub border:       Color,
    pub muted:        Color,
    pub bg_personal:  Color,
    pub bg_planner:   Color,
    pub bg_backlog:   Color,
    pub cursor:       String,
    pub sym_high:     String,
    pub sym_urgent:   String,
    pub sym_blocked:  String,
    pub sym_done:     String,
    pub sym_open:     String,
    pub sym_progress: String,
    pub sym_dot:      String,
}

impl Theme {
    pub fn from_config(tc: &ThemeConfig) -> Self {
        let pal = &tc.palette;
        let s   = |src: &str| parse_style(src, pal);
        let c   = |src: &str, fb: Color| s(src).fg.unwrap_or(fb);
        Self {
            highlight:    s(&tc.highlight),
            accent:       c(&tc.accent,      Color::Cyan),
            warning:      c(&tc.warning,     Color::Yellow),
            danger:       c(&tc.danger,      Color::Red),
            text:         c(&tc.text,        Color::Reset),
            bg:           c(&tc.bg,          Color::Reset),
            border:       c(&tc.border,      Color::Reset),
            muted:        c(&tc.muted,       Color::DarkGray),
            bg_personal:  c(&tc.bg_personal, Color::Reset),
            bg_planner:   c(&tc.bg_planner,  Color::Reset),
            bg_backlog:   c(&tc.bg_backlog,  Color::Reset),
            cursor:       tc.cursor.clone(),
            sym_high:     tc.sym_high.clone(),
            sym_urgent:   tc.sym_urgent.clone(),
            sym_blocked:  tc.sym_blocked.clone(),
            sym_done:     tc.sym_done.clone(),
            sym_open:     tc.sym_open.clone(),
            sym_progress: tc.sym_progress.clone(),
            sym_dot:      tc.sym_dot.clone(),
        }
    }

    // ── style helpers ─────────────────────────────────────────────────────────

    fn cursor_style(&self) -> Style {
        if let Some(hl_bg) = self.highlight.bg {
            Style::new().add_modifier(Modifier::BOLD).fg(self.accent).bg(hl_bg)
        } else {
            Style::new().add_modifier(Modifier::BOLD | Modifier::REVERSED)
        }
    }

    fn dim(&self) -> Style { Style::new().fg(self.muted) }

    fn bold_style(&self) -> Style { Style::new().add_modifier(Modifier::BOLD).fg(self.text) }

    fn border_style(&self) -> Style { Style::new().fg(self.border) }

    fn chip_style(&self) -> Style {
        let bg = if self.text == Color::Reset { Color::White } else { self.text };
        let fg = if self.bg   == Color::Reset { Color::Black } else { self.bg   };
        Style::new().bg(bg).fg(fg)
    }

    fn label_style(&self) -> Style { Style::new().fg(self.muted) }

    // ── block factories ───────────────────────────────────────────────────────

    fn block(&self, title: &str) -> Block<'static> {
        Block::default().title(Span::styled(format!(" {title} "), self.border_style()))
    }

    fn padded_block(&self, title: &str) -> Block<'static> {
        self.block(title).padding(Padding::new(1, 1, 1, 1))
    }

    // ── bar builder ───────────────────────────────────────────────────────────

    fn bar_line<'a>(&self, items: &[(&'a str, &'a str)]) -> Line<'a> {
        bar_make_line(items, self.chip_style(), self.label_style())
    }

    fn bar_text(&self, items: &[(&'static str, &'static str)], width: u16) -> Text<'static> {
        let chip = self.chip_style();
        let lbl  = self.label_style();
        let iw = |k: &str, l: &str| -> usize {
            2 + UnicodeWidthStr::width(k) + 1 + UnicodeWidthStr::width(l) + 2
        };
        let total: usize = 1 + items.iter().map(|(k, l)| iw(k, l)).sum::<usize>();
        if total <= width as usize {
            return Text::from(bar_make_line(items, chip, lbl));
        }
        let mut w = 1usize;
        let mut split = items.len().max(1);
        for (i, (k, l)) in items.iter().enumerate() {
            w += iw(k, l);
            if w > width as usize { split = i.max(1); break; }
        }
        Text::from(vec![
            bar_make_line(&items[..split], chip, lbl),
            bar_make_line(&items[split..], chip, lbl),
        ])
    }
}

// ── entry point ───────────────────────────────────────────────────────────────

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    if area.width < 60 || area.height < 20 {
        let rows = Layout::default().direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(1), Constraint::Fill(1)])
            .split(area);
        f.render_widget(
            Paragraph::new(format!("Terminal too small ({}×{}) — resize to at least 60×20", area.width, area.height))
                .alignment(Alignment::Center).style(Style::new().add_modifier(Modifier::DIM)),
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
        Screen::Detail { task, message, selected_field, from_planner } => {
            let (rn, un) = app.repo.as_ref().map(|r| (r.info.name.as_str(), r.info.username.as_str())).unwrap_or(("", ""));
            draw_detail(f, app.context, task, message.as_deref(), *selected_field, &app.cached_cakes, *from_planner, rn, un, &t);
        }
        Screen::Create { focus, task_type, priority,
                         users, user_filter, user_sel, cakes, cake_filter, cake_sel } =>
            draw_create(f, *focus, task_type, priority,
                        users, user_filter, *user_sel, cakes, cake_filter, *cake_sel, &t),
        Screen::EditTask { task_id, title, description, context, from_planner, .. } => {
            let (rn, un) = app.repo.as_ref().map(|r| (r.info.name.as_str(), r.info.username.as_str())).unwrap_or(("", ""));
            draw_edit_task(f, *context, task_id, title, description, *from_planner, rn, un, &t);
        }
        Screen::CreateCake { title } => draw_create_cake(f, title, &t),
        Screen::PickCake { cakes, selected, filter, .. } =>
            draw_pick_cake(f, cakes, *selected, filter, &t),
        Screen::AssignTask { users, selected, filter, .. } =>
            draw_assign_task(f, users, *selected, filter, &t),
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
        filter: &app.filter, filter_active: app.filter_active, hide_done: app.hide_done,
        cakes: &app.cached_cakes,
        repo_name, username, theme,
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
    f.render_widget(Paragraph::new("——————————— Everyone Deserves Cake ———————————").alignment(Alignment::Center).style(Style::new().fg(theme.warning)), rows[3]);
    let input_line = Line::from(vec![
        Span::raw("Connect your task repo ⇒ "),
        Span::styled(input, theme.bold_style()),
        Span::styled("_", Style::new().add_modifier(Modifier::SLOW_BLINK)),
    ]);
    f.render_widget(Paragraph::new(input_line).alignment(Alignment::Center), rows[5]);
    f.render_widget(Paragraph::new("(e.g. ~/tasks or /home/user/your-folder)").alignment(Alignment::Center).style(theme.dim()), rows[6]);
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
    f.render_widget(Paragraph::new("Empty git repo detected. Initialize as a gitcake repo?").alignment(Alignment::Center), rows[1]);
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
    context:       TaskContext,
    tasks:         &'a [Task],
    selected:      usize,
    message:       Option<&'a str>,
    pull_error:    Option<&'a str>,
    lock_warning:  Option<&'a str>,
    filter:        &'a str,
    filter_active: bool,
    hide_done:     bool,
    cakes:         &'a [Cake],
    repo_name:     &'a str,
    username:      &'a str,
    theme:         &'a Theme,
}

fn draw_task_list(f: &mut Frame, p: TaskListParams<'_>) {
    let TaskListParams { context, tasks, selected, message, pull_error, lock_warning,
                         filter, filter_active, hide_done, cakes, repo_name, username, theme } = p;
    let area = f.area();
    let [top_row, rest] = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Fill(1)]).areas(area);
    let active = if context == TaskContext::Backlog { ActiveView::Backlog } else { ActiveView::Personal };
    render_top_bar(f, top_row, active, repo_name, username, None, theme);
    let view_bg = if context == TaskContext::Backlog { theme.bg_backlog } else { theme.bg_personal };
    let block = task_list_block(filter, filter_active, pull_error, lock_warning, theme)
        .style(Style::new().bg(view_bg));
    let inner = block.inner(rest);
    f.render_widget(block, rest);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Fill(1), Constraint::Length(1), Constraint::Length(2), Constraint::Length(1),
    ]).split(inner);
    let empty_msg = if filter.is_empty() { "No tasks yet. ^C to create one." } else { "No tasks match the filter." };
    // title col = inner - (cursor1+flag3+type4+bites5+owner12=25 fixed + 5 gaps)
    let title_col_width = inner.width.saturating_sub(30) as usize;
    let tasks_owned: Vec<(String, Task)> = tasks.iter().map(|t| {
        let owner = if context == TaskContext::Backlog {
            t.owner.as_deref().unwrap_or("(none)").to_string()
        } else {
            String::new()
        };
        (owner, t.clone())
    }).collect();
    let (items, index_map) = build_tree_items(cakes, &tasks_owned, selected, filter, title_col_width, hide_done, theme);
    if items.is_empty() {
        f.render_widget(Paragraph::new(empty_msg).alignment(Alignment::Center).style(theme.dim()), rows[0]);
    } else {
        let table = Table::new(items, TREE_COL_WIDTHS)
            .header(tree_column_header(hide_done))
            .column_spacing(1)
            .row_highlight_style(theme.highlight);
        let mut state = TableState::default();
        state.select(index_map.iter().position(|e| *e == Some(selected)));
        f.render_stateful_widget(table, rows[0], &mut state);
    }
    f.render_widget(filter_line_widget(filter, filter_active, theme), rows[1]);
    f.render_widget(Paragraph::new(hint_bar(context, rows[2].width, theme)), rows[2]);
    render_flash_row(f, rows[3], message, theme);
}

fn task_list_block(filter: &str, filter_active: bool, pull_error: Option<&str>, lock_warning: Option<&str>, theme: &Theme) -> Block<'static> {
    let mut block = Block::default().padding(Padding::new(1, 1, 1, 1));
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

fn filter_line_widget<'a>(filter: &'a str, filter_active: bool, theme: &Theme) -> Paragraph<'a> {
    let dim = theme.dim();
    if filter_active {
        if filter.is_empty() {
            Paragraph::new(Line::from(vec![
                Span::raw("  "),
                Span::styled("/ ", dim),
                Span::styled("_", Style::new().add_modifier(Modifier::SLOW_BLINK)),
                Span::styled("  '@' users  '!' exclude  '#' cake", dim),
            ]))
        } else {
            Paragraph::new(Line::from(vec![
                Span::raw("  "),
                Span::styled("/ ", dim),
                Span::styled(filter, Style::new().add_modifier(Modifier::BOLD)),
                Span::styled("_", Style::new().add_modifier(Modifier::SLOW_BLINK)),
            ]))
        }
    } else if !filter.is_empty() {
        Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("/{filter}"), dim),
            Span::styled("  Esc: clear", dim),
        ]))
    } else {
        Paragraph::new(Line::from(vec![Span::raw("  "), Span::styled("Use '/' to filter", dim)]))
    }
}

fn render_flash_row(f: &mut Frame, area: Rect, message: Option<&str>, theme: &Theme) {
    let line = match message {
        Some(msg) => Line::from(vec![Span::raw("  "), Span::styled(msg.to_string(), theme.dim())]),
        None      => Line::from(""),
    };
    f.render_widget(Paragraph::new(line), area);
}

// ── task row ──────────────────────────────────────────────────────────────────

fn type_char(t: &TaskType) -> &'static str {
    match t { TaskType::Task => "tsk", TaskType::Bug => "bug", TaskType::Incident => "inc" }
}

fn task_flag(task: &Task, theme: &Theme) -> (String, Style) {
    if task.status == TaskStatus::Done { return (String::new(), Style::new()); }
    if task.ai_flagged {
        return (theme.sym_blocked.clone(), Style::new().fg(theme.danger).add_modifier(Modifier::BOLD));
    }
    match task.priority {
        Priority::Urgent => (theme.sym_urgent.clone(), Style::new().fg(theme.danger).add_modifier(Modifier::BOLD)),
        Priority::High   => (theme.sym_high.clone(),   Style::new().fg(theme.warning).add_modifier(Modifier::BOLD)),
        Priority::Normal => (String::new(),            Style::new()),
    }
}

fn bite_progress(bites: &[gitcake_core::models::task::Bite]) -> Option<String> {
    if bites.is_empty() { return None; }
    let done = bites.iter().filter(|b| b.done).count();
    Some(format!("{done}/{}", bites.len()))
}

// ── tree view ─────────────────────────────────────────────────────────────────

// cursor(1) flag(3) type(4) title(fill) bites(5) owner(12)  col_spacing(1) × 5 gaps
const TREE_COL_WIDTHS: [Constraint; 6] = [
    Constraint::Length(1), Constraint::Length(3), Constraint::Length(4),
    Constraint::Fill(1),
    Constraint::Length(5), Constraint::Length(12),
];

fn tree_column_header(hide_done: bool) -> Row<'static> {
    let rev  = Style::new().add_modifier(Modifier::REVERSED);
    let revd = Style::new().add_modifier(Modifier::REVERSED | Modifier::DIM);
    let assigned_label = if hide_done { "ASSIGNED [H]" } else { "ASSIGNED" };
    Row::new(vec![
        Cell::from("").style(revd),
        Cell::from("").style(revd),
        Cell::from("TASK").style(rev),
        Cell::from("TITLE").style(rev),
        Cell::from("PRG%").style(revd),
        Cell::from(assigned_label).style(rev),
    ]).style(Style::new().add_modifier(Modifier::REVERSED))
}

fn task_row_tree(task: &Task, owner: &str, is_sel: bool, prefix: &str, theme: &Theme) -> Row<'static> {
    let base = match task.status {
        TaskStatus::Done       => Style::new().add_modifier(Modifier::DIM),
        TaskStatus::InProgress => Style::new().add_modifier(Modifier::BOLD),
        TaskStatus::Open       => Style::new(),
    };
    let cursor_sty = if is_sel {
        Style::new().add_modifier(Modifier::BOLD).fg(theme.accent)
    } else {
        Style::new().fg(theme.muted)
    };
    let (flag_str, flag_sty) = task_flag(task, theme);
    let (status_sym, status_sty) = match task.status {
        TaskStatus::InProgress => (theme.sym_progress.clone(), Style::new().add_modifier(Modifier::BOLD).fg(theme.warning)),
        TaskStatus::Open       => (theme.sym_open.clone(),     Style::new().fg(theme.text)),
        TaskStatus::Done       => (theme.sym_done.clone(),     Style::new().add_modifier(Modifier::DIM)),
    };
    let cursor_char = if is_sel { theme.cursor.clone() } else { " ".to_string() };
    let mut title_spans: Vec<Span<'static>> = Vec::new();
    if !prefix.is_empty() {
        title_spans.push(Span::styled(prefix.to_string(), theme.dim()));
    }
    title_spans.extend([
        Span::styled(status_sym, status_sty),
        Span::raw(" "),
        Span::styled(truncate_title(&task.title, 200), base),
    ]);
    Row::new(vec![
        Cell::from(cursor_char).style(cursor_sty),
        Cell::from(Line::from(flag_str).right_aligned()).style(flag_sty),
        Cell::from(type_char(&task.task_type).to_string()).style(base.add_modifier(Modifier::DIM)),
        Cell::from(Line::from(title_spans)),
        Cell::from(bite_progress(&task.bites).unwrap_or_default()).style(base.add_modifier(Modifier::DIM)),
        Cell::from(truncate_title(owner, 12)).style(base.add_modifier(Modifier::DIM)),
    ]).style(base)
}

fn build_tree_items(
    cakes: &[Cake],
    tasks: &[(String, Task)],
    selected: usize,
    filter: &str,
    title_col_width: usize,
    hide_done: bool,
    theme: &Theme,
) -> (Vec<Row<'static>>, Vec<Option<usize>>) {
    use crate::app::filter_matches_with_cakes;
    let f = if filter.is_empty() { String::new() } else { filter.to_lowercase() };
    let is_vis = |t: &Task| (f.is_empty() || filter_matches_with_cakes(t, cakes, &f))
                           && (!hide_done || t.status != TaskStatus::Done);
    let mut rows: Vec<Row<'static>> = Vec::new();
    let mut index_map: Vec<Option<usize>> = Vec::new();
    let mut vis_idx = 0usize;
    let mut first_group = true;
    let cake_hdr_sty = Style::new().add_modifier(Modifier::BOLD).fg(theme.accent);
    let rule_sty     = Style::new().fg(theme.muted);
    let make_header  = |title: String, progress: String| -> Row<'static> {
        let rule_len = title_col_width
            .saturating_sub(UnicodeWidthStr::width(title.as_str()) + UnicodeWidthStr::width(progress.as_str()) + 1);
        let hdr_line = Line::from(vec![
            Span::styled(title,                cake_hdr_sty),
            Span::styled(progress,             rule_sty),
            Span::styled(" ",                  rule_sty),
            Span::styled("─".repeat(rule_len), rule_sty),
        ]);
        Row::new(vec![
            Cell::from(""), Cell::from(""), Cell::from(""),
            Cell::from(hdr_line),
            Cell::from(""), Cell::from(""),
        ])
    };
    for cake in cakes {
        let cake_vis: Vec<(&str, &Task)> = tasks.iter()
            .filter(|(_, t)| t.cake_id.as_deref() == Some(&cake.id) && is_vis(t))
            .map(|(o, t)| (o.as_str(), t))
            .collect();
        if cake_vis.is_empty() { continue; }
        let total  = tasks.iter().filter(|(_, t)| t.cake_id.as_deref() == Some(&cake.id)).count();
        let active = tasks.iter().filter(|(_, t)| t.cake_id.as_deref() == Some(&cake.id) && t.status != TaskStatus::Done).count();
        let progress = if total > 0 { format!(" {active}/{total}") } else { String::new() };
        if !first_group { rows.push(Row::new(vec![""; 6])); index_map.push(None); }
        first_group = false;
        rows.push(make_header(cake.title.clone(), progress));
        index_map.push(None);
        render_tree_rows(&mut rows, &mut index_map, &mut vis_idx, selected, &cake_vis, theme);
    }
    let standalone: Vec<(&str, &Task)> = tasks.iter()
        .filter(|(_, t)| t.cake_id.is_none() && is_vis(t))
        .map(|(o, t)| (o.as_str(), t))
        .collect();
    if !standalone.is_empty() {
        if !first_group { rows.push(Row::new(vec![""; 6])); index_map.push(None); }
        rows.push(make_header("STANDALONE".to_string(), String::new()));
        index_map.push(None);
        render_tree_rows(&mut rows, &mut index_map, &mut vis_idx, selected, &standalone, theme);
    }
    (rows, index_map)
}

fn render_tree_rows(
    rows: &mut Vec<Row<'static>>,
    index_map: &mut Vec<Option<usize>>,
    vis_idx: &mut usize,
    selected: usize,
    group: &[(&str, &Task)],
    theme: &Theme,
) {
    let roots: Vec<(&str, &Task)> = group.iter()
        .filter(|(_, t)| t.parent_id.as_ref()
            .map_or(true, |pid| !group.iter().any(|(_, pt)| pt.id == *pid)))
        .copied()
        .collect();
    let root_count = roots.len();
    for (ri, (owner, root_task)) in roots.iter().enumerate() {
        let is_last_root = ri + 1 == root_count;
        let root_conn    = if is_last_root { "└── " } else { "├── " };
        let child_vert   = if is_last_root { "    " } else { "│   " };
        let children: Vec<(&str, &Task)> = group.iter()
            .filter(|(_, t)| t.parent_id.as_deref() == Some(root_task.id.as_str()))
            .copied()
            .collect();
        rows.push(task_row_tree(root_task, owner, *vis_idx == selected, root_conn, theme));
        index_map.push(Some(*vis_idx));
        *vis_idx += 1;
        let child_count = children.len();
        for (ci, (co, ct)) in children.iter().enumerate() {
            let is_last_child = ci + 1 == child_count;
            let prefix = format!("{}{}", child_vert, if is_last_child { "└── " } else { "├── " });
            rows.push(task_row_tree(ct, co, *vis_idx == selected, &prefix, theme));
            index_map.push(Some(*vis_idx));
            *vis_idx += 1;
        }
    }
}

// ── detail ────────────────────────────────────────────────────────────────────

fn draw_detail(f: &mut Frame, context: TaskContext, task: &Task, _message: Option<&str>, selected: DetailField, cakes: &[Cake], from_planner: bool, repo_name: &str, username: &str, theme: &Theme) {
    let area = f.area();
    let [top_row, rest] = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Fill(1)]).areas(area);
    let (crumb_label, active_tab) = if from_planner {
        ("PLANNER", ActiveView::Planner)
    } else if context == TaskContext::Backlog {
        ("BACKLOG", ActiveView::Backlog)
    } else {
        ("PERSONAL", ActiveView::Personal)
    };
    let crumb = format!("{crumb_label} › {}", task.id);
    render_top_bar(f, top_row, active_tab, repo_name, username, Some(&crumb), theme);
    let sty = theme.border_style();
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" Task ", sty),
            Span::styled(task.id.clone(), sty.add_modifier(Modifier::BOLD)),
            Span::styled(" ", sty),
        ]))
        .padding(Padding::new(1, 1, 1, 1));
    let inner = block.inner(rest);
    f.render_widget(block, rest);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Fill(1),   Constraint::Length(2),
    ]).split(inner);
    render_detail_fields(f, &rows, task, selected, cakes, theme);
    render_detail_desc(f, rows[12], task, theme);
    f.render_widget(Paragraph::new(detail_nav_bar(rows[13].width, theme)), rows[13]);
}

fn render_detail_fields(f: &mut Frame, rows: &[Rect], task: &Task, selected: DetailField, cakes: &[Cake], theme: &Theme) {
    let render_field = |f: &mut Frame, row: Rect, field: DetailField, label: &str, value: &str| {
        let is_sel = selected == field;
        let cur = if is_sel { format!("{} ", theme.cursor) } else { "  ".to_string() };
        let (cur_sty, lbl_sty, val_sty) = if is_sel {
            (Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
             Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
             theme.highlight)
        } else {
            (theme.dim(), Style::new().fg(theme.muted).add_modifier(Modifier::BOLD), theme.dim())
        };
        f.render_widget(Paragraph::new(Line::from(vec![
            Span::styled(cur, cur_sty),
            Span::styled(format!("{:<12}", label), lbl_sty),
            Span::styled(value.to_string(), val_sty),
        ])), row);
    };
    let cake_title = task.cake_id.as_deref()
        .and_then(|id| cakes.iter().find(|c| c.id == id))
        .map(|c| c.title.as_str()).unwrap_or("none");
    let owner_val = task.owner.as_deref().unwrap_or("none");
    render_field(f, rows[1],  DetailField::Type,      "TYPE:",       type_label(&task.task_type));
    render_field(f, rows[2],  DetailField::Status,    "STATUS:",     status_label(&task.status));
    render_field(f, rows[3],  DetailField::Priority,  "PRIORITY:",   priority_label(&task.priority));
    render_field(f, rows[4],  DetailField::AiFlagged, "AI FLAGGED:", if task.ai_flagged { "yes" } else { "no" });
    render_field(f, rows[5],  DetailField::Cake,      "CAKE:",       cake_title);
    render_field(f, rows[6],  DetailField::Assign,    "ASSIGN:",     owner_val);
    let file_path = format!("{}s/{}.md", type_label(&task.task_type), task.id);
    let ro = |label: &str, value: String| Paragraph::new(Line::from(vec![
        Span::raw("  "),
        Span::styled(format!("{:<12}", label), theme.bold_style()),
        Span::styled(value, theme.dim()),
    ]));
    f.render_widget(ro("FILE:",    file_path), rows[7]);
    f.render_widget(ro("CREATED:", task.created.format("%Y-%m-%d %H:%M").to_string()), rows[8]);
    f.render_widget(ro("TITLE:",   task.title.clone()), rows[10]);
}

fn render_detail_desc(f: &mut Frame, area: Rect, task: &Task, theme: &Theme) {
    const MAX: usize = 850;
    let desc_str = task.description.as_deref().unwrap_or_default();
    let count = desc_str.chars().count();
    let counter_sty = if count > MAX {
        Style::new().fg(theme.danger).add_modifier(Modifier::BOLD)
    } else if count > MAX * 7 / 10 {
        Style::new().fg(theme.warning)
    } else {
        theme.dim()
    };
    let mut lines: Vec<Line> = Vec::new();
    if !desc_str.is_empty() {
        lines.push(Line::from(Span::styled(format!("  {count}/{MAX} chars"), counter_sty)));
    }
    lines.extend(desc_str.lines().map(|l| desc_line_render(l, theme)));
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
        Line::from(vec![Span::styled(format!("  {} ", theme.sym_done), theme.dim()), Span::styled(t.to_string(), theme.dim())])
    } else if let Some(t) = line.strip_prefix("!bite ") {
        Line::from(vec![Span::styled(format!("  {} ", theme.sym_open), theme.bold_style()), Span::styled(t.to_string(), theme.bold_style())])
    } else if let Some(t) = line.strip_prefix("!!crumb ").or_else(|| line.strip_prefix("!crumb ")) {
        Line::from(vec![Span::styled(format!("  {} ", theme.sym_dot), theme.dim()), Span::styled(t.to_string(), theme.dim())])
    } else {
        Line::from(vec![Span::raw("  "), Span::styled(line.to_string(), theme.dim())])
    }
}

// ── edit task ─────────────────────────────────────────────────────────────────

fn draw_edit_task(f: &mut Frame, context: TaskContext, task_id: &str, title: &Input, description: &str, from_planner: bool, repo_name: &str, username: &str, theme: &Theme) {
    let area = f.area();
    let [top_row, rest] = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Fill(1)]).areas(area);
    let active_tab = if from_planner { ActiveView::Planner }
        else if context == TaskContext::Backlog { ActiveView::Backlog }
        else { ActiveView::Personal };
    let crumb = format!("EDIT › {task_id}");
    render_top_bar(f, top_row, active_tab, repo_name, username, Some(&crumb), theme);
    let block = theme.padded_block("Edit Task");
    let inner = block.inner(rest);
    f.render_widget(block, rest);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Length(1),  // TITLE
        Constraint::Length(1),  // blank
        Constraint::Length(1),  // DESC label
        Constraint::Fill(1),    // DESC preview
        Constraint::Length(2),  // hint bar
    ]).split(inner);
    draw_edit_title(f, rows[0], title, true, theme);
    let desc_lbl = create_field_label(false, theme);
    f.render_widget(Paragraph::new(Line::from(vec![
        Span::styled("  DESC    ", desc_lbl),
        Span::styled("Tab to open editor", theme.dim()),
    ])), rows[2]);
    let preview = if description.is_empty() { "(no description)" } else { description };
    f.render_widget(Paragraph::new(preview).style(theme.dim()), rows[3]);
    f.render_widget(Paragraph::new(theme.bar_text(&[
        ("Tab", "edit desc"), ("↵", "save"), ("Esc", "cancel"), ("^Q", "quit"),
    ], rows[4].width)), rows[4]);
}

fn draw_edit_title(f: &mut Frame, area: Rect, input: &Input, active: bool, theme: &Theme) {
    let label_sty = create_field_label(active, theme);
    let prefix = "  TITLE   ";
    let pw = prefix.len() as u16;
    let iw = area.width.saturating_sub(pw + 2) as usize;
    let scroll = input.visual_scroll(iw);
    let display: String = input.value().chars().skip(scroll).take(iw).collect();
    f.render_widget(Paragraph::new(Line::from(vec![
        Span::styled(prefix, label_sty),
        Span::styled(display, if active { Style::new() } else { theme.dim() }),
    ])), area);
    if active {
        let col = (input.visual_cursor().max(scroll) - scroll) as u16;
        f.set_cursor_position((area.x + pw + col, area.y));
    }
}

// ── create ────────────────────────────────────────────────────────────────────

fn create_field_label(active: bool, theme: &Theme) -> Style {
    if active { Style::new().fg(theme.accent).add_modifier(Modifier::BOLD) }
    else       { Style::new().fg(theme.muted).add_modifier(Modifier::BOLD) }
}

fn num_key_badge(n: &'static str) -> Span<'static> {
    Span::raw(format!("[{n}]"))
}

fn num_field_label(s: &'static str) -> Span<'static> {
    Span::styled(format!(" {s:<8} "), Style::new().add_modifier(Modifier::BOLD))
}

#[allow(clippy::too_many_arguments)]
fn draw_create(f: &mut Frame, focus: CreateFocus, task_type: &TaskType, priority: &Priority, users: &[String], user_filter: &str, user_sel: usize, cakes: &[Cake], cake_filter: &str, cake_sel: usize, theme: &Theme) {
    let area = f.area();
    let assign_h: u16 = if focus == CreateFocus::Assignee { 4 } else { 1 };
    let cake_h:   u16 = if focus == CreateFocus::Cake     { 4 } else { 1 };
    let popup_h = 10 + assign_h + cake_h;
    let popup = centered_rect(65, popup_h, area);
    f.render_widget(Clear, popup);
    let block = theme.padded_block("New Task");
    let inner = block.inner(popup);
    f.render_widget(block, popup);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Length(1),         // hint line
        Constraint::Length(1),         // blank
        Constraint::Length(1),         // [1] TYPE
        Constraint::Length(1),         // [2] PRIORITY
        Constraint::Length(assign_h),  // [3] ASSIGN
        Constraint::Length(cake_h),    // [4] CAKE
        Constraint::Length(1),         // blank
        Constraint::Length(1),         // hint bar
    ]).split(inner);
    f.render_widget(
        Paragraph::new(Span::styled("Use numbers 1-4 to configure your slice.", theme.dim())),
        rows[0],
    );
    draw_create_type_chips(f, rows[2], task_type, theme);
    draw_create_priority_chips(f, rows[3], priority, theme);
    draw_create_assign(f, rows[4], users, user_filter, user_sel, focus == CreateFocus::Assignee, theme);
    draw_create_cake_field(f, rows[5], cakes, cake_filter, cake_sel, focus == CreateFocus::Cake, theme);
    f.render_widget(Paragraph::new(create_hint_bar_text(rows[7].width, theme)), rows[7]);
}

fn draw_create_type_chips(f: &mut Frame, area: Rect, task_type: &TaskType, theme: &Theme) {
    let types = [TaskType::Task, TaskType::Bug, TaskType::Incident];
    let mut spans = vec![num_key_badge("1"), num_field_label("TYPE")];
    for t in &types {
        if t == task_type {
            spans.push(Span::styled(format!("[{}]", type_label(t)), Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)));
        } else {
            spans.push(Span::styled(format!(" {} ", type_label(t)), theme.dim()));
        }
        spans.push(Span::raw("  "));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_create_priority_chips(f: &mut Frame, area: Rect, priority: &Priority, theme: &Theme) {
    let priorities = [Priority::Normal, Priority::High, Priority::Urgent];
    let mut spans = vec![num_key_badge("2"), num_field_label("PRIORITY")];
    for p in &priorities {
        if p == priority {
            spans.push(Span::styled(format!("[{}]", priority_label(p)), Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)));
        } else {
            spans.push(Span::styled(format!(" {} ", priority_label(p)), theme.dim()));
        }
        spans.push(Span::raw("  "));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_create_assign(f: &mut Frame, area: Rect, users: &[String], filter: &str, sel: usize, active: bool, theme: &Theme) {
    let f_lower = filter.to_lowercase();
    let filtered: Vec<&str> = users.iter()
        .filter(|u| f_lower.is_empty() || u.to_lowercase().contains(&f_lower))
        .map(|s| s.as_str()).collect();
    let label = num_field_label("ASSIGN");
    let key   = num_key_badge("3");
    if !active {
        let val = filtered.get(sel).copied().unwrap_or("none");
        f.render_widget(Paragraph::new(Line::from(vec![
            key, label, Span::styled(val.to_string(), theme.dim()),
        ])), area);
        return;
    }
    let sub = Layout::default().direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(1); area.height as usize]).split(area);
    f.render_widget(Paragraph::new(Line::from(vec![
        key, label.clone(), Span::styled(filter.to_string(), Style::new()),
    ])), sub[0]);
    // "[3] ASSIGN   " = 3 + 10 = 13 display cols
    f.set_cursor_position((sub[0].x + 13 + filter.len() as u16, sub[0].y));
    for (i, row) in sub.iter().enumerate().skip(1) {
        if let Some(name) = filtered.get(i - 1) {
            let is_sel = (i - 1) == sel;
            let (cur, cur_sty, row_sty) = if is_sel {
                (format!("{} ", theme.cursor), Style::new().fg(theme.accent), theme.highlight)
            } else {
                ("  ".to_string(), theme.dim(), Style::new())
            };
            f.render_widget(Paragraph::new(Line::from(vec![
                Span::raw("    "), Span::styled(cur, cur_sty), Span::styled(name.to_string(), row_sty),
            ])), *row);
        }
    }
}

fn draw_create_cake_field(f: &mut Frame, area: Rect, cakes: &[Cake], filter: &str, sel: usize, active: bool, theme: &Theme) {
    let f_lower = filter.to_lowercase();
    let none_vis = f_lower.is_empty() || "none".contains(&f_lower);
    let opts: Vec<Option<&Cake>> = (if none_vis { vec![None] } else { vec![] })
        .into_iter()
        .chain(cakes.iter().filter(|c| f_lower.is_empty() || c.title.to_lowercase().contains(&f_lower)).map(Some))
        .collect();
    let cake_name = |entry: Option<&Cake>| -> String {
        entry.map(|c| c.title.clone()).unwrap_or_else(|| "None (default)".to_string())
    };
    let label = num_field_label("CAKE");
    let key   = num_key_badge("4");
    if !active {
        let val = opts.get(sel).map(|e| cake_name(*e)).unwrap_or_else(|| "None (default)".to_string());
        f.render_widget(Paragraph::new(Line::from(vec![
            key, label, Span::styled(val, theme.dim()),
        ])), area);
        return;
    }
    let sub = Layout::default().direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(1); area.height as usize]).split(area);
    f.render_widget(Paragraph::new(Line::from(vec![
        key, label.clone(), Span::styled(filter.to_string(), Style::new()),
    ])), sub[0]);
    // "[4] CAKE     " = 3 + 10 = 13 display cols
    f.set_cursor_position((sub[0].x + 13 + filter.len() as u16, sub[0].y));
    for (i, row) in sub.iter().enumerate().skip(1) {
        if let Some(entry) = opts.get(i - 1) {
            let name = cake_name(*entry);
            let is_sel = i - 1 == sel;
            let (cur, cur_sty, row_sty) = if is_sel {
                (format!("{} ", theme.cursor), Style::new().fg(theme.accent), theme.highlight)
            } else {
                ("  ".to_string(), theme.dim(), Style::new())
            };
            f.render_widget(Paragraph::new(Line::from(vec![
                Span::raw("    "), Span::styled(cur, cur_sty), Span::styled(name.to_string(), row_sty),
            ])), *row);
        }
    }
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
    f.render_widget(Paragraph::new("↑↓: navigate  Type to filter  Enter: select  Esc: cancel").style(theme.dim()), rows[3]);
}

fn user_picker_items(filtered: &[&String], selected: usize, theme: &Theme) -> Vec<ListItem<'static>> {
    filtered.iter().enumerate().map(|(i, u)| {
        if i == selected {
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", theme.cursor), theme.cursor_style()),
                Span::styled(u.to_string(), theme.highlight),
            ]))
        } else {
            ListItem::new(Line::from(vec![Span::raw("  "), Span::styled(u.to_string(), Style::new())]))
        }
    }).collect()
}

// ── planner view ──────────────────────────────────────────────────────────────

fn draw_planner_view(f: &mut Frame, app: &App, cakes: &[Cake], tasks: &[(String, Task)], selected: usize, theme: &Theme) {
    let area = f.area();
    let (repo_name, username) = app.repo.as_ref()
        .map(|r| (r.info.name.as_str(), r.info.username.as_str())).unwrap_or(("", ""));
    let [top_row, rest] = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Fill(1)]).areas(area);
    render_top_bar(f, top_row, ActiveView::Planner, repo_name, username, None, theme);
    let block = Block::default().padding(Padding::new(1, 1, 1, 1)).style(Style::new().bg(theme.bg_planner));
    let inner = block.inner(rest);
    f.render_widget(block, rest);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Fill(1), Constraint::Length(1), Constraint::Length(2), Constraint::Length(1),
    ]).split(inner);
    let empty_msg = if app.filter.is_empty() { "No active tasks. ^C to create one." } else { "No tasks match the filter." };
    // title col = inner - fixed (1+3+4+5+12=25) - 5 gaps = 30
    let title_col_width = inner.width.saturating_sub(30) as usize;
    let (items, index_map) = build_tree_items(cakes, tasks, selected, &app.filter, title_col_width, app.hide_done, theme);
    if items.is_empty() {
        f.render_widget(Paragraph::new(empty_msg).alignment(Alignment::Center).style(theme.dim()), rows[0]);
    } else {
        let table = Table::new(items, TREE_COL_WIDTHS)
            .header(tree_column_header(app.hide_done))
            .column_spacing(1)
            .row_highlight_style(theme.highlight);
        let mut state = TableState::default();
        state.select(index_map.iter().position(|e| *e == Some(selected)));
        f.render_stateful_widget(table, rows[0], &mut state);
    }
    f.render_widget(filter_line_widget(&app.filter, app.filter_active, theme), rows[1]);
    f.render_widget(Paragraph::new(planner_hint_bar(rows[2].width, theme)), rows[2]);
    render_flash_row(f, rows[3], None, theme);
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
            Span::raw("Move \""), Span::styled(task_title, theme.bold_style()), Span::raw("\" to the shared backlog?"),
        ]),
        TaskContext::Backlog => Line::from(vec![
            Span::raw("Are you sure you want to delete \""), Span::styled(task_title, theme.bold_style()), Span::raw("\"?"),
        ]),
    };
    f.render_widget(Paragraph::new(question_line).wrap(ratatui::widgets::Wrap { trim: true }), rows[0]);
    f.render_widget(Paragraph::new(subtext).style(theme.dim()), rows[2]);
    f.render_widget(Paragraph::new("[y/N]"), rows[4]);
}

fn draw_push_prompt(f: &mut Frame, theme: &Theme) {
    let inner = render_popup(f, "Quit gitcake", 60, 7, theme);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1), Constraint::Length(2),
    ]).split(inner);
    f.render_widget(Paragraph::new("You have uncommitted task changes.").style(theme.dim()), rows[0]);
    f.render_widget(Paragraph::new("Push before quitting? [y/N]"), rows[1]);
    f.render_widget(Paragraph::new(theme.bar_text(&[
        ("y", "push + quit"), ("Enter/n", "quit"), ("Esc", "cancel"),
    ], rows[3].width)), rows[3]);
}

// ── command bars / top bar ────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum ActiveView { Personal, Planner, Backlog }

fn render_top_bar(f: &mut Frame, area: Rect, active: ActiveView, repo_name: &str, username: &str, breadcrumb: Option<&str>, theme: &Theme) {
    let bar_bg = if theme.text == Color::Reset { Color::White } else { theme.text };
    f.render_widget(Block::default().style(Style::new().bg(bar_bg)), area);
    let tabs = [(ActiveView::Personal, "PERSONAL"), (ActiveView::Planner, "PLANNER"), (ActiveView::Backlog, "BACKLOG")];
    let active_fg    = theme.highlight.bg.unwrap_or(Color::Rgb(0, 31, 96));
    let inactive_sty = Style::new().bg(bar_bg).fg(Color::DarkGray);
    let active_sty   = Style::new().bg(bar_bg).fg(active_fg).add_modifier(Modifier::BOLD);
    let mut spans = vec![Span::raw("  ")];
    for (view, label) in &tabs {
        if *view == active {
            spans.push(Span::styled(format!("{} {label}  ", theme.cursor), active_sty));
        } else {
            spans.push(Span::styled(format!("  {label}  "), inactive_sty));
        }
        spans.push(Span::raw("   "));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
    let right = if let Some(crumb) = breadcrumb {
        format!(" {} ", crumb)
    } else {
        format!(" {} · {} ", repo_name, username)
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(right, Style::new().bg(bar_bg).fg(Color::DarkGray)))).alignment(Alignment::Right),
        area,
    );
}

fn bar_make_line<'a>(items: &[(&'a str, &'a str)], chip: Style, lbl: Style) -> Line<'a> {
    let mut spans = vec![Span::raw(" ")];
    for (key, label) in items {
        spans.push(Span::styled(format!(" {key} "), chip));
        spans.push(Span::styled(format!(" {label}  "), lbl));
    }
    Line::from(spans)
}

fn hint_bar(context: TaskContext, width: u16, theme: &Theme) -> Text<'static> {
    if context == TaskContext::Backlog {
        theme.bar_text(&[
            ("WASD", "nav"), ("^C", "create"), ("E", "edit"),
            ("^A", "assign"), ("^D", "delete"), ("H", "done"),
            ("⇧R", "pull"), ("^R", "push"), ("^Q", "quit"),
        ], width)
    } else {
        theme.bar_text(&[
            ("WASD", "nav"), ("^C", "create"), ("E", "edit"), ("F", "cycle"),
            ("^A", "assign"), ("^B", "backlog"), ("H", "done"),
            ("⇧R", "pull"), ("^R", "push"), ("^Q", "quit"),
        ], width)
    }
}

fn planner_hint_bar(width: u16, theme: &Theme) -> Text<'static> {
    theme.bar_text(&[
        ("WASD", "nav"), ("^C", "create"), ("C", "cake"),
        ("^A", "assign"), ("^B", "backlog"), ("H", "done"),
        ("⇧R", "pull"), ("^R", "push"), ("^Q", "quit"),
    ], width)
}

fn detail_nav_bar(width: u16, theme: &Theme) -> Text<'static> {
    theme.bar_text(&[
        ("WASD", "nav"), ("F", "cycle"), ("E", "edit"),
        ("⇧R", "pull"), ("^R", "push"), ("^Q", "quit"),
    ], width)
}

fn create_hint_bar_text(width: u16, theme: &Theme) -> Text<'static> {
    theme.bar_text(&[("↵/^C", "write & create"), ("3/4", "focus+type"), ("Esc", "cancel")], width)
}

// ── shared helpers ────────────────────────────────────────────────────────────

fn draw_error_line(f: &mut Frame, area: Rect, err: Option<&str>, theme: &Theme) {
    if let Some(e) = err {
        f.render_widget(
            Paragraph::new(format!("✗ {e}")).alignment(Alignment::Center).style(Style::new().fg(theme.danger)),
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
    match p { Priority::Urgent => "urgent", Priority::High => "high", Priority::Normal => "normal" }
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
