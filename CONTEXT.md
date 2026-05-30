# gitcake

A personal work tracker for developers, backed by a shared git repository. Each developer tracks their own slices locally and syncs at will to share progress with the team. Quick access to what you're working on — no browser, no login, no context switch.

## Language

**Practitioner**:
The target user of gitcake. Anyone who uses git as part of their daily hands-on technical work — developers, DevOps engineers, SREs, data engineers, security engineers. Excludes people who work *in* tech companies but are not doing the technical work themselves (management, product owners, business analysts).
_Avoid_: user, team member, employee

**Slice**:
A discrete unit of work tracked by a developer. Has a type, title, status, optional description, and optional assignee. Previously called "task" — "slice" reflects the idea of cutting a manageable piece from the whole.
_Avoid_: task, ticket, issue, item, card

**Type**:
A classification label on a slice. One of `task`, `bug`, or `incident`. Purely visual — does not affect lifecycle or behavior.
_Avoid_: category, kind, label

**Status**:
The lifecycle state of a slice: `open` (created, not yet activated), `in-progress` (developer has explicitly activated it), or `done` (marked complete).
_Avoid_: state, phase, stage

**Cake Repo**:
The dedicated git repository used as the source of truth for all slices. Separate from any code repository. Must already exist with a remote configured.
_Avoid_: database, store, backend, project

**User Folder**:
The directory within the cake repo named after a developer (`git config user.name`, lowercased, spaces → hyphens). Contains that developer's `open` and `in-progress` slices.
_Avoid_: user directory, personal folder, workspace

**Completed Folder**:
The `completed/{username}/` directory where done slices are moved on sync via `git mv`. Serves as a team-visible archive of finished work.
_Avoid_: archive, done folder, history

**Backlog**:
The shared `backlog/` folder at the repo root. Anyone can create slices here. Slices are identified by random 6-char hex IDs to avoid collision. No single owner.
_Avoid_: inbox, queue, pool

**Sync**:
The manual operation a developer triggers explicitly: move done slices to `completed/` → commit changes in the user folder → push. Never automatic.
_Avoid_: save, upload, backup, publish, autosave

**Core Library**:
The `gitcake-core` Rust crate. Contains all business logic — slice file parsing, git operations, repo scanning — with no dependency on any specific frontend. The open API that the TUI, CLI, MCP server, and GUI all depend on.
_Avoid_: backend, server, engine

**TUI**:
The ratatui terminal UI (`cake` binary). The v1 primary interface, designed to run in a terminal split pane alongside other tools. Shows slices grouped by status with keyboard navigation.
_Avoid_: widget, app, dashboard

**CLI**:
The planned non-interactive command-line mode of the `cake` binary. Exposes slice operations as subcommands for use in scripts and by AI agents.
_Avoid_: terminal, shell commands

**MCP Server**:
The planned `gitcake-mcp` crate. Exposes gitcake-core as a Model Context Protocol server so AI tools (Claude, Cursor, etc.) can read and write slices natively as typed tool calls.
_Avoid_: plugin, extension, API
