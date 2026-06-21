# gitcake — ratatui UI Spec

A redesign spec for gitcake's three core screens, written for implementation in
**Rust + [ratatui](https://ratatui.rs)**. This describes *layout, widgets, theming,
and interaction* — not storage. Adapt the data types to whatever gitcake already
uses; only the view layer is prescribed here.

**Visual source of truth (open these alongside this doc):**
- `gitcake - Two-pane Flow.html` — the chosen design: Planning → Detail → Create, all two-pane.
- `gitcake - Structure.html` — alternative structures (Framed / Tabbed) that were rejected, for context.
- Original screenshots: the list view and the incident detail/create screens.

> The mockups are built in HTML/CSS for *fidelity preview only*. CSS line-height
> makes rows look airy; in a real terminal every row is **1 cell tall**. Read the
> mockups for structure, hierarchy, and color — not pixel metrics.

Target: **ratatui ≥ 0.28**, **crossterm** backend, truecolor terminal.

---

## 1. Terminology

| gitcake term | Maps to | Lives where |
|---|---|---|
| **cake** | epic / project | a group header in the tree |
| **slice** | task | a row under a cake; the thing you "open" |
| **bite** | subtask | a child row / item inside a slice's detail |
| **crumb** | note / comment | attached to a slice (see Open Decision #2) |

A slice also carries a **type** (`task` / `bug` / `incident`), a **status**
(`open` / `in-progress` / `done`), a **priority** (`normal` / `high` / `urgent`),
an **AI-flag** (bool), an **assignee**, and a backing markdown file in the repo.

---

## 2. Visual system

### 2.1 Theme tokens → `config.toml`

All color is **data**, never hardcoded. Load a `[theme]` table into a `Theme`
struct; every widget reads `theme.*`. The default ("Frosting") palette:

```toml
# config.toml
[theme]
bg          = "#1b1714"  # terminal background (usually left transparent)
fg          = "#efe2d4"  # primary text
dim         = "#9a8a78"  # secondary text, labels, metadata
faint       = "#352d27"  # rules, dividers, inactive borders
border      = "#4a4039"  # panel borders
accent      = "#e6a4b4"  # frosting pink: titles, selection, active tab, breadcrumb
sel_bg      = "#2a2420"  # selection / focused-row background tint
in_progress = "#e6b450"  # honey  — status ●, "ahead" warning
done        = "#a7c080"  # pistachio — status ✓
open        = "#6f6258"  # muted  — status ○, tree connectors
flag        = "#ea7079"  # raspberry — AI flag ⚑, ai author
priority    = "#d98c6a"  # caramel — priority + / ++
```

```rust
#[derive(Clone)]
pub struct Theme {
    pub bg: Color, pub fg: Color, pub dim: Color, pub faint: Color,
    pub border: Color, pub accent: Color, pub sel_bg: Color,
    pub in_progress: Color, pub done: Color, pub open: Color,
    pub flag: Color, pub priority: Color,
}
// parse "#rrggbb" -> Color::Rgb(r,g,b); provide Theme::frosting() as the default.
```

> On a 16-color terminal, fall back: drop `sel_bg` and use `Modifier::REVERSED`
> for selection; map the semantic colors to the nearest ANSI color.

### 2.2 Glyphs

| Meaning | Glyph | Codepoint | Style |
|---|---|---|---|
| status: open | `○` | U+25CB | `open` |
| status: in-progress | `●` | U+25CF | `in_progress` |
| status: done | `✓` | U+2713 | `done` |
| priority: high / urgent | `+` / `++` | ASCII | `priority` (bold) |
| AI flag | `⚑` | U+2691 | `flag` |
| tree branch | `├─` `└─` | U+251C/2514 + U+2500 | `open`/`faint` |
| breadcrumb sep | `▸` | U+25B8 | `dim` |
| unpushed / git | `⚭` `⚠` | U+26AD / U+26A0 | `in_progress` / `flag` |
| progress bar | `█` / `░` | U+2588 / U+2591 | `accent` / `faint` |

Glyphs render in whatever font the user's terminal uses (per your design note).
Stick to these common Unicode points; avoid Nerd-Font-only icons.

### 2.3 Panels

Every region is a ratatui **`Block`** with **rounded** borders and a **title**:

```rust
fn panel<'a>(title: &'a str, t: &Theme) -> Block<'a> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(t.border))
        .title(Span::styled(
            format!(" {title} "),
            Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Left)
}
```

Optional right-aligned secondary title (e.g. file path) via a second
`.title(...)` with `Alignment::Right` and `dim` style.

### 2.4 Selection & focus

- **Selected list row / focused field:** background `sel_bg`, an accent left edge,
  text stays `fg`. With ratatui, set the row/line `Style` to
  `Style::new().bg(t.sel_bg)` and prefix the line with a `▌`/`▐` accent span (or
  use `List::highlight_symbol("⇒ ")` + `highlight_style`).
- **Selected segmented option** (e.g. the chosen `status`): tinted bg + colored fg +
  bold — `Style::new().fg(tone).bg(t.sel_bg).add_modifier(Modifier::BOLD)` where
  `tone` is the semantic color for that value (in-progress→`in_progress`, high→`priority`, etc.).

---

## 3. Screens

Three screens, one shared chrome. Global frame for every screen:

```
┌ row 0 ──────────────────────────────────────────────────────────────┐
│ ⇒ PERSONAL   PLANNER   BACKLOG                git-test · peter-olofsson │  Tabs (Length 1)
├ body ────────────────────────────────────────────────────────────────┤
│                                                                       │  Min(0)
│            (screen-specific — see below)                              │
│                                                                       │
├ footer ───────────────────────────────────────────────────────────────┤
│ <keymap>                                                              │  Length 1–2
└───────────────────────────────────────────────────────────────────────┘
```

```rust
let [tabs, body, footer] = Layout::vertical([
    Constraint::Length(1),
    Constraint::Min(0),
    Constraint::Length(2), // 1 keymap row; +1 for the planning warning line
]).areas(area);
```

Top bar: a `Tabs` widget on the left (`PERSONAL` / `PLANNER` / `BACKLOG`,
`.highlight_style(accent+BOLD)`, active prefixed `⇒`), and a right-aligned
`repo · user` `Span` (render as a second paragraph or split the row).

### 3.1 Planning  (`Screen::Planning`)

The primary view: a **full-width** tree of cakes → slices → bites. There is no
detail panel here — pressing **D** opens the highlighted slice in the Detail screen.

```
body:  one full-width TASKS panel
┌─ TASKS ──────────────────────────────────────────────────────────────┐
│ TASK TITLE                                        PRG  ASSIGNED       │
│ Auth Service  5/6 ───────────────────────────────────────────────    │
│⇒ +  inc ● Migrate session tokens to encrypted store ⚑  1/3  peter-…   │
│     tsk ● Login Flow                                   1/3  peter-…   │
│   ++tsk    ├─● Add OAuth provider button                             │
│     tsk    └─✓ Handle redirect callback                             │
│     tsk ● Signup Flow                                  0/3  ada-okoro │
│   ⚑ bug    └─○ Email validator rejects valid addresses              │
│ Backend API  2/2 ────────────────────────────────────────────────   │
│  +  tsk ● Optimize slow DB queries                     1/3  peter-…   │
│  +  tsk ○ Rate limiting middleware                     0/4  grace-…   │
│ STANDALONE ──────────────────────────────────────────────────────   │
│  ++ tsk ○ Test1                                                      │
│     bug ✓ Prod: memory spike on API servers                         │
└──────────────────────────────────────────────────────────────────────┘
⚠ last session ended without pushing — ^T to sync
W/S move  D open  ^V create  ⇧V cake  Q filter  Spc status  F cycle  E edit  ^R assign  ^B backlog  ⇧T pull  ^T push  ^Q quit
```

**TASKS panel** — a `List<ListItem>` inside the panel, with a `ListState` driving
the highlight. Build items by flattening cakes→slices→bites:
- **Cake header** = a non-selectable `ListItem` styled `accent + BOLD`, with the
  count (`5/6`) in `dim` and a trailing `─` rule (pad with `Constraint`/spaces to width).
- **Slice row** = a `Line` of spans: priority gutter (`+`/`++`/`⚑`, width 2, `priority`/`flag`),
  type tag (`inc`/`tsk`/`bug`, width 3, `dim`), status glyph, title (`fg`,
  truncate with `…`), then two right-hand columns: `PRG` (`1/3`, dim) and
  `ASSIGNED` (assignee, dim, truncated). Lay each row out with an inner
  `Layout::horizontal([Min(0), Length(5), Length(16)])` (title / prg / assignee)
  so columns line up under the `TASK TITLE … PRG  ASSIGNED` header.
- **Bite row** = same, indented 2–3 cells with a `├─`/`└─` connector in `faint`;
  done bites get `CROSSED_OUT` + `dim`.

Skip cake headers when moving the selection (W/S jumps slice→slice; selection
lands only on slices/bites).

**Footer** — warning line (`flag`) only shown when there are unpushed commits;
then the keymap (`dim`, with each key cap in a `sel_bg` chip — render the cap as a
`Span` styled `bg(sel_bg)`).

### 3.2 Detail  (`Screen::Detail`)  — reached with **D / ↵**

```
body: left column splits vertically; right = full-height CONTENT
╭─ SLICE PROPERTIES ────────────╮╭─ SLICE CONTENT ────────────────────────────╮
│                               ││                                              │
│ [1] TYPE     bug           ◀  ││ Auth Service  ▸  slice · #f6a7b8c9  baked … │
│    ↳ task  ·  bug  ·  incident││                                              │
│ [2] STATUS   in-progress      ││ Migrate session tokens to encrypted store    │
│ [3] PRIORITY urgent           ││                                              │
│ [4] AI FLAG  yes              ││ BITES  1/3  ──────────────────              │
│ [5] CAKE     Auth Service     ││ ├─✓ Audit plain-text token usage             │
│ [6] ASSIGN   peter-olofsson   ││ ├─● Add AES-256 KV adapter                  │
│                               ││ └─○ Migrate existing sessions                │
╰───────────────────────────────╯│                                              │
╭─ CAKE SLICES ─────────────────╮│ BODY  tasks/f6a7b8c9.md  91/850  ────       │
│                               ││ Legal flagged plain-text session tokens…     │
│ ● ++ Migrate session tokens   ││                                              │
│ ○    Login Flow               ││                                              │
│ ✓    Setup CI pipeline        ││                                              │
│                               ││                                              │
╰───────────────────────────────╯╰──────────────────────────────────────────────╯
W/S nav  A back  ⇥/⇧⇥ siblings  1-6 fields  F cycle  Spc status  E edit  ^R assign  ⇧T pull  ^T push  ^Q quit
```

All blocks have 1-line padding on all four sides.

**SLICE PROPERTIES** (fixed 34 cols) — six fields, each `[n] LABEL   value ◀` on
focus. Expand-on-focus renders a ribbon beneath: `↳ opt1 · opt2 · opt3` with the
current highlighted. **F** or `1`–`6` cycles the value. No file path or creation
date shown here.

**CAKE SLICES** — sibling slices in the same cake, sorted in-progress → open → done,
then urgent → high → normal within each group. `++`/`+` priority shown before the
title. Current slice highlighted with `sel_bg`. If the slice has no cake, the block
is titled **STANDALONE** and shows "Not part of a Cake".

**SLICE CONTENT** — breadcrumb line: `cake ▸ slice · #id` left-aligned, `baked
YYYY-MM-DD` right-aligned on the same row. Title bold. BITES section (if any) with
tree connectors. BODY section with prose only — bite/crumb lines are excluded since
they are already shown in BITES. **E** opens `$EDITOR`.

**Tab / Shift-Tab** in detail cycles through cake siblings in display order
(in-progress → open → done, urgent → high → normal), replacing the current slice
in-place without returning to the list.

### 3.3 Create  (`Screen::Create`)

Full-screen two-pane — **not** a popup. Same layout skeleton as Detail.

```
╭─ PROPERTIES ──────────────────╮╭─ PREVIEW ──────────────────────────────────╮
│                               ││                                              │
│ TITLE    Migrate session tok… ││ Auth Service  ▸  task                        │
│ ─────────────────────────     ││                                              │
│ [1] TYPE      [task]          ││ ++ Migrate session tokens to encrypted store │
│ [2] PRIORITY  [urgent]        ││                                              │
│ [3] ASSIGN    peter-olofsson ▶││ ──────────────────────────────────          │
│ [4] CAKE      Auth Service  ▶ ││ tasks/xxxxxxxx.md                            │
│                               ││ E  →  open $EDITOR for body                 │
│                               ││ ↵  →  create immediately                    │
╰───────────────────────────────╯╰──────────────────────────────────────────────╯
1 type  2 priority  3 assign  4 cake  ↵ create  ^V editor  Esc cancel  ^Q quit
```

Title typed inline at top of PROPERTIES (full cursor support). Fields 1–2 show
inline chip selection. Fields 3–4 expand to a filterable list in-place when focused.
PREVIEW reflects all choices live.

---

## 4. Navigation & keybindings — one-handed (left hand)

Design goal: **everything except typing task text is reachable with the left hand.**
WASD is the anchor; every verb lives on the left keyboard block. Text entry (titles,
body, filter query) is the only two-hand action — and while an input is focused the
command keys below are suspended until `Esc`/`↵`.

```
 Esc  1  2  3  4  5         1–5  focus property field
 Tab  Q  W  E  R  T         W/S move   A/D out / in
 Ctl  A  S  D  F  G         F cycle    Q filter  E edit  ^R assign  ^T push  ⇧T pull
 Sft  Z  X  C  V  B         ^V create  ⇧V cake
          SPACE             Space  advance status (open→in-progress→done)
```

### Global (Planning + Detail)
| Key | Action |
|---|---|
| `W` / `S` | move selection up / down (skips cake headers) |
| `D` / `↵` | open / go deeper (slice → Detail) |
| `A` / `Esc` | back / go out (Detail → Planning) |
| `Tab` / `⇧Tab` | switch top tab (PERSONAL · PLANNER · BACKLOG) |
| `Space` | advance status (open → in-progress → done) |
| `Ctrl+V` | create a new slice (→ Create) |
| `Shift+V` | create a new cake (available in all three views) |
| `E` | edit title/body in `$EDITOR` |
| `Ctrl+R` | assign to user |
| `Q` | filter / search |
| `Ctrl+T` · `⇧T` | push · pull |
| `Ctrl+Q` | quit |

### Detail
| Key | Action |
|---|---|
| `W` / `S` | move cursor between fields |
| `1`–`6` | directly cycle/toggle the corresponding field |
| `F` | cycle focused field's value |
| `Space` | advance status (open → in-progress → done) |
| `Tab` / `⇧Tab` | cycle to next/previous slice in the same cake |
| `Ctrl+R` | open assign picker |
| `E` | edit title + body in `$EDITOR` |
| `A` / `Esc` | back to list |
| `Ctrl+T` / `⇧T` | push / pull |

### Create
| Key | Action |
|---|---|
| type / `Backspace` | edit title inline |
| `1`–`4` | cycle TYPE / PRIORITY or focus ASSIGN / CAKE |
| `↑` / `↓` | navigate list when ASSIGN or CAKE is focused |
| `↵` | create immediately |
| `Ctrl+V` | open `$EDITOR` for body, then create on save |
| `Esc` | cancel (or collapse focused field first) |

> Generate the footer keymap from the same binding table the handler uses, so the UI and documentation never drift.

---

## 5. App state & event loop

```rust
enum Screen { Planning, Detail, Create }

struct App {
    theme: Theme,
    screen: Screen,
    cakes: Vec<Cake>,          // your existing model
    tree: Vec<RowRef>,         // flattened, cached; what the List renders
    list_state: ListState,     // planning selection
    open: Option<SliceId>,     // which slice Detail/Create is showing
    field_focus: usize,        // 0..6 in Detail/Create
    bite_state: ListState,
    draft: Option<DraftSlice>, // Create buffer (title input, chosen fields)
    git: GitStatus,            // { ahead, behind, last_push }
    running: bool,
}

// loop: draw(f, &app)  ->  read crossterm event  ->  app.handle_key(k)
// draw dispatches on app.screen to planning()/detail()/create() render fns,
// each taking the frame area and &App.
```

Keep render functions pure over `&App`; mutate only in `handle_key`. Recompute
`tree` when cakes change, not every frame.

---

## 6. Git surfacing (keep it light)

Only ever show **sync state**, never raw git internals:
- Planning footer: `⚠ last session ended without pushing — T to sync` (only when `ahead > 0`).
- Detail properties: `⚭ {ahead} ahead · push {rel} ago`.
- `T` push / `⇧T` pull run in the background; reflect result in the status line.

Compute ahead/behind with `git2` (`graph_ahead_behind`) or by shelling to
`git rev-list --count @{u}..HEAD`. Don't block the UI thread — run on a worker and
post results back to the loop.

---

## 7. Crates & build order

```
ratatui, crossterm        # TUI + backend
serde, toml               # config.toml -> Theme
git2  (or std::process)   # sync state + push/pull
chrono / time             # "3d ago" relative times
unicode-width             # correct truncation / right-alignment
```

Suggested milestones:
1. App loop + global frame (tabs + body + footer) + `Theme` from `config.toml`.
2. **Planning** full-width tree `List` over your data; W/S/D/A nav + left-hand verbs.
3. **Detail** two-pane; expand-on-focus field ribbon + `F` cycle; BITES list; BODY paragraph.
4. **Create** (clone Detail layout; title input + live preview).
5. Git status line + `T`/`⇧T` sync.
6. Polish: truncation, column alignment via `unicode-width`, 16-color fallback.

---

## 8. Resolved decisions

1. **No peek panel** — Planning is a single full-width list; open a slice with **D**.
2. **No structured crumbs** — notes are plain prose inside the slice's markdown
   **body** (`E` to edit); the `BODY` block renders the file. There is no crumb
   list/widget, no author/time columns.
3. **Expand-on-focus fields** — a focused property field reveals its option ribbon
   inline (`↳ open · in-progress · done`); `F` cycles; unfocused fields show only the
   current value.
4. **One-handed keymap** — all verbs sit on the left keyboard block (see §4); typing
   task text is the only two-hand action.
```
