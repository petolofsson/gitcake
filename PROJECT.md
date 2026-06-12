# gitcake

Personal work tracker for developers, backed by a dedicated git repo. Track slices locally, sync to share with the team. No browser, no login, no lock-in — plain markdown files in git.

---

## Stack

| Layer | Technology |
|---|---|
| Core library | Rust (`gitcake-core`) |
| TUI | Rust + ratatui (`gitcake-tui`, binary: `gitcake`) |
| CLI | Rust (`gitcake-tui`, same binary: `gitcake`) |
| MCP server | Rust (`gitcake-mcp`, binary: `gitcake-mcp`) |
| GUI (future) | Tauri v2 + Svelte (`gitcake-gui`) |
| Git operations | Shell out to system `git` binary |

```
gitcake/
  Cargo.toml        ← workspace root
  core/             ← gitcake-core: business logic, no UI dependencies
  tui/              ← gitcake-tui: ratatui TUI + CLI subcommands, binary: gitcake
  mcp/              ← gitcake-mcp: MCP server, binary: gitcake-mcp
  src-tauri/        ← gitcake-gui: Tauri GUI (future)
```

---

## Repository structure (v2)

```
(dedicated gitcake repo)/
  gitcake.toml      ← repo marker
  cakes/
    e4f1a2b3.md     ← epic: CI/CD Pipeline Modernization
  tasks/
    a3f2b1c4.md     ← owner: alice-smith, cake_id: e4f1a2b3
    b5c0f3a1.md     ← owner: (none) = backlog
  bugs/
    d7e91a2b.md     ← owner: bob-jones
  incidents/
    f1a2b3c4.md
```

- One folder per **type** (`tasks/`, `bugs/`, `incidents/`) — not per person
- Cakes (epics) live in `cakes/` — separate entity, not a slice type
- Ownership is a frontmatter field (`owner:`), not a directory
- `owner: alice-smith` → appears in Alice's personal view
- No `owner:` field → appears in the shared backlog view
- Done slices stay in their type folder — status is frontmatter only, no file moves
- Auto-migrates v1 repos (personal folders + backlog/) on first open

---

## gitcake.toml

```toml
name = "acme-tasks"
```

Lives at the repo root. Marks the directory as a gitcake repo.

---

## Cake file format

```yaml
---
id: "e4f1a2b3"
title: CI/CD Pipeline Modernization
created: 2026-05-31T10:00:00
owner: alice-smith
---
Optional description.
```

Cakes are epics — they group slices. Status is derived (open slice count), not stored. Slices reference a cake via `cake_id:` in their frontmatter.

---

## Slice file format (v2)

```yaml
---
id: "a3f2b1c4"
title: Fix login redirect
status: in-progress
created: 2026-05-29T09:14:00
done:
owner: alice-smith
priority: high
ai_flagged: true
order: 2
parent: b5c0f3a1
cake_id: e4f1a2b3
---
Some context about this slice.

!bite Design login form UI
!bite Implement API integration
!!bite Add validation logic
!crumb Fix typo in error message
```

| Field | Description |
|---|---|
| `id` | 8-char hex string — same as the filename stem |
| `title` | Short slice title |
| `status` | `open`, `in-progress`, or `done` |
| `created` | Timestamp when created |
| `done` | Timestamp when marked done (empty until then) |
| `owner` | Optional. Git username of the person responsible. Empty = unowned (backlog) |
| `priority` | Optional. `urgent`, `high`, or `normal` (default, omitted from file) |
| `ai_flagged` | Optional. `true` when AI needs human input to continue. Omitted when false |
| `order` | Optional. Sequence number for AI-planned work ordering |
| `parent` | Optional. ID of a parent slice for grouping subtasks |
| `cake_id` | Optional. ID of the cake (epic) this slice belongs to |

Type is derived from the parent folder name (`tasks/` → task, `bugs/` → bug, `incidents/` → incident) and is not stored in the file. Optional fields are omitted from the file when at their defaults.

---

## Bites and crumbs

Bites and crumbs are subtask syntax written in the slice description body. Parsed at load time — no new files, no separate storage.

| Syntax | Meaning |
|---|---|
| `!bite text` | Bite — open subtask |
| `!!bite text` | Bite — done subtask |
| `!crumb text` | Crumb — optional ad-hoc note/fix |
| `!!crumb text` | Crumb — done |

Rules: line must start with the token (no leading whitespace). Mid-line occurrences are treated as prose.

**Personal view**: shows `X/Y` bite progress after the title (e.g. `2/4`).
**Detail view**: bites render as `○ text` (open) or `✓ text` (done, dim). Crumbs render as `· text` (always dim).

