# git-task

A personal work journal for developers, backed by a shared git repository. Each developer tracks their own tasks locally and syncs at will to share progress with the team.

## Language

**Task**:
A discrete unit of work tracked by a developer. Has a type, title, status, and optional description.
_Avoid_: ticket, issue, item, card

**Type**:
A classification label on a task. One of `task`, `bug`, or `incident`. Purely visual — does not affect lifecycle or behavior.
_Avoid_: category, kind, label

**Status**:
The lifecycle state of a task: `open` (created, not yet activated), `in-progress` (developer has explicitly activated it at least once), or `done` (marked complete).
_Avoid_: state, phase, stage

**Task Repo**:
The dedicated git repository used as the source of truth for all tasks. Separate from any code repository. Must already exist with a remote configured.
_Avoid_: database, store, backend, project

**User Folder**:
The directory within the task repo named after a developer (`git config user.name`, lowercased, spaces → hyphens). Contains that developer's `open` and `in-progress` tasks.
_Avoid_: user directory, personal folder, workspace

**Completed Folder**:
The `completed/{username}/` directory where done tasks are moved on sync via `git mv`. Serves as a team-visible archive of finished work.
_Avoid_: archive, done folder, history

**Sync**:
The manual operation a developer triggers explicitly: move done tasks to `completed/` → commit changes in the user folder → pull → push. Never automatic.
_Avoid_: save, upload, backup, publish, autosave

**Core Library**:
The `git-task-core` Rust crate. Contains all business logic — task file parsing, git operations, repo scanning — with no dependency on any specific frontend. The open API that both the TUI and GUI depend on.
_Avoid_: backend, server, engine

**TUI**:
The ratatui terminal UI (`gt` binary). The v1 primary interface, designed to run in a terminal split pane alongside other tools. Shows tasks grouped by status with keyboard navigation.
_Avoid_: widget, app, dashboard
