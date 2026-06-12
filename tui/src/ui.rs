use std::{collections::HashMap, str::FromStr};

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Cell, Clear, List, ListItem, ListState, Padding, Paragraph, Row, Table, TableState},
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
    pub highlight:      Style,
    pub accent:         Color,
    pub in_progress:    Color,
    pub flag:           Color,
    pub priority_color: Color,
    pub fg:             Color,
    pub bg:             Color,
    pub dim:            Color,
    pub faint:          Color,
    pub border:         Color,
    pub sel_bg:         Color,
    pub done_color:     Color,
    pub open_color:     Color,
    pub bg_personal:    Color,
    pub bg_planner:     Color,
    pub bg_backlog:     Color,
    pub cursor:         String,
    pub sym_high:       String,
    pub sym_urgent:     String,
    pub sym_blocked:    String,
    pub sym_done:       String,
    pub sym_open:       String,
    pub sym_progress:   String,
    pub sym_dot:        String,
}

impl Theme {
    pub fn from_config(tc: &ThemeConfig) -> Self {
        let pal = &tc.palette;
        let s   = |src: &str| parse_style(src, pal);
        let c   = |src: &str, fb: Color| s(src).fg.unwrap_or(fb);
        Self {
            highlight:      s(&tc.highlight),
            accent:         c(&tc.accent,         Color::Rgb(230, 164, 180)),
            in_progress:    c(&tc.in_progress,    Color::Yellow),
            flag:           c(&tc.flag,           Color::Red),
            priority_color: c(&tc.priority_color, Color::Rgb(217, 140, 106)),
            fg:             c(&tc.fg,             Color::Reset),
            bg:             c(&tc.bg,             Color::Reset),
            dim:            c(&tc.dim,            Color::DarkGray),
            faint:          c(&tc.faint,          Color::DarkGray),
            border:         c(&tc.border,         Color::DarkGray),
            sel_bg:         c(&tc.sel_bg,         Color::Reset),
            done_color:     c(&tc.done_color,     Color::Green),
            open_color:     c(&tc.open_color,     Color::DarkGray),
            bg_personal:    c(&tc.bg_personal,    Color::Reset),
            bg_planner:     c(&tc.bg_planner,     Color::Reset),
            bg_backlog:     c(&tc.bg_backlog,     Color::Reset),
            cursor:         tc.cursor.clone(),
            sym_high:       tc.sym_high.clone(),
            sym_urgent:     tc.sym_urgent.clone(),
            sym_blocked:    tc.sym_blocked.clone(),
            sym_done:       tc.sym_done.clone(),
            sym_open:       tc.sym_open.clone(),
            sym_progress:   tc.sym_progress.clone(),
            sym_dot:        tc.sym_dot.clone(),
        }
    }

    // ── style helpers ─────────────────────────────────────────────────────────

    fn dim_style(&self) -> Style { Style::new().fg(self.dim) }

    fn bold_style(&self) -> Style { Style::new().add_modifier(Modifier::BOLD).fg(self.fg) }

    fn chip_style(&self) -> Style { Style::new().bg(self.sel_bg).fg(self.fg) }

    fn chip_cap_style(&self) -> Style { Style::new().fg(self.sel_bg).bg(self.bg) }

    fn label_style(&self) -> Style { Style::new().fg(self.dim) }

    fn cursor_style(&self) -> Style {
        Style::new().add_modifier(Modifier::BOLD).fg(self.accent).bg(self.sel_bg)
    }

    // ── bar builders ──────────────────────────────────────────────────────────

    fn bar_line<'a>(&self, items: &[(&'a str, &'a str)]) -> Line<'a> {
        bar_make_line(items, self.chip_style(), self.chip_cap_style(), self.label_style())
    }

    fn bar_text(&self, items: &[(&'static str, &'static str)], width: u16) -> Text<'static> {
        let chip = self.chip_style();
        let cap  = self.chip_cap_style();
        let lbl  = self.label_style();
        // each chip is now: cap(1) + " key "(k+2) + cap(1) + " label  "(l+3)
        let iw = |k: &str, l: &str| -> usize {
            1 + 1 + UnicodeWidthStr::width(k) + 1 + 1 + 1 + UnicodeWidthStr::width(l) + 2
        };
        let total: usize = 1 + items.iter().map(|(k, l)| iw(k, l)).sum::<usize>();
        if total <= width as usize {
            return Text::from(bar_make_line(items, chip, cap, lbl));
        }
        let mut w = 1usize;
        let mut split = items.len().max(1);
        for (i, (k, l)) in items.iter().enumerate() {
            w += iw(k, l);
            if w > width as usize { split = i.max(1); break; }
        }
        Text::from(vec![
            bar_make_line(&items[..split], chip, cap, lbl),
            bar_make_line(&items[split..], chip, cap, lbl),
        ])
    }
}

// ── panel factory ─────────────────────────────────────────────────────────────

fn panel(title: &str, theme: &Theme) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.border))
        .title(Span::styled(
            format!(" {title} "),
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Left)
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
        Screen::TaskList { tasks, selected, message } => {
            let active = if app.context == TaskContext::Backlog { ActiveView::Backlog } else { ActiveView::Personal };
            let (rn, un) = app.repo.as_ref().map(|r| (r.info.name.as_str(), r.info.username.as_str())).unwrap_or(("", ""));
            let owned: Vec<(String, Task)> = tasks.iter().map(|t| {
                let owner = if active == ActiveView::Backlog {
                    t.owner.as_deref().unwrap_or("(none)").to_string()
                } else { String::new() };
                (owner, t.clone())
            }).collect();
            draw_list_view(f, ListViewParams {
                active, tasks: &owned, selected: *selected, message: message.as_deref(),
                pull_error: app.pull_error.as_deref(), lock_warning: app.lock_warning.as_deref(),
                filter: &app.filter, filter_active: app.filter_active, hide_done: app.hide_done,
                cakes: &app.cached_cakes, repo_name: rn, username: un, theme: &t,
            });
        }
        Screen::Detail { task, siblings, message, selected_field, from_planner } => {
            let (rn, un) = app.repo.as_ref().map(|r| (r.info.name.as_str(), r.info.username.as_str())).unwrap_or(("", ""));
            draw_detail(f, app.context, task, siblings, message.as_deref(), *selected_field, &app.cached_cakes, *from_planner, rn, un, app.git_ahead.as_ref(), &t);
        }
        Screen::Create { title, focus, task_type, priority,
                         users, user_filter, user_sel, cakes, cake_filter, cake_sel } => {
            let (rn, un) = app.repo.as_ref().map(|r| (r.info.name.as_str(), r.info.username.as_str())).unwrap_or(("", ""));
            draw_create(f, *focus, title, task_type, priority,
                        users, user_filter, *user_sel, cakes, cake_filter, *cake_sel, &t, rn, un);
        }
        Screen::CreateCake { title } => draw_create_cake(f, title, &t),
        Screen::PickCake { cakes, selected, filter, .. } =>
            draw_pick_cake(f, cakes, *selected, filter, &t),
        Screen::AssignTask { users, selected, filter, .. } =>
            draw_assign_task(f, users, *selected, filter, &t),
        Screen::DeleteConfirm { task_title, .. } =>
            draw_delete_confirm(f, task_title, &t),
        Screen::SyncConfirm => draw_sync_confirm(f, app.context, &t),
        Screen::PushPrompt  => draw_push_prompt(f, &t),
        Screen::PlannerView { cakes, tasks, selected } => {
            let (rn, un) = app.repo.as_ref().map(|r| (r.info.name.as_str(), r.info.username.as_str())).unwrap_or(("", ""));
            draw_list_view(f, ListViewParams {
                active: ActiveView::Planner, tasks, selected: *selected, message: None,
                pull_error: None, lock_warning: None,
                filter: &app.filter, filter_active: app.filter_active, hide_done: app.hide_done,
                cakes, repo_name: rn, username: un, theme: &t,
            });
        }
    }
}

