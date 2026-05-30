# gitcake

Personal work tracker for developers, backed by a dedicated git repo. Track slices locally, sync to share with the team. No browser, no login, no lock-in — plain markdown files in git.

---

## Stack

| Layer | Technology |
|---|---|
| Core library | Rust (`gitcake-core`) |
| TUI | Rust + ratatui (`gitcake-tui`, binary: `gitcake`) |
| CLI | Rust (`gitcake-tui`, same binary: `gitcake`) |
| MCP server | Rust (`gitcake-mcp`, binary: `gitcake-mcp`) |
| GUI (future) | Tauri v2 + Svelte (`gitcake-gui`) |
| Git operations | Shell out to system `git` binary |

```
gitcake/
  Cargo.toml        ← workspace root
  core/             ← gitcake-core: business logic, no UI dependencies
  tui/              ← gitcake-tui: ratatui TUI + CLI subcommands, binary: gitcake
  mcp/              ← gitcake-mcp: MCP server, binary: gitcake-mcp
  src-tauri/        ← gitcake-gui: Tauri GUI (future)
```

---

## Repository structure

```
(dedicated gitcake repo)/
  gitcake.toml           ← repo marker
  alice-smith/           ← open + in-progress slices
    a3f2b1c4.md
  completed/
    alice-smith/         ← done slices, moved here on sync
      d7e91a2b.md
  backlog/               ← shared team backlog
    b5c0f3a1.md
  bob-jones/
    f1a2b3c4.md
```

- Dedicated git repo, separate from code repos
- One folder per developer (`git config user.name` lowercased, spaces → hyphens, ASCII only)
- One file per slice, named by 8-char hex ID
- Done slices move to `completed/{username}/` on sync via `git mv`
- Backlog slices live in `backlog/` — shared, no ownership, hex IDs prevent collision
- Repo must already exist with a remote configured

---

## gitcake.toml

```toml
name = "acme-tasks"
```

Lives at the repo root. Marks the directory as a gitcake repo.

---

## Slice file format

```yaml
---
id: "a3f2b1c4"
type: task
title: Fix login redirect
status: in-progress
created: 2026-05-29T09:14:00
done:
assignee: alice-smith
---
Optional description in markdown.
```

| Field | Description |
|---|---|
| `id` | 8-char hex string — same as the filename stem |
| `type` | `task`, `bug`, or `incident` — visual label only |
| `title` | Short slice title |
| `status` | `open`, `in-progress`, or `done` |
| `created` | Timestamp when created |
| `done` | Timestamp when marked done (empty until then) |
| `assignee` | Optional. Git username of the person responsible |

---

## Status lifecycle

```
open → in-progress → done
```

Cycling wraps: `done` → `in-progress`. Transition is explicit (F key) in personal context. In backlog context, F claims the selected slice into your personal list instead.

---

## TUI key bindings

| Key | Action |
|---|---|
| `W` / `↑` / `S` / `↓` | Navigate |
| `D` | View slice detail |
| `A` | Back |
| `C` | Create new slice |
| `E` | Edit slice in `$EDITOR` |
| `F` | Cycle status (personal) / Claim slice (backlog) |
| `B` | Toggle personal / backlog |
| `T` | Team view (read-only) |
| `Shift+R` | Pull |
| `Ctrl+A` | Assign slice (moves file to assignee's folder) |
| `Ctrl+O` | Change repo path |
| `Ctrl+R` | Push (commit + push) |
| `Ctrl+D` | Move to backlog (personal) / Permanently delete (backlog) |
| `Ctrl+Q` | Quit |

Auto-pulls on open. Prompts to push on quit if uncommitted changes exist.

---

## CLI subcommands

```bash
gitcake                          # launch TUI (last saved repo)
gitcake --repo /path/to/repo     # launch TUI with specific repo (session only)
gitcake --new                    # launch TUI with fresh setup screen

gitcake list [--json] [--status open|in-progress|done] [--backlog]
gitcake create "title" [--type task|bug|incident] [--assign username]
gitcake done <id>
gitcake start <id>
gitcake delete <id>
gitcake assign <id> --to <username>
gitcake sync
gitcake --repo /path/to/repo <subcommand>
```

Plain text by default; `--json` for machine-readable output. Thin layer on `gitcake-core` — no business logic in the CLI. Running `gitcake` with no arguments launches the TUI.

---

## MCP server

`gitcake-mcp` — Model Context Protocol server wrapping `gitcake-core`. Enables Claude Code, Cursor, and any MCP-compatible AI to read and write slices as typed tool calls.

Tools: `list_slices`, `create_slice`, `start_slice`, `done_slice`, `assign_slice`, `list_users`, `sync`.

Configure in `.mcp.json`:
```json
{
  "mcpServers": {
    "gitcake": { "command": "gitcake-mcp" }
  }
}
```

---

## Out of scope for v1

GUI, team dashboard, milestones/priorities/due dates, external tool export (Jira/Linear/ADO), auto-commit/push, conflict resolution beyond surfacing the git error, time tracking beyond `done − created`.
