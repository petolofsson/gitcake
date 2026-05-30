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

```sh
gitcake                        # open last used repo
gitcake --repo /path/to/repo   # open a specific repo (session only)
gitcake --new                  # fresh setup screen to enter any repo
```

`--repo` and `--new` are session-only — they do not change your saved default. To permanently switch repos, use `Ctrl+O` inside the TUI.

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
| `F` | Claim backlog slice (backlog) / Cycle status (personal) |
| `Shift+R` | Pull |
| `Ctrl+A` | Assign slice |
| `Ctrl+R` | Push (commit + push) |
| `Ctrl+D` | Move to backlog (personal) / Permanently delete (backlog) |
| `Ctrl+O` | Change repo path |
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

# run any command against a specific repo without changing saved config
gitcake --repo /path/to/repo list
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

## Platform notes

**Linux** — fully supported.

**macOS** — fully supported. Use the `gitcake-macos-aarch64` binary from the release page, or install via `cargo install`.

**WSL** — use the Linux binary. Keep the gitcake repo under the WSL filesystem (`~/...`) rather than on the Windows drive (`/mnt/c/...`) to avoid slow git operations.

**Windows (native)** — not supported in v1.