// ── setup ─────────────────────────────────────────────────────────────────────

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
    let block = panel("gitcake", theme).padding(Padding::new(1, 1, 1, 1));
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
    f.render_widget(Paragraph::new("——————————— Everyone Deserves Cake ———————————").alignment(Alignment::Center).style(Style::new().fg(theme.in_progress)), rows[3]);
    let input_line = Line::from(vec![
        Span::raw("Connect your task repo ⇒ "),
        Span::styled(input, theme.bold_style()),
        Span::styled("_", Style::new().add_modifier(Modifier::SLOW_BLINK)),
    ]);
    f.render_widget(Paragraph::new(input_line).alignment(Alignment::Center), rows[5]);
    f.render_widget(Paragraph::new("(e.g. ~/tasks or /home/user/your-folder)").alignment(Alignment::Center).style(theme.dim_style()), rows[6]);
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
    let block = panel("gitcake — initialize repo", theme).padding(Padding::new(1, 1, 1, 1));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Fill(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1),
        Constraint::Length(1),
    ]).split(inner);
    f.render_widget(Paragraph::new("Empty git repo detected. Initialize as a gitcake repo?").alignment(Alignment::Center), rows[1]);
    f.render_widget(Paragraph::new(path).alignment(Alignment::Center).style(theme.dim_style()), rows[2]);
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

struct ListViewParams<'a> {
    active:        ActiveView,
    tasks:         &'a [(String, Task)],
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

fn draw_list_view(f: &mut Frame, p: ListViewParams<'_>) {
    let ListViewParams { active, tasks, selected, message, pull_error, lock_warning,
                         filter, filter_active, hide_done, cakes, repo_name, username, theme } = p;
    let area = f.area();
    let [top_row, _gap, panel_area, hint_area] = Layout::vertical([
        Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1), Constraint::Length(2),
    ]).areas(area);
    render_top_bar(f, top_row, active, repo_name, username, None, theme);
    let view_bg = match active {
        ActiveView::Personal => theme.bg_personal,
        ActiveView::Backlog  => theme.bg_backlog,
        ActiveView::Planner  => theme.bg_planner,
    };
    let block = list_view_block(active, filter, filter_active, pull_error, lock_warning, theme)
        .style(Style::new().bg(view_bg));
    let inner = block.inner(panel_area);
    f.render_widget(block, panel_area);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Fill(1), Constraint::Length(1), Constraint::Length(1),
    ]).split(inner);
    let title_col_width = inner.width.saturating_sub(30) as usize;
    let separate_done = active == ActiveView::Planner;
    let (items, index_map) = build_tree_items(cakes, tasks, selected, filter, title_col_width, hide_done, separate_done, theme);
    let empty_msg = if filter.is_empty() { "No tasks yet. ^C to create one." } else { "No tasks match the filter." };
    if items.is_empty() {
        f.render_widget(Paragraph::new(empty_msg).alignment(Alignment::Center).style(theme.dim_style()), rows[0]);
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
    render_flash_row(f, rows[2], message, theme);
    f.render_widget(Paragraph::new(list_view_hint_bar(active, hint_area.width, theme)), hint_area);
}

fn list_view_block(_active: ActiveView, filter: &str, filter_active: bool, pull_error: Option<&str>, lock_warning: Option<&str>, theme: &Theme) -> Block<'static> {
    let mut block = panel("TASKS", theme).padding(Padding::new(1, 1, 1, 1));
    if !filter.is_empty() && !filter_active {
        block = block.title_top(Line::from(vec![
            Span::styled(format!(" /{filter} "), theme.dim_style()),
            Span::styled(" Q: clear ", theme.dim_style()),
        ]).right_aligned());
    }
    if let Some(err) = pull_error {
        block = block.title_bottom(Line::from(vec![
            Span::styled(format!(" ⚠ {err} "), Style::new().fg(theme.in_progress)),
            Span::styled(" Esc: dismiss ", theme.dim_style()),
        ]).left_aligned());
    }
    if let Some(warn) = lock_warning {
        block = block.title_bottom(
            Line::from(Span::styled(format!(" ⚠ {warn} "), Style::new().fg(theme.in_progress))).right_aligned()
        );
    }
    block
}

fn filter_line_widget<'a>(filter: &'a str, filter_active: bool, theme: &Theme) -> Paragraph<'a> {
    let dim = theme.dim_style();
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
            Span::styled("  Q: clear", dim),
        ]))
    } else {
        Paragraph::new(Line::from(vec![Span::raw("  "), Span::styled("Q or / to filter", dim)]))
    }
}

