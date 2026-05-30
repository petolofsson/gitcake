# gitcake

A personal work tracker for developers, backed by a dedicated git repo. Each developer tracks their own slices (tasks) locally and syncs at will to share progress with the team. The goal is zero friction — a quick glance at what you're working on without logging into Jira, Azure DevOps, or any external service.

Future: CLI commands for scripting, an MCP server for AI integration, and export to external tools. The git repo is the source of truth; everything else consumes it.

---

## Vision

**gitcake should become the developer's scratchpad — always open, always honest about what they're actually working on.**

The target group is anyone who uses git as part of their daily hands-on technical work — developers, DevOps engineers, SREs, data engineers, security engineers. Not management, not product owners, not people who are *in* tech but not *working hands-on with* it.

This shapes every decision:

- **Lives where developers live.** The terminal, not a browser tab or a PM tool. Zero friction to start — a single binary, a git repo you already have.
- **Owned by the developer.** Data lives in your git repo. You control it. You can delete the tool tomorrow and the files are still there, readable by any text editor.
- **Composable.** Plain markdown files in a git repo. Scripts, grep, GitHub Actions, future exporters — anything can read it without an API or account.
- **No lock-in, no server, no account.** Like git itself — install it, use it, stop using it. No subscription, no cloud dependency.
- **Open core.** The `gitcake-core` library is the open API. Anyone can build frontends, integrations, or extensions. A healthy ecosystem around the core is what turns a useful tool into something developers depend on.

---

## Stack

| Layer | Technology |
|---|---|
| Shared core library | Rust (`gitcake-core`) |
| TUI frontend (v1) | Rust + ratatui (`gitcake-tui`, binary: `cake`) |
| CLI frontend (planned) | Rust (`gitcake-cli`, same binary: `cake --command`) |
| MCP server (planned) | Rust (`gitcake-mcp`) |
| GUI frontend (future) | Tauri v2 + Svelte (`gitcake-gui`) |
| Git operations | Shell out to system `git` binary |

### Workspace structure

```
gitcake/
  Cargo.toml        ← workspace root
  core/             ← gitcake-core: all business logic, no UI dependencies
  tui/              ← gitcake-tui: ratatui TUI, binary: cake
  src-tauri/        ← gitcake-gui: Tauri GUI (future)
  src/              ← Svelte frontend (for GUI only)
```

`core/` is the open API. Any frontend — including community-built ones — can depend on it independently.

---

## Repository structure

```
(dedicated gitcake repo)/
  gitcake.toml           ← repo-level config
  alice-smith/           ← open + in-progress slices
    001.md
    002.md
  completed/
    alice-smith/         ← done slices, moved here on sync
      003.md
  backlog/               ← shared team backlog
    a3f9c1.md
  bob-jones/
    001.md
```

- One dedicated git repo for slices, separate from code repos
- One folder per developer, named from `git config user.name` (lowercased, spaces → hyphens)
- One file per slice, named by sequential numeric ID per user folder
- Done slices move to `completed/{username}/` on sync via `git mv`
- Backlog slices live in `backlog/` with random hex IDs (shared, no ownership)
- The repo must already exist with a remote — if you can't set up a remote repo, this tool is not for you

---

## gitcake.toml

```toml
name = "acme-tasks"
```

Minimal for v1. Lives at the repo root. Used to validate that a directory is a gitcake repo.

---

## Slice (task) file format

```yaml
---
id: "001"
type: task
title: Fix login redirect
status: in-progress
created: 2026-05-29T09:14:00
done:
assignee: alice-smith
---
Optional description in markdown.
```

### Fields

| Field | Description |
|---|---|
| `id` | Sequential numeric string for personal slices; 6-char hex for backlog slices |
| `type` | `task`, `bug`, or `incident`. Required, default `task`. Purely a visual label |
| `title` | Short slice title |
| `status` | `open`, `in-progress`, or `done` |
| `created` | Full timestamp when slice was created |
| `done` | Full timestamp when slice was marked done (empty until then) |
| `assignee` | Optional. Git username of the person responsible |

---

## Status lifecycle

```
open → in-progress → done
```

| Status | Trigger |
|---|---|
| `open` | Slice created |
| `in-progress` | Developer explicitly activates the slice |
| `done` | Developer marks it done |

---

## TUI (v1)

The primary interface is a ratatui TUI binary (`cake`), designed to run in a terminal split pane alongside other tools (Claude Code, editor, shell).

### Key bindings

| Key | Action |
|---|---|
| `W` / `↑` | Navigate up |
| `S` / `↓` | Navigate down |
| `D` | View slice detail |
| `A` | Back |
| `C` | Create new slice |
| `E` | Edit slice in `$EDITOR` |
| `F` | Cycle status (open → in-progress → done → in-progress) |
| `B` | Toggle personal / backlog context |
| `Ctrl+A` | Assign slice |
| `Ctrl+R` | Sync (commit + push) |
| `Ctrl+D` | Delete slice |
| `Ctrl+Q` | Quit |

### On open

Auto-pulls from remote. Shows result in header.

### On exit

Prompts to push if uncommitted changes exist.

### Slice list

Slices displayed in three groups:

1. `in-progress` — highlighted
2. `open` — normal
3. `done` (not yet synced) — dimmed

---

## CLI (planned)

A non-interactive mode for scripting and AI integration:

```bash
cake list [--json] [--status open|in-progress|done] [--backlog]
cake create "title" [--type task|bug|incident] [--assign username]
cake done <id>
cake start <id>
cake delete <id>
cake assign <id> --to <username>
cake sync
```

---

## MCP server (planned)

A Model Context Protocol server (`gitcake-mcp`) exposing gitcake-core as AI tools. Enables Claude Code, Cursor, and any MCP-compatible AI to read and write slices natively.

---

## GUI (future / community)

The Tauri + Svelte GUI (`gitcake-gui`) depends on `gitcake-core` and provides an always-on-top floating widget.

---

## Conflict handling

Each developer owns their own folder. If a conflict occurs it is surfaced as a git error and resolved manually outside the app. No merge drivers in v1.

---

## Development guidelines

- **No AI attribution in commits.** Commit messages must not mention AI, Claude, or contain `Co-Authored-By` lines.
- **One module at a time.** Build, test, commit before moving to the next module.
- **Tests live alongside the module.** Each Rust module gets its tests in the same file.
- **`core/` has no UI dependencies.** Never import ratatui, Tauri, or any frontend crate into `gitcake-core`.
- **Follow CODERULES.md.** All 10 rules apply to every crate.

---

## Out of scope for v1

- GUI (Tauri/Svelte) frontend
- CLI subcommands — TUI only in v1
- MCP server
- Team lead / shared dashboard view
- Milestones, priority, due dates, story points
- Jira / Azure DevOps / Linear export
- Auto-commit, auto-pull, auto-push
- Multi-machine conflict resolution
- Running timer / time tracking (cycle time = done − created only)