---

## Work hierarchy

```
Cake     ← epic (optional grouping, owns many slices)
  Slice  ← task/bug/incident (the daily work unit)
    Bite ← concrete subtask in the description
    Crumb← optional ad-hoc note/fix in the description
```

Slices can exist without a cake (standalone). Cakes can exist with zero slices.

---

## Status lifecycle

```
open → in-progress → done
```

Cycling wraps: `done` → `in-progress`. Transition is explicit (F key) in personal context. In backlog context, F claims the selected slice into your personal list; Ctrl+A opens a user picker to assign to anyone.

---

## TUI key bindings

### Personal / Backlog / Planner — shared list bindings

| Key | Action |
|---|---|
| `W` / `↑`  `S` / `↓` | Navigate |
| `Enter` | View slice detail |
| `Ctrl+C` | Create new slice |
| `Q` / `/` | Filter — matches title, hex ID, owner, type, status, `ai_flagged`, `urgent`, `high`. Prefix `@` for owner, `!` to exclude, `#` for cake name. `Esc` to clear |
| `Ctrl+R` | Assign slice to any user (opens picker) |
| `Ctrl+B` | Move to backlog (clears owner, resets status to open) |
| `Ctrl+D` | Permanently delete slice |
| `Shift+T` | Pull |
| `Ctrl+T` | Push (commit + push) |
| `H` | Toggle DONE section visibility |
| `Tab` | Cycle view forward: Personal → Planner → Backlog |
| `Shift+Tab` | Cycle view in reverse |
| `Ctrl+Q` | Quit |

**Planner only:** `Shift+C` creates a new cake inline.

List displays a priority/flag indicator: `++` (urgent, red), `+` (high, yellow), `⚑` (ai_flagged, red). Tasks are sorted in-progress → open → done, then urgent → high → normal within each group. Personal view owner column is blank (implied). Backlog shows owner.

Tree layout groups slices by cake then standalone, with `├──`/`└──` connectors and status symbol inline before the title.

Planner shows all slices grouped by cake — active slices under their cake header, then a STANDALONE section, then a DONE section at the bottom. Each cake header shows open/total slice count. `H` toggles the DONE section.

### Create slice view

Full-screen two-pane layout: left = **PROPERTIES** panel (34 cols), right = **PREVIEW** panel. A `N of 4 set` counter and accent progress bar appear at the bottom of PROPERTIES.

| Key | Action |
|---|---|
| Any char / `Backspace` | Edit title (typed inline at top of PROPERTIES) |
| `1` | Cycle TYPE (task → bug → incident → task) |
| `2` | Cycle PRIORITY (normal → high → urgent → normal) |
| `3` | Focus ASSIGN — type to filter, `↑`/`↓` to select |
| `4` | Focus CAKE — type to filter, `↑`/`↓` to select |
| `Enter` | Create immediately |
| `Ctrl+C` | Open `$EDITOR` to write body, then create on save |
| `Esc` | If ASSIGN/CAKE focused: return to title. If title: cancel |
| `Ctrl+Q` | Quit |

PREVIEW shows a live tree rendering of how the slice will appear in the list (cake header + slice row with type, priority, and title), plus `WRITES <type>/<hash>.md` and `BODY $EDITOR on create · E inline`.

### Detail view

Three-block layout: left column splits into **SLICE PROPERTIES** (top) and **CAKE SLICES** or **STANDALONE** (bottom); right column is **CONTENT**. All blocks have 1-line padding on all sides.

| Key | Action |
|---|---|
| `A` / `Esc` | Back to list |
| `W` / `S` | Move cursor between fields |
| `1`–`6` | Directly cycle/toggle the corresponding field |
| `F` | Cycle the focused field's value |
| `Space` | Cycle status directly |
| `B` | Add a bite (sub-task) |
| `E` | Edit title + description in `$EDITOR` |
| `Tab` / `Shift+Tab` | Cycle to next/previous slice in the same cake |
| `Ctrl+R` | Assign slice to any user |
| `Ctrl+T` | Push |
| `Shift+T` | Pull |
| `Ctrl+Q` | Quit |

**SLICE PROPERTIES** shows six editable fields (TYPE, STATUS, PRIORITY, AI FLAG, CAKE, ASSIGN) with a focused-row caret and an expand ribbon showing options, plus two read-only rows (FILE, CREATED). A git-ahead status line (`N ahead · push Xd ago`) is pinned to the panel bottom.

**CAKE SLICES** lists all sibling slices in the same cake, sorted in-progress → open → done then urgent → high → normal. The current slice is highlighted. Standalone slices show **STANDALONE** with "Not part of a Cake".

