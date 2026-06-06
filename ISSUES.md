# gitcake — Known Issues & Improvements

---

## Resolved — Critical
- **Editor trap**: nano default, vi silent fallback, `$VISUAL`/`$EDITOR` respected first.
- **Silent auth failures**: pull errors stored in `App.pull_error`, classified (auth/network/other), shown on bottom border in yellow until Esc.
- **Bad task file kills list**: `collect_tasks()` skips unreadable files, returns filenames as warnings. Header shows "N slice(s) could not be read".
- **Terminal close bypasses push**: lock file at `~/.config/gitcake/{repo-key}.lock` (contains PID). Stale lock warns on next open: "Last session ended without pushing".
- **Git conflict**: `classify_push_error()` detects rejected/non-fast-forward pushes, shows pull-then-`^R`-retry message.

## Resolved — High
- **Cannot clear optional fields via patch**: `description`, `order`, `parent_id` in `TaskPatch` changed to `Option<Option<T>>` (`None`=keep, `Some(None)`=clear, `Some(Some(v))`=set). CLI `--parent ""` and `--description ""` now clear the field as documented. MCP `edit_slice` uses the same empty-string convention.

## Resolved — Medium
- **`cycle_status` full disk scan**: replaced `list_tasks()` call (all files) with `get_task(id)` (single file) when cycling status with F.
- **`yaml_str` missing escape sequences**: `\n`, `\r`, `\t` in titles/descriptions now escaped correctly; malformed YAML from CLI/MCP no longer possible.
- **Silent assignment failure in `create_via_editor`**: `let _ = assign_task(...)` replaced with a match that surfaces the error as a status message.
- **Cursor resets on every action**: `enter_task_list(preserve_id)` restores position by task ID after sort.
- **Non-ASCII usernames**: `get_username()` validates `[a-z0-9-]`, returns `UsernameInvalid` with instructions.
- **Long titles overflow**: `truncate_title()` uses `unicode-width`; budget = `inner_width − 22 − assignee_cols`.
- **No mid-session pull**: `Ctrl+P` pulls and reloads, cursor preserved, same error classifier as startup.
- **Small terminal garbles UI**: `draw()` checks `width < 60 || height < 20`, shows centred "Terminal too small" message.

- **Wrong DONE label**: `done_local` (not synced) vs `done_synced` (`is_completed`). Section headers include counts.
- **No team view**: `T` opens read-only view grouped by user folder (active tasks only).
- **No in-app repo change**: `Ctrl+O` opens setup screen with current path pre-filled; Esc returns.
- **Backlog assignee not validated**: `assign_backlog_task()` checks against known user folders.
- **No scroll memory from detail**: fixed by cursor-preservation — `handle_detail` back passes task ID.
- **No repo/user in header**: title shows `gitcake · {repo} · {username}` (or `· BACKLOG`).
- **No task count per group**: section headers show counts, e.g. `● IN PROGRESS (2)`.

## Resolved — Low
- **CODERULE 5 — `.unwrap()` in `list_team_tasks`**: changed to `.expect("owner is Some — filtered above")`.
- **CODERULE 5 — `.unwrap()` in `migrate_v1_to_v2`**: `file_name()` now returns `AppError::Parse` via `?` instead of panicking.
- **Push prompt shown on clean quit**: `try_quit` checks `has_local_changes()` first; clean repo exits immediately without prompting.
- **`process_running` Linux-only**: `#[cfg(target_os = "linux")]` gate added; non-Linux builds treat any existing lock as stale rather than silently misfiring.
- **No retry hint**: `classify_push_error()` appends `· ^R to retry` to all sync failures.
- **Backlog done lifecycle**: policy — no done lifecycle for backlog. `G` claims a slice into the personal folder, preserving its hex ID, sets status open, removes from backlog.
- **`launched_editor` dead code**: removed.
- **`render_help` used by 2 screens**: setup/init-repo now use `action_bar()`, `render_help` deleted.
- **Concurrent instances stale state**: documented as known limitation; lock file warns on next open.
- **Setup screen showed Q instead of ^Q**: label corrected to `^Q`.

