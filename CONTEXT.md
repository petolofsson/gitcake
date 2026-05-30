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
The lifecycle state of a task: `open` (created, not yet started), `in-progress` (developer has activated it at least once), or `done` (marked complete).
_Avoid_: state, phase, stage

**Task Repo**:
The dedicated git repository used as the source of truth for all tasks. Separate from any code repository. Must already exist with a remote configured.
_Avoid_: database, store, backend, project

**User Folder**:
The directory within the task repo named after a developer (`git config user.name`, lowercased, spaces → hyphens). Contains that developer's open and in-progress tasks.
_Avoid_: user directory, personal folder, workspace

**Completed Folder**:
The `completed/{username}/` directory where done tasks are moved on sync. Serves as a team-visible archive of finished work.
_Avoid_: archive, done folder, history

**Sync**:
The manual operation a developer triggers explicitly: pull latest → commit local changes in the user folder → push. Never automatic.
_Avoid_: save, upload, backup, publish, autosave

**Widget**:
The always-on-top floating panel that is the primary interface. Shows all tasks with `in-progress` tasks highlighted and others dimmed.
_Avoid_: window, panel, overlay, HUD
