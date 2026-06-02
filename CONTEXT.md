# gitcake — Language

**Practitioner**: Target user. Anyone doing hands-on technical work with git daily — developers, DevOps, SREs, data engineers, security engineers. Not management or product owners.

**Design doc**: `doc/DESIGN.md` — visual identity, color semantics, typography, navigation patterns, and ratatui implementation notes for the TUI.

**Slice**: A discrete unit of work. Has type, title, status, optional description, optional owner. Not: task, ticket, issue, card.

**Type**: Classification label on a slice — `task`, `bug`, or `incident`. Determines which type folder the file lives in. Visual only, no lifecycle effect.

**Status**: Lifecycle state — `open` (created), `in-progress` (activated), `done` (complete). Explicit transitions only. Stored as frontmatter; the file does not move when status changes.

**Owner**: Git username of the person responsible for a slice (`owner:` frontmatter field). Empty = unowned. Ownership determines which view the slice appears in — personal or backlog.

**Personal view**: Filtered view showing all slices where `owner == current user`, across all type folders.

**Backlog view**: Filtered view showing all slices where `owner` is empty, across all type folders.

**Type folders**: `tasks/`, `bugs/`, `incidents/` at the repo root. All slices live here regardless of status or owner. No files move after initial creation.

**Cake Repo**: The dedicated git repository used as source of truth for all slices. Separate from code repos. Must have a remote configured.

**Claim**: Taking ownership of a backlog slice by setting `owner` to the current user. Fast path: F key. No file move — the slice stays in its type folder.

**Sync**: Explicit operation only — commit all type folder changes and push. Never automatic.

**Core Library**: `gitcake-core` — all business logic, no UI dependencies. The open API that all frontends depend on.

**TUI**: `gitcake` binary — ratatui terminal UI. Primary interactive interface.

**CLI**: Non-interactive subcommands on the `gitcake` binary for scripts and AI agents. Running `gitcake` with no arguments launches the TUI; any subcommand runs non-interactively.

**MCP Server**: `gitcake-mcp` — exposes core as Model Context Protocol tools for AI integration (Claude Code, Cursor, etc.).