fn render_flash_row(f: &mut Frame, area: Rect, message: Option<&str>, theme: &Theme) {
    let line = match message {
        Some(msg) => Line::from(vec![Span::raw("  "), Span::styled(msg.to_string(), theme.dim_style())]),
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
        return (theme.sym_blocked.clone(), Style::new().fg(theme.flag).add_modifier(Modifier::BOLD));
    }
    match task.priority {
        Priority::Urgent => (theme.sym_urgent.clone(), Style::new().fg(theme.flag).add_modifier(Modifier::BOLD)),
        Priority::High   => (theme.sym_high.clone(),   Style::new().fg(theme.priority_color).add_modifier(Modifier::BOLD)),
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
        TaskStatus::Done       => Style::new().fg(theme.done_color).add_modifier(Modifier::DIM),
        TaskStatus::InProgress => Style::new().add_modifier(Modifier::BOLD),
        TaskStatus::Open       => Style::new(),
    };
    let cursor_sty = if is_sel {
        Style::new().add_modifier(Modifier::BOLD).fg(theme.accent)
    } else {
        Style::new().fg(theme.dim)
    };
    let (flag_str, flag_sty) = task_flag(task, theme);
    let (status_sym, status_sty) = match task.status {
        TaskStatus::InProgress => (theme.sym_progress.clone(), Style::new().add_modifier(Modifier::BOLD).fg(theme.in_progress)),
        TaskStatus::Open       => (theme.sym_open.clone(),     Style::new().fg(theme.open_color)),
        TaskStatus::Done       => (theme.sym_done.clone(),     Style::new().add_modifier(Modifier::DIM).fg(theme.done_color)),
    };
    let cursor_char = if is_sel { theme.cursor.clone() } else { " ".to_string() };
    let mut title_spans: Vec<Span<'static>> = Vec::new();
    if !prefix.is_empty() {
        title_spans.push(Span::styled(prefix.to_string(), Style::new().fg(theme.faint)));
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

#[allow(clippy::too_many_arguments)]
fn build_tree_items(
    cakes: &[Cake],
    tasks: &[(String, Task)],
    selected: usize,
    filter: &str,
    title_col_width: usize,
    hide_done: bool,
    separate_done: bool,
    theme: &Theme,
) -> (Vec<Row<'static>>, Vec<Option<usize>>) {
    use crate::app::filter_matches_with_cakes;
    let f = if filter.is_empty() { String::new() } else { filter.to_lowercase() };
    let is_vis = |t: &Task| (f.is_empty() || filter_matches_with_cakes(t, cakes, &f))
                           && (!(hide_done || separate_done) || t.status != TaskStatus::Done);
    let mut rows: Vec<Row<'static>> = Vec::new();
    let mut index_map: Vec<Option<usize>> = Vec::new();
    let mut vis_idx = 0usize;
    let mut first_group = true;
    let cake_hdr_sty = Style::new().add_modifier(Modifier::BOLD).fg(theme.accent);
    let rule_sty     = Style::new().fg(theme.faint);
    let make_header  = |title: String, progress: String| -> Row<'static> {
        let rule_len = title_col_width
            .saturating_sub(UnicodeWidthStr::width(title.as_str()) + UnicodeWidthStr::width(progress.as_str()) + 1);
        let hdr_line = Line::from(vec![
            Span::styled(title,                cake_hdr_sty),
            Span::styled(progress,             Style::new().fg(theme.dim)),
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
        first_group = false;
    }
    if separate_done && !hide_done {
        let done_vis: Vec<(&str, &Task)> = tasks.iter()
            .filter(|(_, t)| (f.is_empty() || filter_matches_with_cakes(t, cakes, &f))
                             && t.status == TaskStatus::Done)
            .map(|(o, t)| (o.as_str(), t))
            .collect();
        if !done_vis.is_empty() {
            if !first_group { rows.push(Row::new(vec![""; 6])); index_map.push(None); }
            rows.push(make_header("DONE".to_string(), String::new()));
            index_map.push(None);
            render_tree_rows(&mut rows, &mut index_map, &mut vis_idx, selected, &done_vis, theme);
        }
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
            .is_none_or(|pid| !group.iter().any(|(_, pt)| pt.id == *pid)))
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

// ── detail two-pane ──────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn draw_detail(f: &mut Frame, context: TaskContext, task: &Task, siblings: &[Task], _message: Option<&str>, selected: DetailField, cakes: &[Cake], from_planner: bool, repo_name: &str, username: &str, git_ahead: Option<&(u32, String)>, theme: &Theme) {
    let area = f.area();
    let [top_row, _gap, body, footer] = Layout::vertical([
        Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1), Constraint::Length(2),
    ]).areas(area);
    let (crumb_label, active_tab) = if from_planner { ("PLANNER", ActiveView::Planner) }
        else if context == TaskContext::Backlog { ("BACKLOG", ActiveView::Backlog) }
        else { ("PERSONAL", ActiveView::Personal) };
    let crumb = format!("{crumb_label} › {}", task.id);
    render_top_bar(f, top_row, active_tab, repo_name, username, Some(&crumb), theme);
    let [left, right] = Layout::horizontal([Constraint::Length(34), Constraint::Fill(1)]).areas(body);
    let sib_h = if siblings.is_empty() { 5u16 } else { (siblings.len() as u16 + 4).min(12) };
    let [prop_area, sib_area] = Layout::vertical([Constraint::Fill(1), Constraint::Length(sib_h)]).areas(left);
    let prop_block = panel("SLICE PROPERTIES", theme).padding(Padding::new(1, 1, 1, 1));
    let prop_inner = prop_block.inner(prop_area);
    f.render_widget(prop_block, prop_area);
    draw_detail_properties(f, prop_inner, task, selected, cakes, git_ahead, theme);
    let sib_title = if siblings.is_empty() { "STANDALONE" } else { "CAKE SLICES" };
    let sib_block = panel(sib_title, theme).padding(Padding::new(1, 1, 1, 1));
    let sib_inner = sib_block.inner(sib_area);
    f.render_widget(sib_block, sib_area);
    draw_detail_siblings(f, sib_inner, task, siblings, theme);
    let content_block = panel("CONTENT", theme).padding(Padding::new(1, 1, 1, 1));
    let content_inner = content_block.inner(right);
    f.render_widget(content_block, right);
    draw_detail_content(f, content_inner, task, cakes, theme);
    f.render_widget(Paragraph::new(detail_nav_bar(footer.width, theme)), footer);
}

fn draw_detail_properties(f: &mut Frame, area: Rect, task: &Task, focused: DetailField, cakes: &[Cake], git_ahead: Option<&(u32, String)>, theme: &Theme) {
    let cake_name = task.cake_id.as_deref()
        .and_then(|id| cakes.iter().find(|c| c.id == id))
        .map(|c| c.title.clone())
        .unwrap_or_else(|| "none".to_string());
    let fields: &[(DetailField, &str, &str)] = &[
        (DetailField::Type,      "1", "TYPE"),
        (DetailField::Status,    "2", "STATUS"),
        (DetailField::Priority,  "3", "PRIORITY"),
        (DetailField::AiFlagged, "4", "AI FLAG"),
        (DetailField::Cake,      "5", "CAKE"),
        (DetailField::Assign,    "6", "ASSIGN"),
    ];
    let mut y = area.y;
    for (field, badge, label) in fields {
        if y >= area.y + area.height { break; }
        let is_focused = *field == focused;
        let value = detail_field_value(*field, task, &cake_name);
        let val_sty = detail_field_value_style(*field, task, theme);
        let bg = if is_focused { Style::new().bg(theme.sel_bg) } else { Style::new() };
        let lbl_sty = if is_focused {
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(theme.fg).add_modifier(Modifier::BOLD)
        };
        let caret = if is_focused { " ◀" } else { "   " };
        let row = Rect { x: area.x, y, width: area.width, height: 1 };
        let val_max = area.width.saturating_sub(15) as usize;
        f.render_widget(Paragraph::new(
            Line::from(vec![
                Span::styled(format!("[{badge}] "), theme.dim_style()),
                Span::styled(format!("{label:<9}"), lbl_sty),
                Span::styled(truncate_title(&value, val_max), val_sty),
                Span::styled(caret.to_string(), Style::new().fg(theme.accent)),
            ]).style(bg)
        ), row);
        y += 1;
        if is_focused && y < area.y + area.height {
            if let Some(opts) = detail_field_options(*field) {
                let ribbon_row = Rect { x: area.x, y, width: area.width, height: 1 };
                detail_ribbon(f, ribbon_row, opts, &value, theme);
                y += 1;
            }
        }
    }
    // read-only metadata rows
    let meta: &[(&str, String)] = &[
        ("FILE",    format!("{}/{}.md", type_label_plural(&task.task_type), task.id)),
        ("CREATED", task.created.format("%Y-%m-%d").to_string()),
    ];
    for (label, value) in meta {
        if y >= area.y + area.height { break; }
        let val_max = area.width.saturating_sub(12) as usize;
        f.render_widget(Paragraph::new(Line::from(vec![
            Span::styled(format!("    {label:<7}"), theme.dim_style()),
            Span::styled(truncate_title(value, val_max), theme.dim_style()),
        ])), Rect { x: area.x, y, width: area.width, height: 1 });
        y += 1;
    }
    let _ = y;
    // git status pinned to panel bottom
    if let Some((ahead, last_push)) = git_ahead {
        let bottom_y = area.y + area.height - 1;
        let line = if *ahead == 0 {
            Line::from(vec![
                Span::styled(format!("  push {last_push}"), theme.dim_style()),
            ])
        } else {
            Line::from(vec![
                Span::styled(format!("  {} ahead", ahead), Style::new().fg(theme.in_progress)),
                Span::styled(format!(" · push {last_push}"), theme.dim_style()),
            ])
        };
        f.render_widget(Paragraph::new(line), Rect { x: area.x, y: bottom_y, width: area.width, height: 1 });
    }
}

fn status_sort_key(s: &TaskStatus) -> u8 {
    match s { TaskStatus::InProgress => 0, TaskStatus::Open => 1, TaskStatus::Done => 2 }
}

fn priority_sort_key(p: &Priority) -> u8 {
    match p { Priority::Urgent => 0, Priority::High => 1, Priority::Normal => 2 }
}

fn draw_detail_siblings(f: &mut Frame, area: Rect, task: &Task, siblings: &[Task], theme: &Theme) {
    if siblings.is_empty() {
        f.render_widget(Paragraph::new(Span::styled(
            "Not part of a Cake".to_string(), theme.dim_style(),
        )), area);
        return;
    }
    let mut sorted: Vec<&Task> = siblings.iter().collect();
    sorted.sort_by_key(|t| (status_sort_key(&t.status), priority_sort_key(&t.priority)));
    let mut y = area.y;
    for sib in sorted {
        if y >= area.y + area.height { break; }
        let is_current = sib.id == task.id;
        let sym = match sib.status {
            TaskStatus::InProgress => Span::styled(format!("{} ", theme.sym_progress), Style::new().fg(theme.in_progress)),
            TaskStatus::Done       => Span::styled(format!("{} ", theme.sym_done),     Style::new().fg(theme.done_color).add_modifier(Modifier::DIM)),
            TaskStatus::Open       => Span::styled(format!("{} ", theme.sym_open),     Style::new().fg(theme.open_color)),
        };
        let prio = match sib.priority {
            Priority::Urgent => Span::styled(format!("{} ", theme.sym_urgent), Style::new().fg(theme.flag).add_modifier(Modifier::BOLD)),
            Priority::High   => Span::styled(format!("{} ", theme.sym_high),   Style::new().fg(theme.priority_color)),
            Priority::Normal => Span::raw(""),
        };
        let title_max = area.width.saturating_sub(5) as usize;
        let (title_sty, row_bg) = if is_current {
            (Style::new().fg(theme.fg).add_modifier(Modifier::BOLD), Style::new().bg(theme.sel_bg))
        } else {
            (theme.dim_style(), Style::new())
        };
        f.render_widget(Paragraph::new(
            Line::from(vec![
                sym, prio,
                Span::styled(truncate_title(&sib.title, title_max), title_sty),
            ]).style(row_bg)
        ), Rect { x: area.x, y, width: area.width, height: 1 });
        y += 1;
    }
}

fn detail_field_value(field: DetailField, task: &Task, cake_name: &str) -> String {
    match field {
        DetailField::Type      => type_label(&task.task_type).to_string(),
        DetailField::Status    => match task.status {
            TaskStatus::Open       => "open",
            TaskStatus::InProgress => "in-progress",
            TaskStatus::Done       => "done",
        }.to_string(),
        DetailField::Priority  => priority_label(&task.priority).to_string(),
        DetailField::AiFlagged => if task.ai_flagged { "yes" } else { "no" }.to_string(),
        DetailField::Cake      => cake_name.to_string(),
        DetailField::Assign    => task.owner.clone().unwrap_or_else(|| "none".to_string()),
    }
}

fn detail_field_value_style(field: DetailField, task: &Task, theme: &Theme) -> Style {
    match field {
        DetailField::Status => match task.status {
            TaskStatus::InProgress => Style::new().fg(theme.in_progress).add_modifier(Modifier::BOLD),
            TaskStatus::Done       => Style::new().fg(theme.done_color),
            TaskStatus::Open       => Style::new().fg(theme.open_color),
        },
        DetailField::Priority => match task.priority {
            Priority::Urgent => Style::new().fg(theme.flag).add_modifier(Modifier::BOLD),
            Priority::High   => Style::new().fg(theme.priority_color),
            Priority::Normal => theme.dim_style(),
        },
        DetailField::AiFlagged => if task.ai_flagged { Style::new().fg(theme.flag) } else { theme.dim_style() },
        _                      => theme.dim_style(),
    }
}

fn detail_field_options(field: DetailField) -> Option<&'static [&'static str]> {
    match field {
        DetailField::Type      => Some(&["task", "bug", "incident"]),
        DetailField::Status    => Some(&["open", "in-progress", "done"]),
        DetailField::Priority  => Some(&["normal", "high", "urgent"]),
        DetailField::AiFlagged => Some(&["no", "yes"]),
        _                      => None,
    }
}

fn detail_ribbon(f: &mut Frame, area: Rect, opts: &[&str], current: &str, theme: &Theme) {
    let mut spans = vec![Span::styled("   ↳ ", theme.dim_style())];
    for (i, opt) in opts.iter().enumerate() {
        if i > 0 { spans.push(Span::styled("  ·  ", theme.dim_style())); }
        if *opt == current {
            spans.push(Span::styled(opt.to_string(), Style::new().fg(theme.accent).add_modifier(Modifier::BOLD).bg(theme.sel_bg)));
        } else {
            spans.push(Span::styled(opt.to_string(), theme.dim_style()));
        }
    }
    f.render_widget(Paragraph::new(Line::from(spans)).style(Style::new().bg(theme.sel_bg)), area);
}

fn draw_detail_content(f: &mut Frame, area: Rect, task: &Task, cakes: &[Cake], theme: &Theme) {
    let cake_name = task.cake_id.as_deref()
        .and_then(|id| cakes.iter().find(|c| c.id == id))
        .map(|c| c.title.as_str()).unwrap_or("standalone");
    let mut lines: Vec<Line<'static>> = Vec::new();
    let created_str = format!("baked {}", task.created.format("%Y-%m-%d"));
    let left_part = format!("{}  ▸  slice · #{}", cake_name, task.id);
    let gap = (area.width as usize).saturating_sub(left_part.chars().count() + created_str.len());
    lines.push(Line::from(vec![
        Span::styled(cake_name.to_string(), Style::new().fg(theme.accent)),
        Span::styled("  ▸  slice · #".to_string(), theme.dim_style()),
        Span::styled(task.id.clone(), theme.dim_style()),
        Span::styled(" ".repeat(gap), Style::new()),
        Span::styled(created_str, theme.dim_style()),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(task.title.clone(), Style::new().fg(theme.fg).add_modifier(Modifier::BOLD))));
    lines.push(Line::from(""));
    if !task.bites.is_empty() {
        let done_b = task.bites.iter().filter(|b| b.done).count();
        let tot_b  = task.bites.len();
        lines.push(Line::from(vec![
            Span::styled("BITES".to_string(), Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  {done_b}/{tot_b}  "), theme.dim_style()),
            Span::styled("─".repeat(14), Style::new().fg(theme.faint)),
        ]));
        for (i, bite) in task.bites.iter().enumerate() {
            let conn = if i + 1 == tot_b { "└─" } else { "├─" };
            let (sym, sty, txt) = if bite.done {
                (&theme.sym_done, Style::new().fg(theme.done_color), Style::new().fg(theme.done_color).add_modifier(Modifier::DIM))
            } else {
                (&theme.sym_open, Style::new().fg(theme.open_color), Style::new())
            };
            lines.push(Line::from(vec![
                Span::styled(conn.to_string(), Style::new().fg(theme.faint)),
                Span::styled(sym.clone(), sty),
                Span::styled(format!(" {}", bite.text), txt),
            ]));
        }
        lines.push(Line::from(""));
    }
    let desc_str = task.description.as_deref().unwrap_or("");
    let char_count = desc_str.chars().count();
    let file_path = format!("{}/{}.md", type_label_plural(&task.task_type), task.id);
    let count_sty = if char_count > 850 { Style::new().fg(theme.flag).add_modifier(Modifier::BOLD) }
        else if char_count > 595 { Style::new().fg(theme.in_progress) }
        else { theme.dim_style() };
    lines.push(Line::from(vec![
        Span::styled("BODY".to_string(), Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled(format!("  {file_path}  "), theme.dim_style()),
        Span::styled(format!("{char_count}/850"), count_sty),
        Span::styled("  ─────", Style::new().fg(theme.faint)),
    ]));
    if desc_str.is_empty() {
        lines.push(Line::from(Span::styled("  (empty — E to edit)", theme.dim_style())));
    } else {
        let prose: Vec<&str> = desc_str.lines()
            .filter(|l| !l.starts_with("!bite ") && !l.starts_with("!!bite ")
                     && !l.starts_with("!crumb ") && !l.starts_with("!!crumb "))
            .collect();
        if prose.is_empty() {
            lines.push(Line::from(Span::styled("  (bites only — E to edit body)", theme.dim_style())));
        } else {
            for l in prose {
                lines.push(desc_line_render(l, theme));
            }
        }
    }
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
        Line::from(vec![Span::styled(format!("  {} ", theme.sym_done), Style::new().fg(theme.done_color).add_modifier(Modifier::DIM)), Span::styled(t.to_string(), Style::new().fg(theme.done_color).add_modifier(Modifier::DIM))])
    } else if let Some(t) = line.strip_prefix("!bite ") {
        Line::from(vec![Span::styled(format!("  {} ", theme.sym_open), Style::new().fg(theme.open_color)), Span::styled(t.to_string(), Style::new())])
    } else if let Some(t) = line.strip_prefix("!!crumb ").or_else(|| line.strip_prefix("!crumb ")) {
        Line::from(vec![Span::styled(format!("  {} ", theme.sym_dot), theme.dim_style()), Span::styled(t.to_string(), theme.dim_style())])
    } else {
        Line::from(vec![Span::raw("  "), Span::styled(line.to_string(), theme.dim_style())])
    }
}

fn type_label_plural(t: &TaskType) -> &'static str {
    match t { TaskType::Task => "tasks", TaskType::Bug => "bugs", TaskType::Incident => "incidents" }
}

// ── create two-pane ───────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn draw_create(f: &mut Frame, focus: CreateFocus, title: &Input, task_type: &TaskType, priority: &Priority, users: &[String], user_filter: &str, user_sel: usize, cakes: &[Cake], cake_filter: &str, cake_sel: usize, theme: &Theme, repo_name: &str, username: &str) {
    let area = f.area();
    let [top_row, _gap, body, footer] = Layout::vertical([
        Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1), Constraint::Length(2),
    ]).areas(area);
    render_top_bar(f, top_row, ActiveView::Personal, repo_name, username, Some("NEW SLICE"), theme);
    let [left, right] = Layout::horizontal([Constraint::Length(34), Constraint::Fill(1)]).areas(body);
    let prop_block = panel("PROPERTIES", theme).padding(Padding::new(1, 1, 1, 1));
    let prop_inner = prop_block.inner(left);
    f.render_widget(prop_block, left);
    draw_create_properties(f, prop_inner, focus, title, task_type, priority, users, user_filter, user_sel, cakes, cake_filter, cake_sel, theme);
    let preview_block = panel("PREVIEW", theme).padding(Padding::new(1, 1, 1, 1));
    let preview_inner = preview_block.inner(right);
    f.render_widget(preview_block, right);
    draw_create_preview(f, preview_inner, title.value(), task_type, priority, cakes, cake_filter, cake_sel, theme);
    f.render_widget(Paragraph::new(create_hint_bar_text(footer.width, theme)), footer);
}