## Resolved — Code Quality
- **O1 — KeyMap clone per keypress**: `handle_task_list` and `handle_detail` now borrow `&self.config.keys` instead of cloning; 9 String allocations eliminated per keypress.
- **O2 — Vec allocations in render path (CODERULE 4)**: `draw_task_list` replaced four `collect()` calls (visible, in-progress, open, done groups) with a single filtered iterator pass and per-status counters. Item building extracted to `task_row` helper.
- **O3 — Spurious Vec in `enter_task_list`**: cursor position search replaced with a direct iterator `position()` call; no Vec allocated for filter application.
- **O4 — CODERULE 3 function length**: all functions in `tui/src/app.rs` and `tui/src/ui.rs` reduced to ≤50 non-blank lines by extracting: `handle_filter_input`, `handle_list_pull`, `do_task_list_edit`, `do_claim_backlog`, `handle_list_action_keys`, `do_detail_edit`, `do_detail_field_cycle`, `task_list_block`, `build_task_items`, `filter_line_widget`, `render_detail_fields`, `render_detail_desc`, `task_list_params`, `draw_create_type_field`, `draw_create_assign_field`, `user_picker_items`, `build_team_items`.

## Resolved — Features (post-v0.1.0, session 2026-06-06)
- **Create form redesign**: replaced dedicated full-screen with a centered popup overlay. TITLE field uses `tui-input` with a real terminal cursor. TYPE shows inline chips `[task] bug incident` cycled with ←→. ASSIGN and CAKE expand to filterable inline lists when focused (type to filter, ↑↓ to navigate). Tab/Shift+Tab advance fields. Enter submits from any non-description field. Dependencies: `tui-input = "0.15.3"`, `ratatui-textarea = "0.9.1"`.
- **Inline description in create form**: `ratatui-textarea` field between TITLE and TYPE. Enter inserts newlines; Tab advances to next field. 850-char limit validated on submit, same as the edit path. Removed `create_via_editor` and `Screen::PickAssignee`; simplified `Screen::PickCake` (dropped `from_create` path).
- **Filter extended syntax**: `/` now accepts `@owner` (owner-only match) and `!term` (exclude matching slices). Hint text `you can use '@' for users '!' to exclude` shown dimmed in `theme.dim()` when the filter bar is open and empty. `filter_matches()` wrapper added in `app.rs`; all call sites updated.
- **Flash row**: permanent 1-row strip at the very bottom of personal/backlog/planner views for action messages (e.g. "Task created.", "Claimed."). Hint bar moved up one slot; the old blank gap between filter line and hint bar is gone. Messages no longer float as a block title.
- **Breadcrumb in detail view**: detail screen now has the full top tab bar. The right side shows `PERSONAL › task_id` (or `PLANNER ›` / `BACKLOG ›`) instead of the static `repo · username`.
- **Navbar wrapping**: `bar_text()` on `Theme` measures each chip+label pair with `UnicodeWidthStr` and splits items across two rows when total width exceeds available area. All hint bar rows changed from `Constraint::Length(1)` to `Constraint::Length(2)`. Items always split at chip boundaries — never mid-chip.

## Resolved — Features (post-v0.1.0, session 2026-06-03 continued)
- **Priority system overhaul**: `low` removed; `urgent` added above `high`. Indicators: `++` (urgent, danger+bold), `+` (high, warning+bold), blank (normal). Old repos with `priority: low` silently migrate to normal via serde. Cycle: normal→+→++→normal. Filter keywords: `urgent`, `high`.
- **`blocked` → `ai_flagged`**: field renamed throughout (model, YAML, TUI label `AI FLAGGED:`, CLI, MCP). YAML reads both `ai_flagged:` and `blocked:` (serde alias for backward compat). Writes `ai_flagged:` going forward.
- **`⚑` for ai_flagged**: red flag symbol replaces `!`. Cursor changed to `⇒` (double arrow, matches setup screen).

