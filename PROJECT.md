# git-task

A cross-platform tool for developers to track their own work, backed by a dedicated git repo. Personal work journal — not a strict top-down task manager. At the end of the day, developers sync and the team lead (or anyone) can see what has been done by pulling the repo.

Future: export to Jira, Azure DevOps, or similar tools. The git repo is the source of truth; external tools are consumers of it.

---

## Vision

**git-task should become as important to technical practitioners as git itself.**

The target group is anyone who uses git as part of their daily work — developers, DevOps engineers, SREs, data engineers, security engineers. Not management, not product owners, not people who are *in* tech but not *working hands-on with* it. If the people doing the technical work love it, adoption follows naturally. No top-down mandate required.

This shapes every decision:

- **Lives where developers live.** The terminal, not a browser tab or a PM tool. Zero friction to start — a single binary, a git repo you already have.
- **Owned by the developer.** Data lives in your git repo. You control it. You can delete the tool tomorrow and the files are still there, readable by any text editor.
- **Composable.** Plain markdown files in a git repo. Scripts, grep, GitHub Actions, future exporters — anything can read it without an API or account.
- **No lock-in, no server, no account.** Like git itself — install it, use it, stop using it. No subscription, no cloud dependency.
- **Open core.** The `git-task-core` library is the open API. Anyone can build frontends, integrations, or extensions. A healthy ecosystem around the core is what turns a useful tool into something developers depend on.

---

## Stack

| Layer | Technology |
|---|---|
| Shared core library | Rust (`git-task-core`) |
| TUI frontend (v1) | Rust + ratatui (`git-task-tui`, binary: `gt`) |
| GUI frontend (future) | Tauri v2 + Svelte (`git-task-gui`) |
| Git operations | Shell out to system `git` binary |

### Workspace structure

```
git-task/
  Cargo.toml        ← workspace root
  core/             ← git-task-core: all business logic, no UI dependencies
  tui/              ← git-task-tui: ratatui TUI, depends on core
  src-tauri/        ← git-task-gui: Tauri GUI, depends on core
  src/              ← Svelte frontend (for GUI only)
```

`core/` is the open API. Any frontend — including community-built ones — can depend on it independently.

---

## Repository structure

```
(dedicated git-task repo)/
  git-task.toml          ← repo-level config
  alice-smith/           ← open + in-progress tasks
    001.md
    002.md
  completed/
    alice-smith/         ← done tasks, moved here on sync
      003.md
  bob-jones/
    001.md
```

- One dedicated git repo for tasks, separate from code repos
- One folder per developer, named from `git config user.name` (lowercased, spaces → hyphens)
- One file per task, named by sequential numeric ID per user folder
- Done tasks move to `completed/{username}/` on sync via `git mv`
- The repo must already exist with a remote — if you can't set up a remote repo, this tool is not for you

---

## git-task.toml

```toml
name = "acme-tasks"
```

Minimal for v1. Lives at the repo root. Used to validate that a directory is a git-task repo.

---

## Task file format

```yaml
---
id: "001"
type: task
title: Fix login redirect
status: in-progress
created: 2026-05-29T09:14:00
done:
---
Optional description in markdown.
```

### Fields

| Field | Description |
|---|---|
| `id` | Sequential numeric string, unique within the user's folder. Scans both `{username}/` and `completed/{username}/` to determine the next ID. |
| `type` | `task`, `bug`, or `incident`. Required, default `task`. Purely a visual label — no behavioral difference. |
| `title` | Short task title |
| `status` | `open`, `in-progress`, or `done` |
| `created` | Full timestamp when task was created |
| `done` | Full timestamp when task was marked done (empty until then) |

The `id` field is kept in the file (not just in the filename) for future export to external tools.

---

## Status lifecycle

```
open → in-progress → done
```

| Status | Trigger |
|---|---|
| `open` | Task created |
| `in-progress` | Developer explicitly activates the task for the first time |
| `done` | Developer marks it done |

Timestamps on `created` and `done` allow measuring cycle time (done − created).

There is **no automatic status transition** on app open.

---

## TUI (v1)

The primary interface is a ratatui TUI binary (`gt`), designed to run in a terminal split pane alongside other tools (Claude Code, editor, shell). This keeps tasks visible while working without requiring a separate GUI window.

### First run

- Prompt to enter the path to an existing git-task repo (or browse)
- Validates: is a git repo, has a remote, has `git-task.toml`
- Blocks with a clear error if `git config user.name` is not set
- Creates `{username}/` folder in the repo on connect

### On open

- Prompt: **"Pull latest changes? [y/N]"**
- If yes: runs `git pull`

### On exit

- If there are uncommitted changes in `{username}/`: prompt **"Push before exiting? [y/N]"**
- If yes: commits changes in `{username}/` and pushes (no pull)

### Task list

Tasks displayed in three groups:

1. `in-progress` — highlighted
2. `open` — normal
3. `done` (not yet pushed) — dimmed

### Create

- Inline form: title (required), type (default: task), description (optional)
- App assigns the next sequential ID

### Edit

- Title and description are editable after creation

### Mark done

- Sets status to `done` and records `done` timestamp
- File stays in `{username}/` until the next sync

### Sync

- Single action: move done tasks to `completed/{username}/` via `git mv` → commit `{username}/` changes → `git pull` → `git push`
- Always manual — no auto-commit, no auto-pull, no auto-push

---

## GUI (future / community)

The Tauri + Svelte GUI (`git-task-gui`) depends on `git-task-core` and provides an always-on-top floating widget. It is a future deliverable and may also be built by the open-source community using `git-task-core` as a library.

---

## Conflict handling

Each developer owns their own folder. Editing another developer's files is a workflow violation. If a conflict does occur (e.g. same user on two machines without syncing), it is surfaced as a git error and resolved manually outside the app. No merge drivers or domain-aware resolution in v1.

---

## Out of scope for v1

- GUI (Tauri/Svelte) frontend
- CLI subcommands (e.g. `gt add`, `gt done 001`) — TUI only
- Team lead / shared dashboard view
- Task reassignment between users
- Milestones, priority, due dates, story points
- Jira / Azure DevOps / Linear export
- Auto-commit, auto-pull, auto-push
- Multi-machine conflict resolution
- Running timer / time tracking (cycle time = done − created only)
