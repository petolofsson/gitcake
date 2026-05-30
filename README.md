# gitcake

Personal work tracker for developers, backed by a dedicated git repo. Track slices locally, sync to share with the team. No browser, no login, no lock-in — plain markdown files in git.

## Install

Requires Rust and a system `git` binary.

```sh
cargo install --git https://github.com/petolofsson/gitcake gitcake-tui
cargo install --git https://github.com/petolofsson/gitcake gitcake-mcp  # optional, for AI tools
```

## Setup

1. Create a new git repo (or use an existing one) and add a `gitcake.toml` at the root:

   ```toml
   name = "my-tasks"
   ```

2. Add a remote and push at least one commit so git has an `origin` to sync to.

3. Run `gitcake` — on first launch it will ask for the path to that repo.

## TUI

Run `gitcake` with no arguments to open the terminal UI.

| Key | Action |
|---|---|
| `W` / `↑`  `S` / `↓` | Navigate |
| `D` | View slice detail |
| `A` | Back |
| `C` | Create new slice |
| `E` | Edit in `$EDITOR` |
| `F` | Cycle status (open → in-progress → done) |
| `B` | Toggle personal / backlog |
| `T` | Team view (read-only) |
| `G` | Claim backlog slice |
| `Ctrl+A` | Assign slice |
| `Ctrl+P` | Pull |
| `Ctrl+O` | Change repo path |
| `Ctrl+R` | Sync (commit + push) |
| `Ctrl+D` | Delete slice |
| `Ctrl+Q` | Quit |

The TUI pulls on open and prompts to push on quit if there are unsynced changes.

## CLI

```sh
gitcake list [--json] [--status open|in-progress|done] [--backlog]
gitcake create "title" [--type task|bug|incident] [--assign username]
gitcake start <id>
gitcake done <id>
gitcake delete <id>
gitcake assign <id> --to <username>
gitcake sync
```

## MCP

`gitcake-mcp` exposes gitcake to Claude Code, Cursor, and any MCP-compatible AI tool.

Add to `~/.claude/.mcp.json`:

```json
{
  "mcpServers": {
    "gitcake": { "command": "/path/to/gitcake-mcp" }
  }
}
```

Tools: `list_slices`, `create_slice`, `start_slice`, `done_slice`, `assign_slice`, `list_users`, `sync`.
