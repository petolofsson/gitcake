# git-task — Known Issues & Improvements

Captured during 100-user stress analysis. Ordered by impact.

---

## Critical — breaks core usage

- [x] **Editor trap on first use**
  Default editor changed to `nano`; falls back to `vi` silently if `nano` is absent.
  `$EDITOR`/`$VISUAL` respected first; hint shown only when falling back to `nano`.

- [x] **Silent auth failures at startup**
  Pull errors now stored in `App.pull_error` (separate from ephemeral message).
  Classified as auth / network / other with actionable text. Visible on bottom border
  in yellow until Esc. Same classifier applied to Ctrl+P mid-session pull.

- [x] **One bad task file kills the entire list**
  `collect_tasks()` skips unreadable files and collects their filenames as warnings.
  `scan_tasks()` / `scan_folder()` return `(Vec<Task>, Vec<String>)`. Warning shown
  in header: "N slice(s) could not be read: filename".

- [x] **Terminal close bypasses push prompt**
  Lock file written to `~/.config/git-task/{repo-key}.lock` on open (contains PID).
  Removed on clean exit. Stale lock (PID absent from `/proc`) shows warning at next
  startup: "Last session ended without pushing — consider ^R to sync".

- [x] **Git conflict has no in-app resolution path**
  `classify_push_error()` detects rejected/non-fast-forward pushes and shows:
  "Push rejected: remote has new commits — run `git pull` in the repo, then ^R to retry".

---

## High — degrades experience at scale

- [x] **Cursor resets to position 0 after every action**
  `enter_task_list()` accepts `preserve_id: Option<&str>`. All key actions that know
  the selected task ID (cycle, edit, assign, delete-cancel, detail-back) pass it through.
  Position is restored by searching the re-sorted list for the matching ID.

- [x] **Non-ASCII usernames break folder names**
  `get_username()` validates that the derived folder name contains only
  `[a-z0-9-]`. Returns `AppError::UsernameInvalid(name)` with instructions to
  set a simpler `git config user.name`.

- [x] **Long task titles overflow the row layout**
  `truncate_title()` in `ui.rs` uses `unicode-width` to measure display cells.
  Per-row budget = `inner_width − 17 (fixed prefix) − assignee_cols`.
  Truncated titles append `…` at the correct visual boundary.

- [x] **No manual pull mid-session**
  `Ctrl+P` pulls and reloads the task list, preserving cursor position.
  Shown in ctrl bar. Uses same error classifier as startup pull.

- [x] **Small terminal windows garble the UI**
  `draw()` checks `area.width < 60 || area.height < 20` before any rendering.
  Shows "Terminal too small (WxH) — resize to at least 60×20" centred on screen.

---

## Medium — noticeable friction

- [x] **"DONE (local)" label is wrong for synced tasks**
  Done group split: `done_local` (status=Done, is_completed=false) shows "DONE (local)";
  `done_synced` (is_completed=true) shows "DONE". Section headers include counts.

- [x] **No way to view other team members' tasks**
  `T` key opens a read-only team view grouped by user folder. Shows all active
  (open + in-progress) tasks from every user. WS to navigate, A/Esc to return.
  `list_team_tasks()` added to `TaskRepo` in core.

- [x] **No way to change repo path without editing config manually**
  `Ctrl+O` from the task list opens the setup screen with the current path pre-filled.
  `Screen::Setup` gained `can_cancel: bool`; Esc returns to task list when true.

- [x] **Backlog assignee not validated**
  `assign_backlog_task()` now calls `list_users()` and returns `AppError::InvalidRepo`
  if the assignee has no user folder. The TUI picker already restricts to known users.

- [x] **No scroll position memory when returning from detail view**
  Fixed by the cursor-preservation work: `handle_detail` back passes `Some(&task_id)`
  to `enter_task_list()`.

- [x] **Repo name and current user not shown in header**
  Task list title now reads: `git-task · {repo_name} · {username}` (or `· BACKLOG`).

- [x] **No task count per group**
  Section headers show counts: `● IN PROGRESS (2)`, `○ OPEN (4)`, etc.

---

## Low — polish

- [ ] **No retry button after failed push/sync**
  When sync fails, the error shows in the header but the user has to know to press Ctrl+R again.
  Fix: show explicit retry hint in the header error message.

- [ ] **Backlog done tasks never move to completed/**
  Personal done tasks move to `completed/{username}/` on sync. Backlog done tasks stay in `backlog/` forever.
  Fix: decide policy — move to `completed/backlog/` on sync, or leave in backlog and just mark done.

- [ ] **`launched_editor` flag is unused in create form**
  Minor dead code warning suppressed. Cleanup.

- [ ] **`render_help` only used by two screens**
  Setup and init-repo use the old right-aligned border help pattern. Should be migrated to the standard nav/ctrl bars for full uniformity.

- [ ] **Concurrent gt instances see stale state**
  Two `gt` windows open on the same repo — changes in one don't appear in the other until sync.
  Fix: document as known limitation; optionally add a file-watcher to detect external changes.

---

## Planned features

### CLI subcommands
Add a non-interactive CLI mode to the `cake` binary so any tool (AI or script) can drive it from the shell.

```bash
cake list [--json] [--status open|in-progress|done] [--backlog]
cake create "title" [--type task|bug|incident] [--assign username]
cake done <id>
cake start <id>
cake delete <id>
cake assign <id> --to <username>
cake sync
```

All commands output plain text by default; `--json` outputs machine-readable JSON.
Implementation: thin CLI layer on top of `git-task-core` — the library already has all the logic.
No business logic in the CLI layer. Same rule as the TUI: just call core.

### MCP server (Claude / AI integration)
A Model Context Protocol server wrapping `git-task-core` as a set of typed AI tools.
Enables Claude Code, Cursor, and any MCP-compatible AI to read and write slices natively.

Tools to expose:
- `list_tasks` — returns current tasks with status, assignee, description
- `create_task` — creates a new slice with title, type, optional description
- `update_task_status` — mark open / in-progress / done
- `assign_task` — assign to a user
- `list_users` — who is in this repo
- `sync` — commit and push

Implementation: new crate `gitcake-mcp` in the workspace, depends on `git-task-core`.
The MCP server is the first-class AI integration. CLI subcommands are the universal fallback.

### Rename: git-task → gitcake
- App name: `gitcake`
- Binary: `cake`
- Tasks referred to as "slices" in UI
- Config directory: `~/.config/gitcake/`
- Crate names: `gitcake-core`, `gitcake-tui`, `gitcake-mcp`
- Repo marker file: `gitcake.toml` (replaces `git-task.toml`)