## Resolved — Features (post-v0.1.0, session 2026-06-03)
- **DESIGN.md**: visual design guide written — `doc/DESIGN.md` covers color palette (semantic slots, Catppuccin Mocha + Gruvbox references), typography hierarchy, selection/focus patterns, layout conventions, nav chrome spec, setup screen, hint bar rules, anti-patterns, and ratatui implementation notes.
- **Theme — new semantic slots**: `muted` (explicit fg color, replaces unreliable `Modifier::DIM`), `success` (done badges), `bg_personal`/`bg_planner`/`bg_backlog` (per-view background shading). `dim()` and `label_style()` now use `muted` fg.
- **Full-width white top bar**: single row at the top of every list/planner view. Active tab shown as `> LABEL <` in bold dark navy `rgb(0,31,96)`; inactive tabs in `DarkGray`. Right-aligned `repo · username`. Replaces the old bold-yellow block title.
- **Tab chip strip**: `PERSONAL  PLANNER  BACKLOG` — replaced the view title text. Active tab highlighted. Tab/Shift+Tab cycle remains; the strip makes it self-evident.
- **Single compressed hint bar**: two-row nav+ctrl bars replaced with one line. Context-sensitive (claim vs cycle, ^B vs ^D).
- **Per-view background shading**: `bg_personal=#141414`, `bg_planner=#141820` (cool tint), `bg_backlog=#1a1414` (warm tint). All configurable in `[theme]`.
- **Planner cake headers**: changed from `BOLD|DIM` to `bold+accent`. Progress count `[N/M]` in muted. Section headers (STANDALONE) in `bold+muted`.
- **Setup logo in accent color**: was dim, now rendered in `theme.accent` (cyan default).
- **Detail/create field label styles**: inactive labels = `bold+muted`; active (cursor on) = `bold+accent`. No bg bleed on the label — only the value row gets the highlight wash.
- **Semantic type tags in planner**: `[T]` muted, `[B]` danger, `[I]` danger+bold. Selected rows keep plain label to avoid highlight clash.
- **DONE section header in success color**: was `BOLD|DIM`; now `bold+success` (green). OPEN header uses `bold+muted`.
- **Navbar chip white background**: `chip_style()` falls back to `Color::White` when `theme.text=Reset`, restoring the nano-style key highlight.
- **Description limit**: 850-character hard limit on descriptions. Rejected on save (all three edit paths) with message: "Whoa there buddy, this is a task tracker, not The Lord of The Rings! Keep it under 850 characters. (N/850)". Detail view shows live `N/850 chars` counter above description body; muted <70%, warning 70–100%, danger+bold when over limit.
- **Claiming stays in backlog**: `do_claim_backlog` no longer forces `context = Personal`; user stays in backlog view to claim multiple items.
- **Keybinding cleanup**: Detail back = `A`/`Q` only (Esc removed). `Ctrl+S` removed from create screen. Personal `Ctrl+D` → `Ctrl+B` (move to backlog). Backlog `Ctrl+D` = permanent delete (unchanged). PROJECT.md updated.
- **Blank line below top bar**: top padding restored on all list/planner blocks.
- **Hint bars completed**: `⇧R pull` restored to all views; `A/Q back` shown in detail; `^D delete` restored after compression.

## Resolved — Features (post-v0.1.0, session 2026-06-01)
- **Tab view cycling**: Tab cycles Personal → Planner → Backlog → Personal; Shift+Tab reverses. B and P key handlers removed. Navbar shows `Tab  cycle view` after WASD in all three views.
- **Global ^C create**: Ctrl+C opens the create screen from any non-input view (TaskList, Detail, PlannerView). Bare `C` removed from task list; planner's `C` still creates a cake. Navbar shows `^C  create`.
- **View identity headers**: Block title is now bold yellow `gitcake · PERSONAL VIEW` / `PLANNER VIEW` / `BACKLOG VIEW` in all three views. Repo name and username moved from the title to a dim `info_bar` row above the nav chips.
- **Highlight respects gaps**: Selected row highlight applies only to text spans (ID, type, title). The `▶` cursor uses accent colour with no background; spaces between columns and the flag use a reset-bg spacer span so the highlight band doesn't bleed into whitespace.
- **Setup screen logo**: Roman-font ASCII art ("gitcake" in patorjk Roman font) centered as a block via horizontal layout (not `Alignment::Center`, which breaks ASCII art) + yellow `——————————— Everyone Deserves Cake ———————————` subtitle. Header reads `gitcake - created by peter olofsson`. Input line `Connect your task repo ⇒ path_` with blinking underline cursor (`SLOW_BLINK | UNDERLINED`) on all `_` cursors across all input screens.
- **Borderless UI**: all view borders removed. View titles render as plain header rows (ratatui reserves the row without a border line). Fixes mouse selection copying.
- **Create screen WASD navigation**: W/S navigate between fields (TYPE → ASSIGN TO → CAKE → CONFIRM); F activates the focused field (cycles TYPE, opens picker on ASSIGNEE/CAKE, opens editor on CONFIRM); Enter is secondary activation; Tab and Space removed. `CreateField` derives `Copy`; `next()`/`prev()` helpers added.
- **Uniform highlight style**: All navigable views (task list, planner, detail, create, pickers) use explicit `bg(highlight_bg).fg(highlight_fg)` for the selected row — replacing `Modifier::REVERSED` which produced inconsistent colors across differently-styled cells and empty spaces.
- **Theme system — colors**: `ThemeConfig` in `config.toml` exposes five semantic colors (`highlight_bg`, `highlight_fg`, `accent`, `warning`, `danger`) plus base palette (`text`, `bg`, `border`). `Theme` struct resolved at render time via `Color::from_str` with named-color + hex support. All `Color::*` literals removed from render code.
- **Theme system — symbols**: All TUI symbols configurable (`cursor`, `sym_high`, `sym_low`, `sym_blocked`, `sym_done`, `sym_open`, `sym_progress`, `sym_arrow`, `sym_dot`). No hardcoded symbol characters in render path.
- **Theme methods — block/bar factories**: `Theme::padded_block()`, `Theme::block()`, `Theme::bar_line()` replace free functions. Border color and title color applied automatically from `theme.border`. Navbar chips use `bg(text).fg(bg)` — setting `text = "blue"` makes chips blue-background automatically. Free functions `padded_block()`, `outer_block()`, `action_bar()` deleted.
- **Starship-style style strings + palette**: `ThemeConfig.highlight` is now a style string (`"bold bg:blue fg:white"`) replacing separate `highlight_bg`/`highlight_fg` fields. All color fields accept Starship format: bare color, `fg:color`, `bg:color`, modifiers (`bold dim italic underline`), in any combination. `[theme.palette]` defines named color aliases (e.g. `brand = "#268bd2"`) referenceable anywhere in style strings. `parse_style()` and `resolve_color()` parsers live in `ui.rs`.

