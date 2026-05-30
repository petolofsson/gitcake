# git-task

A cross-platform desktop app for developers to track their own work, backed by git. Personal work log that becomes team-visible when pushed.

## Concept

Each developer maintains their own task list as files in a shared git repo. It is a personal work journal — not a strict top-down task manager. At the end of the day, developers sync and the team lead (or anyone) can see what has been done by pulling the repo.

Future: export to Jira, Azure DevOps, or similar tools. The git repo is the source of truth; external tools are consumers of it.

## Stack

- **Frontend**: Svelte
- **Backend**: Rust (Tauri v2)
- **Git operations**: `git2` crate or shell out

---

## Repository structure

```
(dedicated git-task repo)/
  git-task.toml          ← repo-level config
  alice-smith/           ← created on first connect
    001.md
    002.md
  bob-jones/
    001.md
```

- One dedicated git repo for tasks, separate from code repos
- One folder per developer, named from `git config user.name` (lowercased, spaces → hyphens)
- One file per task, named by sequential numeric ID per user folder
- The repo must already exist with a remote — if you can't set up a remote repo, this tool is not for you

---

## Task file format

```yaml
---
id: "001"
title: Fix login redirect
status: in-progress
created: 2026-05-29T09:14:00
started: 2026-05-29T09:14:00
done:
---
Optional description in markdown.
```

### Fields

| Field | Description |
|---|---|
| `id` | Sequential numeric string, unique within the user's folder |
| `title` | Short task title |
| `status` | `open`, `in-progress`, or `done` |
| `created` | Full timestamp when task was created |
| `started` | Full timestamp when task was first seen in the app |
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
| `in-progress` | App opened — all `open` tasks transition automatically |
| `done` | Developer marks it done in the Tauri app |

Timestamps on `started` and `done` allow measuring time in backlog vs time to complete.

---

## Tauri app

The Tauri desktop app is the primary (and only) interface. There is no user-facing CLI.

### First run
- Directory picker to connect to an existing git-task repo
- App creates `{username}/` folder in the repo on connect

### On every open
- All `open` tasks belonging to the current user transition to `in-progress` (sets `started` timestamp)

### Task list view
Tasks displayed in three groups:
1. `in-progress` — at the top
2. `open` — below
3. `done` — at the bottom

### Create
- Dialog with title (required) and description (optional)
- App assigns the next sequential ID within the user's folder
- Pulls latest before creating to avoid ID collisions

### Edit
- Title and description are editable after creation

### Mark done
- Button or checkbox sets status to `done` and records `done` timestamp

### Sync
- Single button: `git pull` → commit all local changes → `git push`
- No auto-pull on open, no auto-push on done — always manual

---

## Conflict handling

Each developer owns their own folder. Editing another developer's files is a workflow violation. If a conflict does occur, it is surfaced as a git error and resolved manually outside the app. No merge drivers or domain-aware resolution in v1.

---

## Out of scope for v1

- CLI user interface
- Team lead / shared dashboard view
- Task reassignment between users
- Milestones, priority, due dates, story points
- Jira / Azure DevOps / Linear export
- Auto-commit, auto-pull, auto-push
- Multi-machine conflict resolution