#[allow(clippy::too_many_arguments)]
fn draw_create_properties(f: &mut Frame, area: Rect, focus: CreateFocus, title: &Input, task_type: &TaskType, priority: &Priority, users: &[String], user_filter: &str, user_sel: usize, cakes: &[Cake], cake_filter: &str, cake_sel: usize, theme: &Theme) {
    let mut y = area.y;
    // ── Title row ────────────────────────────────────────────────────────────
    if y < area.y + area.height {
        let title_active = focus == CreateFocus::Title;
        let row = Rect { x: area.x, y, width: area.width, height: 1 };
        let lbl_sty = if title_active {
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(theme.fg).add_modifier(Modifier::BOLD)
        };
        let title_val = title.value();
        let max_w = area.width.saturating_sub(9) as usize;
        let display = truncate_title(title_val, max_w);
        let cursor_ch = if title_active { "│" } else { "" };
        f.render_widget(Paragraph::new(Line::from(vec![
            Span::styled("TITLE    ", lbl_sty),
            Span::styled(display.to_string(), Style::new().fg(theme.fg)),
            Span::styled(cursor_ch.to_string(), Style::new().fg(theme.accent).add_modifier(Modifier::RAPID_BLINK)),
        ])).style(if title_active { Style::new().bg(theme.sel_bg) } else { Style::new() }), row);
        if title_active {
            let cursor_x = area.x + 9 + title.visual_cursor() as u16;
            let cursor_x = cursor_x.min(area.x + area.width.saturating_sub(1));
            f.set_cursor_position((cursor_x, y));
        }
        y += 1;
    }
    if y < area.y + area.height {
        f.render_widget(Paragraph::new(Span::styled(
            "─".repeat(area.width as usize), Style::new().fg(theme.faint),
        )), Rect { x: area.x, y, width: area.width, height: 1 });
        y += 1;
    }
    // ── Type field ───────────────────────────────────────────────────────────
    let type_focused = focus == CreateFocus::Title; // type has no focus state; always show inline
    if y < area.y + area.height {
        let cur_type = type_label(task_type);
        let row = Rect { x: area.x, y, width: area.width, height: 1 };
        f.render_widget(Paragraph::new(Line::from(vec![
            Span::styled("[1] ", theme.dim_style()),
            Span::styled(format!("{:<9}", "TYPE"), Style::new().fg(theme.fg).add_modifier(Modifier::BOLD)),
            Span::styled(format!("[{cur_type}]"), Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)),
        ])), row);
        y += 1;
        let _ = type_focused;
    }
    // ── Priority field ───────────────────────────────────────────────────────
    if y < area.y + area.height {
        let cur_prio = priority_label(priority);
        let prio_sty = match priority {
            Priority::Urgent => Style::new().fg(theme.flag).add_modifier(Modifier::BOLD),
            Priority::High   => Style::new().fg(theme.priority_color).add_modifier(Modifier::BOLD),
            Priority::Normal => Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
        };
        f.render_widget(Paragraph::new(Line::from(vec![
            Span::styled("[2] ", theme.dim_style()),
            Span::styled(format!("{:<9}", "PRIORITY"), Style::new().fg(theme.fg).add_modifier(Modifier::BOLD)),
            Span::styled(format!("[{cur_prio}]"), prio_sty),
        ])), Rect { x: area.x, y, width: area.width, height: 1 });
        y += 1;
    }
    // ── Assign field ─────────────────────────────────────────────────────────
    if y < area.y + area.height {
        let assign_active = focus == CreateFocus::Assignee;
        let f_lower = user_filter.to_lowercase();
        let filtered_users: Vec<&str> = users.iter()
            .filter(|u| f_lower.is_empty() || u.to_lowercase().contains(&f_lower))
            .map(|s| s.as_str()).collect();
        let assign_val = filtered_users.get(user_sel).copied().unwrap_or("none");
        let row_bg = if assign_active { Style::new().bg(theme.sel_bg) } else { Style::new() };
        let lbl_sty = if assign_active {
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(theme.fg).add_modifier(Modifier::BOLD)
        };
        if assign_active {
            f.render_widget(Paragraph::new(Line::from(vec![
                Span::styled("[3] ", theme.dim_style()),
                Span::styled(format!("{:<9}", "ASSIGN"), lbl_sty),
                Span::styled(user_filter.to_string(), Style::new()),
                Span::styled("│", Style::new().fg(theme.accent)),
            ])).style(row_bg), Rect { x: area.x, y, width: area.width, height: 1 });
            let cursor_x = (area.x + 13 + user_filter.len() as u16).min(area.x + area.width.saturating_sub(1));
            f.set_cursor_position((cursor_x, y));
            y += 1;
            let list_rows = (area.y + area.height).saturating_sub(y).min(5) as usize;
            for i in 0..list_rows {
                if y >= area.y + area.height { break; }
                if let Some(name) = filtered_users.get(i) {
                    let is_sel = i == user_sel;
                    let (cur, sty) = if is_sel {
                        (format!("{} ", theme.cursor), Style::new().fg(theme.accent))
                    } else {
                        ("  ".to_string(), theme.dim_style())
                    };
                    let bg = if is_sel { Style::new().bg(theme.sel_bg) } else { Style::new() };
                    f.render_widget(Paragraph::new(Line::from(vec![
                        Span::raw("    "),
                        Span::styled(cur, sty),
                        Span::styled(name.to_string(), bg),
                    ])), Rect { x: area.x, y, width: area.width, height: 1 });
                    y += 1;
                }
            }
        } else {
            f.render_widget(Paragraph::new(Line::from(vec![
                Span::styled("[3] ", theme.dim_style()),
                Span::styled(format!("{:<9}", "ASSIGN"), lbl_sty),
                Span::styled(assign_val.to_string(), theme.dim_style()),
                Span::styled("   ▶", Style::new().fg(theme.faint)),
            ])), Rect { x: area.x, y, width: area.width, height: 1 });
            y += 1;
        }
    }
    // ── Cake field ───────────────────────────────────────────────────────────
    if y < area.y + area.height {
        let cake_active = focus == CreateFocus::Cake;
        let f_lower = cake_filter.to_lowercase();
        let none_vis = f_lower.is_empty() || "none".contains(&f_lower);
        let filtered_cakes: Vec<Option<&Cake>> = (if none_vis { vec![None] } else { vec![] })
            .into_iter()
            .chain(cakes.iter().filter(|c| f_lower.is_empty() || c.title.to_lowercase().contains(&f_lower)).map(Some))
            .collect();
        let cake_display = filtered_cakes.get(cake_sel)
            .map(|e| e.map(|c| c.title.as_str()).unwrap_or("none"))
            .unwrap_or("none");
        let lbl_sty = if cake_active {
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(theme.fg).add_modifier(Modifier::BOLD)
        };
        if cake_active {
            f.render_widget(Paragraph::new(Line::from(vec![
                Span::styled("[4] ", theme.dim_style()),
                Span::styled(format!("{:<9}", "CAKE"), lbl_sty),
                Span::styled(cake_filter.to_string(), Style::new()),
                Span::styled("│", Style::new().fg(theme.accent)),
            ])).style(Style::new().bg(theme.sel_bg)), Rect { x: area.x, y, width: area.width, height: 1 });
            let cursor_x = (area.x + 13 + cake_filter.len() as u16).min(area.x + area.width.saturating_sub(1));
            f.set_cursor_position((cursor_x, y));
            y += 1;
            let list_rows = (area.y + area.height).saturating_sub(y).min(5) as usize;
            for i in 0..list_rows {
                if y >= area.y + area.height { break; }
                if let Some(entry) = filtered_cakes.get(i) {
                    let name = entry.map(|c| c.title.as_str()).unwrap_or("None (default)");
                    let is_sel = i == cake_sel;
                    let (cur, sty) = if is_sel {
                        (format!("{} ", theme.cursor), Style::new().fg(theme.accent))
                    } else {
                        ("  ".to_string(), theme.dim_style())
                    };
                    let bg = if is_sel { Style::new().bg(theme.sel_bg) } else { Style::new() };
                    f.render_widget(Paragraph::new(Line::from(vec![
                        Span::raw("    "),
                        Span::styled(cur, sty),
                        Span::styled(name.to_string(), bg),
                    ])), Rect { x: area.x, y, width: area.width, height: 1 });
                    y += 1;
                }
            }
        } else {
            f.render_widget(Paragraph::new(Line::from(vec![
                Span::styled("[4] ", theme.dim_style()),
                Span::styled(format!("{:<9}", "CAKE"), lbl_sty),
                Span::styled(cake_display.to_string(), theme.dim_style()),
                Span::styled("   ▶", Style::new().fg(theme.faint)),
            ])), Rect { x: area.x, y, width: area.width, height: 1 });
            y += 1;
        }
    }
    // ── Progress counter + bar ───────────────────────────────────────────────
    {
        let uf_l = user_filter.to_lowercase();
        let filt_u: Vec<&str> = users.iter()
            .filter(|u| uf_l.is_empty() || u.to_lowercase().contains(&uf_l))
            .map(|s| s.as_str()).collect();
        let assign_is_set = filt_u.get(user_sel).is_some();
        let cf_l = cake_filter.to_lowercase();
        let none_vis_p = cf_l.is_empty() || "none".contains(&cf_l);
        let cake_is_set = if none_vis_p { cake_sel != 0 } else { true };
        let set_count: usize = 2 + if assign_is_set { 1 } else { 0 } + if cake_is_set { 1 } else { 0 };
        if y < area.y + area.height { y += 1; } // spacer
        if y < area.y + area.height {
            f.render_widget(Paragraph::new(Line::from(
                Span::styled(format!("{set_count} of 4 set"), theme.dim_style()),
            )), Rect { x: area.x, y, width: area.width, height: 1 });
            y += 1;
        }
        if y < area.y + area.height {
            let bar_w = area.width as usize;
            let filled = (bar_w * set_count + 2) / 4;
            let spans = vec![
                Span::styled(" ".repeat(filled),                       Style::new().bg(theme.accent)),
                Span::styled(" ".repeat(bar_w.saturating_sub(filled)), Style::new().bg(theme.faint)),
            ];
            f.render_widget(Paragraph::new(Line::from(spans)), Rect { x: area.x, y, width: area.width, height: 1 });
            y += 1;
        }
    }
    let _ = y;
}

