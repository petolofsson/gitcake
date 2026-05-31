# gitcake — Known Issues & Improvements

---

## Resolved — Critical
- **Editor trap**: nano default, vi silent fallback, `$VISUAL`/`$EDITOR` respected first.
- **Silent auth failures**: pull errors stored in `App.pull_error`, classified (auth/network/other), shown on bottom border in yellow until Esc.
- **Bad task file kills list**: `collect_tasks()` skips unreadable files, returns filenames as warnings. Header shows "N slice(s) could not be read".
- **Terminal close bypasses push**: lock file at `~/.config/gitcake/{repo-key}.lock` (contains PID). Stale lock warns on next open: "Last session ended without pushing".
- **Git conflict**: `classify_push_error()` detects rejected/non-fast-forward pushes, shows pull-then-`^R`-retry message.

## Resolved — High
- **Cursor resets on every action**: `enter_task_list(preserve_id)` restores position by task ID after sort.
- **Non-ASCII usernames**: `get_username()` validates `[a-z0-9-]`, returns `UsernameInvalid` with instructions.
- **Long titles overflow**: `truncate_title()` uses `unicode-width`; budget = `inner_width − 22 − assignee_cols`.
- **No mid-session pull**: `Ctrl+P` pulls and reloads, cursor preserved, same error classifier as startup.
- **Small terminal garbles UI**: `draw()` checks `width < 60 || height < 20`, shows centred "Terminal too small" message.

## Resolved — Medium
- **Wrong DONE label**: `done_local` (not synced) vs `done_synced` (`is_completed`). Section headers include counts.
- **No team view**: `T` opens read-only view grouped by user folder (active tasks only).
- **No in-app repo change**: `Ctrl+O` opens setup screen with current path pre-filled; Esc returns.
- **Backlog assignee not validated**: `assign_backlog_task()` checks against known user folders.
- **No scroll memory from detail**: fixed by cursor-preservation — `handle_detail` back passes task ID.
- **No repo/user in header**: title shows `gitcake · {repo} · {username}` (or `· BACKLOG`).
- **No task count per group**: section headers show counts, e.g. `● IN PROGRESS (2)`.

## Resolved — Low
- **No retry hint**: `classify_push_error()` appends `· ^R to retry` to all sync failures.
- **Backlog done lifecycle**: policy — no done lifecycle for backlog. `G` claims a slice into the personal folder, preserving its hex ID, sets status open, removes from backlog.
- **`launched_editor` dead code**: removed.
- **`render_help` used by 2 screens**: setup/init-repo now use `action_bar()`, `render_help` deleted.
- **Concurrent instances stale state**: documented as known limitation; lock file warns on next open.
- **Setup screen showed Q instead of ^Q**: label corrected to `^Q`.

## Resolved — Features
- **Rename: git-task → gitcake**: binary `gt` → `gitcake`, config dir `~/.config/git-task/` → `~/.config/gitcake/`, repo marker `git-task.toml` → `gitcake.toml`, crate names updated.
- **CLI subcommands**: `gitcake list/create/start/done/delete/assign/sync` — thin layer on `gitcake-core`, `--json` flag on `list`, no args launches TUI.
- **MCP server**: `gitcake-mcp` crate — 9 tools over stdio transport.
- **Priority, blocked, order, parent fields**: optional frontmatter fields on all slices. `NewTask`/`TaskPatch` structs replace loose params on `create_task`/`update_task`.
- **MCP get_slice / edit_slice**: fetch single slice by ID; edit any field in place. `create_slice` accepts description and returns full JSON.
- **CLI show / set**: `gitcake show <id> [--json]` and `gitcake set <id> [--title] [--description] [--priority] [--block/--unblock] [--order] [--parent]`.
- **Detail view navigable fields**: W/S moves cursor between TYPE, STATUS, PRIORITY, BLOCKED; F cycles/toggles focused field. Separate row per field with `▶` cursor. `change_task_type` uses `git mv`.
- **Navigation sort fix**: tasks sorted at load time by (status group, priority) so W/S navigation and display order are always consistent.