## Resolved — Features (post-v0.1.0, session 2026-05-31)
- **Bites and crumbs**: `!bite` / `!!bite` / `!crumb` / `!!crumb` syntax in description body. Parsed at load time into `Vec<Bite>` / `Vec<Crumb>` on Task. Personal view shows `X/Y` bite progress after title. Detail view renders `○`/`✓` for bites and `·` for crumbs.
- **Cakes (epics)**: new `cakes/` folder, `Cake` / `NewCake` structs, `cake_file.rs` for I/O. Slices get optional `cake_id:` frontmatter field (patchable via `TaskPatch`). Repo: `create_cake`, `list_cakes`, `get_cake`; push stages `cakes/`. `cached_cakes` on `App` for title resolution in detail view.
- **Planner view** (replaces team view): accessed with `P` key. Groups active slices by cake with open/total progress. STANDALONE section for unattached slices. Slice rows show owner + `→`/`·` status + type + title. `C` creates a new cake inline. `D` opens detail. `P` exits back to personal.
- **Create slice — cake picker step**: Type → Assignee → Cake (optional) → Confirm. `PickCake` screen shared by create flow and detail view. First option is "none" to detach.
- **Detail view — CAKE field**: fifth navigable field after BLOCKED. `F` opens `PickCake` picker, resolves title from `cached_cakes`. Supports attach/detach.
- **Team → Planner UX polish**: WASD navbar label; filter hint spaced one row above navbar (matches personal view); `T`-only exit (A/Esc/Q no longer exit); `D` opens detail with back-to-planner routing; leading space removed from title block; open tasks show `·` dim, in-progress show `→` yellow.
- **Team view improvements**: `T` exits back to personal view; `/` filter works (hides non-matching tasks and empty user sections); full two-row navbar (nav + ctrl); `Shift+R` pull and `Ctrl+R` push work from team view.
- **Rename: git-task → gitcake**: binary `gt` → `gitcake`, config dir `~/.config/git-task/` → `~/.config/gitcake/`, repo marker `git-task.toml` → `gitcake.toml`, crate names updated.
- **CLI subcommands**: `gitcake list/create/start/done/delete/assign/sync` — thin layer on `gitcake-core`, `--json` flag on `list`, no args launches TUI.
- **MCP server**: `gitcake-mcp` crate — 9 tools over stdio transport.
- **Priority, blocked, order, parent fields**: optional frontmatter fields on all slices. `NewTask`/`TaskPatch` structs replace loose params on `create_task`/`update_task`.
- **MCP get_slice / edit_slice**: fetch single slice by ID; edit any field in place. `create_slice` accepts description and returns full JSON.
- **CLI show / set**: `gitcake show <id> [--json]` and `gitcake set <id> [--title] [--description] [--priority] [--block/--unblock] [--order] [--parent]`.
- **Detail view navigable fields**: W/S moves cursor between TYPE, STATUS, PRIORITY, BLOCKED; F cycles/toggles focused field. Separate row per field with `▶` cursor. `change_task_type` uses `git mv`.
- **Navigation sort fix**: tasks sorted at load time by (status group, priority) so W/S navigation and display order are always consistent.