#[allow(clippy::too_many_arguments)]
fn draw_create_preview(f: &mut Frame, area: Rect, title_val: &str, task_type: &TaskType, priority: &Priority, cakes: &[Cake], cake_filter: &str, cake_sel: usize, theme: &Theme) {
    let f_lower = cake_filter.to_lowercase();
    let none_vis = f_lower.is_empty() || "none".contains(&f_lower);
    let filtered_cakes: Vec<Option<&Cake>> = (if none_vis { vec![None] } else { vec![] })
        .into_iter()
        .chain(cakes.iter().filter(|c| f_lower.is_empty() || c.title.to_lowercase().contains(&f_lower)).map(Some))
        .collect();
    let selected_cake = filtered_cakes.get(cake_sel).and_then(|e| *e);
    let type_str = type_label_plural(task_type);
    let file_path = format!("{type_str}/<hash>.md");
    let display_title = if title_val.is_empty() { "← type title".to_string() } else { title_val.to_string() };
    let title_sty = if title_val.is_empty() { theme.dim_style() } else { Style::new().fg(theme.fg).add_modifier(Modifier::BOLD) };
    let (prio_text, prio_sty) = match priority {
        Priority::Urgent => (format!("{} ", theme.sym_urgent), Style::new().fg(theme.flag).add_modifier(Modifier::BOLD)),
        Priority::High   => (format!("{} ", theme.sym_high),   Style::new().fg(theme.priority_color)),
        Priority::Normal => (String::new(), Style::new()),
    };
    let type_ch = type_char(task_type);
    let open_sym = theme.sym_open.clone();
    let rule = "─".repeat(area.width as usize);
    let mut lines: Vec<Line<'static>> = vec![
        Line::from(Span::styled("how it lands in the list", theme.dim_style())),
        Line::from(""),
        Line::from(Span::styled(rule.clone(), Style::new().fg(theme.faint))),
    ];
    if let Some(cake) = selected_cake {
        let cake_name = cake.title.clone();
        let dashes = "─".repeat(area.width.saturating_sub(cake_name.len() as u16 + 1) as usize);
        lines.push(Line::from(vec![
            Span::styled(cake_name, Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" {dashes}"), Style::new().fg(theme.faint)),
        ]));
    }
    let indent = if selected_cake.is_some() { "└── " } else { "" };
    let mut row_spans: Vec<Span<'static>> = vec![Span::raw(indent.to_string())];
    if !prio_text.is_empty() {
        row_spans.push(Span::styled(prio_text, prio_sty));
    }
    row_spans.push(Span::styled(format!("{open_sym} {type_ch} "), theme.dim_style()));
    row_spans.push(Span::styled(display_title, title_sty));
    row_spans.push(Span::styled("█", Style::new().fg(theme.accent)));
    lines.push(Line::from(row_spans));
    lines.push(Line::from(Span::styled(rule, Style::new().fg(theme.faint))));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(format!("{:<8}", "WRITES"), Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled(file_path, theme.dim_style()),
    ]));
    lines.push(Line::from(vec![
        Span::styled(format!("{:<8}", "BODY"), Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("$EDITOR on create  ·  E inline".to_string(), theme.dim_style()),
    ]));
    f.render_widget(Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false }), area);
}