**CONTENT** shows: breadcrumb (`cake ▸ slice · #id`) with creation date right-aligned, title in bold, a BITES section (if any), and a BODY section with the prose description.

Filter persists across screen transitions (detail, edit, assign) until Esc is pressed.

Auto-pulls on open. Prompts to push on quit if uncommitted changes exist.

---

## CLI subcommands

```bash
gitcake                          # launch TUI (last saved repo)
gitcake --repo /path/to/repo     # launch TUI with specific repo (session only)
gitcake --new                    # launch TUI with fresh setup screen

gitcake list [--json] [--status open|in-progress|done] [--backlog]
gitcake create "title" [--type task|bug|incident] [--assign username]
             [--priority urgent|high|normal] [--order N] [--parent <id>]
gitcake show <id> [--json]
gitcake set <id> [--title "..."] [--description "..."] [--priority urgent|high|normal]
          [--block] [--unblock] [--order N] [--parent <id>]
          # pass "" to --description or --parent to clear the field
gitcake done <id>
gitcake start <id>
gitcake delete <id>
gitcake assign <id> --to <username>
gitcake sync
gitcake --repo /path/to/repo <subcommand>
```

Plain text by default; `--json` for machine-readable output. Thin layer on `gitcake-core` — no business logic in the CLI. Running `gitcake` with no arguments launches the TUI.

---

## MCP server

`gitcake-mcp` — Model Context Protocol server wrapping `gitcake-core`. Enables Claude Code, Cursor, and any MCP-compatible AI to read and write slices as typed tool calls.

Tools: `list_slices`, `create_slice`, `get_slice`, `edit_slice`, `start_slice`, `done_slice`, `assign_slice`, `list_users`, `sync`.

- `create_slice` — accepts `title`, `type`, `assignee`, `description`, `priority`, `order`, `parent_id`. Returns full slice JSON.
- `get_slice` — fetch one slice by ID, returns full JSON including description body.
- `edit_slice` — update any field (`title`, `description`, `priority`, `blocked`, `order`, `parent_id`). Omit a field to leave it unchanged. Pass `""` for `description` or `parent_id` to clear the field.

Note: cake operations not yet exposed via MCP tools.

Configure in `~/.claude/.mcp.json` (use full binary path):
```json
{
  "mcpServers": {
    "gitcake": { "command": "/home/you/.cargo/bin/gitcake-mcp" }
  }
}
```

---

## Configuration

Config file: `~/.config/gitcake/config.toml` (created on first save; all keys optional — omitted keys use defaults).

### Key remapping

```toml
[keys]
up           = "w"
down         = "s"
detail       = "d"
back         = "a"
create       = "c"
edit         = "e"
status_cycle = "f"
push         = "ctrl+r"
quit         = "ctrl+q"
```

### Theme

Style strings follow [Starship](https://starship.rs/config/#style-strings) format: space-separated tokens — `bold`, `dim`, `italic`, `underline`, `fg:color`, `bg:color`, or a bare color name (treated as fg). Colors accept ANSI names (`blue`, `cyan` …), hex (`#268bd2`), ANSI 256-index (`21`), or palette aliases.

```toml
[theme.palette]         # optional named colors — reference them anywhere
brand   = "#268bd2"
surface = "#073642"

[theme]
# Interaction
highlight = "bold bg:blue fg:white"   # selected row bg + fg
accent    = "cyan"                     # cursor ▶ color
warning   = "yellow"                   # in-progress / high priority
danger    = "red"                      # urgent priority + ai_flagged indicator
success   = "green"                    # done section header
muted     = "dark_gray"               # secondary text, done items

# Base palette
text   = "reset"   # primary text; drives navbar chip background
bg     = "reset"   # background; drives navbar chip foreground
border = "reset"   # border characters + view title color

# Per-view background shading
bg_personal = "#141414"
bg_planner  = "#141820"
bg_backlog  = "#1a1414"

# Symbols
cursor       = "⇒"
sym_urgent   = "++"
sym_high     = "+"
sym_blocked  = "⚑"
sym_done     = "✓"
sym_open     = "○"
sym_progress = "●"
sym_arrow    = "→"
sym_dot      = "·"
```

Navbar key chips automatically invert relative to `text`/`bg`: chip background = `text`, chip foreground = `bg`. Setting `text = "white"` + `bg = "black"` gives the classic white chip on a dark terminal.

---

## Out of scope for v1

GUI, team dashboard, milestones/priorities/due dates, external tool export (Jira/Linear/ADO), auto-commit/push, conflict resolution beyond surfacing the git error, time tracking beyond `done − created`, Recipe level (above Cake).
