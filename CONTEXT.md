# gitcake — Language

**Practitioner**: Target user. Anyone doing hands-on technical work with git daily — developers, DevOps, SREs, data engineers, security engineers. Not management or product owners.

**Slice**: A discrete unit of work. Has type, title, status, optional description, optional assignee. Not: task, ticket, issue, card.

**Type**: Classification label on a slice — `task`, `bug`, or `incident`. Visual only, no lifecycle effect.

**Status**: Lifecycle state — `open` (created), `in-progress` (activated), `done` (complete). Explicit transitions only.

**Cake Repo**: The dedicated git repository used as source of truth for all slices. Separate from code repos. Must have a remote configured.

**User Folder**: `{username}/` in the cake repo — named from `git config user.name` (lowercased, spaces → hyphens). Holds open and in-progress slices.

**Completed Folder**: `completed/{username}/` — done slices moved here on sync via `git mv`.

**Backlog**: `backlog/` — shared folder, no owner. Hex IDs prevent collision. Anyone can create; G key claims a slice into your personal folder.

**Sync**: Explicit operation only — move done slices → commit → push. Never automatic.

**Core Library**: `gitcake-core` — all business logic, no UI dependencies. The open API that all frontends depend on.

**TUI**: `gitcake` binary — ratatui terminal UI. Primary interactive interface.

**CLI**: Non-interactive subcommands on the `gitcake` binary for scripts and AI agents. Running `gitcake` with no arguments launches the TUI; any subcommand runs non-interactively.

**MCP Server**: `gitcake-mcp` — exposes core as Model Context Protocol tools for AI integration (Claude Code, Cursor, etc.).