fn draw_create_cake(f: &mut Frame, title: &str, theme: &Theme) {
    let area = f.area();
    let block = panel("New Cake", theme).padding(Padding::new(1, 1, 1, 1));
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
        Span::styled("  Enter: create  ", theme.dim_style()),
        Span::styled("  Esc: cancel",     theme.dim_style()),
    ])), rows[3]);
}

// ── pickers ───────────────────────────────────────────────────────────────────

fn draw_pick_cake(f: &mut Frame, cakes: &[Cake], selected: usize, filter: &str, theme: &Theme) {
    let items: Vec<String> = std::iter::once("none".to_string())
        .chain(cakes.iter().map(|c| c.title.clone()))
        .filter(|t| filter.is_empty() || t.to_lowercase().contains(&filter.to_lowercase()))
        .collect();
    draw_user_picker(f, "Pick Cake", &items, selected, filter, theme);
}

fn draw_assign_task(f: &mut Frame, users: &[String], selected: usize, filter: &str, theme: &Theme) {
    draw_user_picker(f, "Assign To", users, selected, filter, theme);
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
        Span::styled("/ ", theme.dim_style()),
        Span::styled(filter, theme.bold_style()),
        Span::styled("_", Style::new().add_modifier(Modifier::SLOW_BLINK)),
    ])), rows[0]);
    if filtered.is_empty() {
        f.render_widget(Paragraph::new("No matching users.").style(theme.dim_style()), rows[2]);
    } else {
        let items = user_picker_items(&filtered, selected, theme);
        let mut state = ListState::default();
        state.select(Some(selected));
        f.render_stateful_widget(List::new(items), rows[2], &mut state);
    }
    f.render_widget(Paragraph::new("↑↓: navigate  Type to filter  Enter: select  Esc: cancel").style(theme.dim_style()), rows[3]);
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
    f.render_widget(Paragraph::new(what).style(theme.dim_style()), rows[0]);
    f.render_widget(Paragraph::new("Continue? [y/N]"), rows[1]);
    f.render_widget(Paragraph::new("y: push  Enter/n/q: cancel").style(theme.dim_style()), rows[3]);
}

fn draw_delete_confirm(f: &mut Frame, task_title: &str, theme: &Theme) {
    let inner = render_popup(f, "Delete", 60, 8, theme);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Min(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Length(1),
    ]).split(inner);
    let question_line = Line::from(vec![
        Span::raw("Delete \""), Span::styled(task_title, theme.bold_style()), Span::raw("\"?"),
    ]);
    f.render_widget(Paragraph::new(question_line).wrap(ratatui::widgets::Wrap { trim: true }), rows[0]);
    f.render_widget(Paragraph::new("This action cannot be undone.").style(theme.dim_style()), rows[2]);
    f.render_widget(Paragraph::new("[y/N]"), rows[4]);
}

fn draw_push_prompt(f: &mut Frame, theme: &Theme) {
    let inner = render_popup(f, "Quit gitcake", 60, 7, theme);
    let rows = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1), Constraint::Length(2),
    ]).split(inner);
    f.render_widget(Paragraph::new("You have uncommitted task changes.").style(theme.dim_style()), rows[0]);
    f.render_widget(Paragraph::new("Push before quitting? [y/N]"), rows[1]);
    f.render_widget(Paragraph::new(theme.bar_text(&[
        ("y", "push + quit"), ("Enter/n", "quit"), ("Esc", "cancel"),
    ], rows[3].width)), rows[3]);
}

// ── command bars / top bar ────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum ActiveView { Personal, Planner, Backlog }

fn render_top_bar(f: &mut Frame, area: Rect, active: ActiveView, repo_name: &str, username: &str, breadcrumb: Option<&str>, theme: &Theme) {
    f.render_widget(Block::default().style(Style::new().bg(theme.bg)), area);
    let tabs = [(ActiveView::Personal, "PERSONAL"), (ActiveView::Planner, "PLANNER"), (ActiveView::Backlog, "BACKLOG")];
    let inactive_sty = Style::new().bg(theme.bg).fg(theme.dim);
    let active_sty   = Style::new().bg(theme.bg).fg(theme.accent).add_modifier(Modifier::BOLD);
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
        format!(" {crumb} ")
    } else {
        format!(" {repo_name} · {username} ")
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(right, Style::new().bg(theme.bg).fg(theme.dim)))).alignment(Alignment::Right),
        area,
    );
}

fn bar_make_line<'a>(items: &[(&'a str, &'a str)], chip: Style, _cap: Style, lbl: Style) -> Line<'a> {
    let mut spans = vec![Span::raw(" ")];
    for (key, label) in items {
        spans.push(Span::styled(format!(" {key} "), chip));
        spans.push(Span::styled(format!(" {label}  "), lbl));
    }
    Line::from(spans)
}

fn list_view_hint_bar(active: ActiveView, width: u16, theme: &Theme) -> Text<'static> {
    let cake_chip: &[(&str, &str)] = if active == ActiveView::Planner { &[("⇧C", "cake")] } else { &[] };
    let mut items: Vec<(&str, &str)> = vec![
        ("WS", "nav"), ("↵", "open"), ("^C", "create"),
    ];
    items.extend_from_slice(cake_chip);
    items.extend_from_slice(&[
        ("Q", "filter"), ("^R", "assign"), ("^B", "backlog"), ("^D", "delete"),
        ("⇧T", "pull"), ("^T", "push"), ("^Q", "quit"),
    ]);
    theme.bar_text(&items, width)
}

fn detail_nav_bar(width: u16, theme: &Theme) -> Text<'static> {
    theme.bar_text(&[
        ("A", "back"), ("WS", "nav"), ("1-6", "fields"), ("F", "cycle"), ("Spc", "status"),
        ("B", "bite"), ("E", "edit"), ("^R", "assign"),
        ("^T", "push"), ("^Q", "quit"),
    ], width)
}

fn create_hint_bar_text(width: u16, theme: &Theme) -> Text<'static> {
    theme.bar_text(&[
        ("1", "type"), ("2", "priority"), ("3", "assign"), ("4", "cake"),
        ("↵", "create"), ("^C", "editor"), ("Esc", "cancel"), ("^Q", "quit"),
    ], width)
}

// ── shared helpers ────────────────────────────────────────────────────────────

fn draw_error_line(f: &mut Frame, area: Rect, err: Option<&str>, theme: &Theme) {
    if let Some(e) = err {
        f.render_widget(
            Paragraph::new(format!("✗ {e}")).alignment(Alignment::Center).style(Style::new().fg(theme.flag)),
            area,
        );
    }
}

fn render_popup(f: &mut Frame, title: &str, percent_x: u16, height: u16, theme: &Theme) -> Rect {
    let area  = f.area();
    let popup = centered_rect(percent_x, height, area);
    f.render_widget(Clear, popup);
    let block = panel(title, theme).padding(Padding::new(1, 1, 1, 1));
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
