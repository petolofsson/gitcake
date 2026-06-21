# gitcake TUI Design Guide

Visual identity, color semantics, typography, and navigation patterns for gitcake's ratatui interface.

---

## 1. Design principles

**Information density over decoration.** Every visual element earns its place. No color for color's sake.

**Keyboard rhythm.** The eye follows the active item; the interface reinforces that with a single, consistent highlight contract.

**Theme-agnostic by default.** The default palette uses `Color::Reset` for most surfaces, so gitcake looks correct on dark terminals, light terminals, and custom themes without special-casing. Explicit hex only enters through the `[theme]` config block.

**Redundancy.** Color is never the *only* signal. Shape, symbol, or bold weight always accompanies it so colorblind users and monochrome terminals remain usable.

---

## 2. Color palette

### 2.1 Semantic slot model

gitcake defines a fixed set of semantic slots. The `[theme]` config maps colors into these slots. Widget code references slots — never raw hex.

| Slot | Default | Role |
|---|---|---|
| `text` | `reset` | Primary foreground (body text, list items) |
| `bg` | `reset` | Primary background (most surfaces) |
| `accent` | `cyan` | Cursor `▶`, focused field border/title, tab chip focus ring |
| `highlight` | `bold bg:blue fg:white` | Selected row — full-width bg + fg swap |
| `warning` | `yellow` | In-progress badge, high-priority `^`, pending states |
| `danger` | `red` | Blocked `!`, error messages, destructive confirmations |
| `muted` | `dim` | Done slices, secondary metadata, crumbs, unfocused pane chrome |
| `success` | `green` | Done status badge, confirmed actions |
| `border` | `reset` | Box-drawing characters, pane dividers, view title text |
| `header` | `bold yellow` | View title line (PERSONAL VIEW, PLANNER, BACKLOG) |
| `info_bar` | `dim` | repo·username line above nav chips |

### 2.2 Suggested palette: Catppuccin Mocha

High-quality default for `config.toml` or theme preset. Paste-ready hex values.

| Slot | Hex | Catppuccin name |
|---|---|---|
| `text` | `#cdd6f4` | Text |
| `bg` | `#1e1e2e` | Base |
| `accent` | `#89b4fa` | Blue |
| `highlight_bg` | `#313244` | Surface0 |
| `warning` | `#f9e2af` | Yellow |
| `danger` | `#f38ba8` | Red |
| `muted` | `#6c7086` | Overlay0 |
| `success` | `#a6e3a1` | Green |
| `border` | `#585b70` | Surface2 (unfocused) |
| `border_focused` | `#cba6f7` | Mauve (focused pane) |
| `header` | `#f9e2af` | Yellow (bold) |
| `info_bar` | `#6c7086` | Overlay0 (dim) |

### 2.3 Alternative: Gruvbox Dark Medium

Warmer, earth-toned. Substitutes into the same slot model.

| Slot | Hex | Gruvbox name |
|---|---|---|
| `text` | `#ebdbb2` | fg1 |
| `bg` | `#282828` | bg0 |
| `accent` | `#83a598` | Blue neutral |
| `highlight_bg` | `#3c3836` | bg1 |
| `warning` | `#fabd2f` | Yellow bright |
| `danger` | `#fb4934` | Red bright |
| `muted` | `#7c6f64` | bg4 |
| `success` | `#b8bb26` | Green bright |
| `border` | `#504945` | bg2 |
| `border_focused` | `#d3869b` | Purple bright |
| `header` | `#fabd2f` | Yellow bright (bold) |
| `info_bar` | `#7c6f64` | bg4 (dim) |

### 2.4 Contrast targets

- Any fg/bg pair where **gitcake controls both sides**: target ≥ 4.5:1 (WCAG AA).
- Any fg on `Color::Reset` background: cannot guarantee a ratio — rely on semantic redundancy (bold + color, not color alone).
- The `danger` and `warning` slots should use bold to remain readable even if the terminal's red or yellow is dim.

---

## 3. Typography and text attributes

### 3.1 Attribute semantics

| Attribute | When to use | When not to use |
|---|---|---|
| **Bold** | Headers, focused titles, active pane label, status badges, errors | Body text, secondary metadata |
| Dim | Done items, crumbs, info bar, unfocused chrome | Primary content — too inconsistent across terminals |
| Italic | Never — poor terminal coverage, no semantic role in gitcake | — |
| Underline | Focused input field active cursor line; URL-like values | General emphasis |
| Reversed | Selection highlight fallback (use full highlight slot instead) | Headers — too aggressive |
| Strikethrough | Done bites/crumbs if terminal supports it | Status indicator (use color + symbol instead) |
| Blink | Never | — |

### 3.2 Hierarchy recipe

For any given widget, apply style from this stack:

```
Level 0 (most common): fg=text, bg=reset      → body content, list items
Level 1:               fg=muted               → secondary info, metadata, done items
Level 2:               fg=accent + bold       → active pane title, focused field label
Level 3:               fg=text, bg=highlight  → selected row (full-width)
Level 4:               fg=warning + bold      → in-progress badge, high-priority marker
Level 5:               fg=danger + bold       → blocked indicator, error state
Level 6:               fg=success + bold      → done badge, confirmed push
```

Never skip levels. If you need to de-emphasise something relative to body text, use Level 1 (muted fg). Do not invent a new color.

---

## 4. Selection and focus

### 4.1 Selected list row

Full-width highlight: `bg=highlight_bg, fg=text, bold`. The background wash is the primary signal; bold reinforces it.

```rust
Style::new()
    .bg(theme.highlight_bg)
    .add_modifier(Modifier::BOLD)
```

Alternatively, `Modifier::REVERSED` is the safe fallback when the user has not configured `highlight`:

```rust
Style::new().add_modifier(Modifier::REVERSED)
```

### 4.2 Focused pane / widget border

When a pane is active, its border (or title) switches from `muted` to `accent`:

```
Unfocused border: fg=muted (Surface2 / bg2)
Focused border:   fg=accent (Blue / aqua)  +  title BOLD
```

gitcake currently has no borders; the same principle applies to the view header chip or any future border introduced.

### 4.3 Cursor indicator

The `▶` cursor marks the navigable row within detail/create views. Style: `fg=accent`. The cursor is always in column 0 of its area; the row content starts at column 2 (one space gutter).

### 4.4 Unfocused vs focused views

When the user enters a sub-view (detail, create, picker), the parent list dims:

```
Focused sub-view:   normal style hierarchy
Background list:    add Modifier::DIM to all rendered lines
```

This establishes a clear modal layer without hiding content.

---

## 5. Navigation chrome

### 5.1 Info bar (row 0)

```
  gitcake-tasks · alice-smith
```

Style: `fg=muted` (dim). Keeps the orientation anchor present without competing with the header. Left-padded 2 spaces.

### 5.2 Nav chips (row 1)

```
  [ PERSONAL ]  [ PLANNER ]  [ BACKLOG ]
```

- Inactive chip: inverted relative to `text`/`bg` — chip bg = `text` color, chip fg = `bg` color
- Active chip: `bold + fg=accent` or `bold + bg=accent + fg=bg`
- One space gap between chips

The chip strip is the only persistent navigation element. Tab cycles right; Shift+Tab cycles left. The active chip is immediately legible.

### 5.3 View header (row 2)

```
  gitcake · PERSONAL VIEW
```

Style: `bold + fg=header` (bold yellow). The prefix `gitcake ·` is dim; the view name is bold. This makes the active context scannable at a glance.

### 5.4 Filter bar

When `/` is active, a filter input appears below the header:

```
  / fix login_
```

Style: `fg=accent` for the `/` prompt; normal text for the query; cursor rendered as underline on the active character position. No border needed — the prompt character is sufficient.

---

## 6. Status and priority indicators

### 6.1 Status badges (inline)

Displayed next to the slice title in lists.

| Status | Symbol | Style |
|---|---|---|
| `open` | `○` | `fg=muted` |
| `in-progress` | `●` or `→` | `fg=warning + bold` |
| `done` | `✓` | `fg=success + dim` |

### 6.2 Priority / blocked indicators (left column)

Displayed to the left of the 8-char hex ID.

| State | Symbol | Style |
|---|---|---|
| `high` | `^` | `fg=warning` |
| `low` | `v` | `fg=muted` |
| `blocked` | `!` | `fg=danger + bold` |
| normal, unblocked | ` ` (space) | — |

This column is always 1 character wide. Aligned with a 1-space left margin.

### 6.3 Bite progress

Inline after the slice title: `2/4` in `fg=muted`. Only shown when there are bites. When all bites are done: `fg=success`.

### 6.4 Type labels

In the Planner view, each slice row shows a type tag:

| Type | Symbol | Style |
|---|---|---|
| task | `[T]` | `fg=muted` |
| bug | `[B]` | `fg=danger` |
| incident | `[I]` | `fg=danger + bold` |

---

## 7. Layout conventions

### 7.1 Column widths (list view)

```
[priority] [8-char id]  [status sym] [title (flex)]  [bite progress] [owner]
  1           8            1            grows             4              max 12
```

Left margin: 2 spaces. Right margin: 1 space.

### 7.2 Detail view rows

Five navigable rows, each structured as:

```
▶  FIELD_NAME   value
```

- Column 0: cursor (`▶` when focused, ` ` otherwise)
- Column 2–13: label (`FIELD_NAME`) — `bold + fg=muted` when unfocused, `bold + fg=accent` when cursor is on this row
- Column 15+: value — normal style, or `fg=warning`/`fg=danger` to reflect semantic state

### 7.3 Section headers (Planner)

Cake headers in the Planner list:

```
  ▸ CI/CD Pipeline Modernization  [3/7]
```

Style: `bold + fg=accent`. The slice count `[3/7]` is `fg=muted`. A `▸` (or `▼` when expanded) marks collapsibility if that's added later. One blank row between cake sections.

### 7.4 Vertical whitespace

- 0 blank lines between header strip (info bar + chips + title) and list
- 1 blank line between the last list item and the status bar / help hint
- Inside detail view: 1 blank line between the header and the navigable fields; 1 blank line between fields and the description body

---

## 8. Setup screen

The setup screen is the first-run experience. It must feel like a tool that has taste.

```
  [ASCII logo — patorjk Roman font, fg=accent]
  ——————————— Everyone Deserves Cake ———————————   [fg=warning]

  GIT REPO PATH   _                                [fg=text, underline on active field]
  YOUR USERNAME   _
  THEME           Catppuccin Mocha ▸
```

- Logo: `fg=accent` (cyan/blue). Not bold — the font weight is already provided by the ASCII art.
- Tagline: `fg=warning` (yellow). Centered. Em-dash separators.
- Input labels: `bold + fg=muted` when inactive; `bold + fg=accent` when focused.
- Active input: underline on text; blinking `_` cursor.
- Cursor blink: rendered in the ratatui tick loop — the `_` toggles visibility every ~500 ms.

---

## 9. Key binding discoverability

The bottom of every non-input view carries a one-line hint bar:

```
  ^V new  ⇧V cake  D detail  Spc status  Q filter  ^R assign  ⇧T pull  ^T push  ^Q quit
```

Style: `fg=muted`. Key names are `bold + fg=accent` (or a slightly brighter muted). This mimics the Helix / bottom status bar pattern.

Rules:
- Never more than one line. Omit less-frequent bindings before wrapping.
- Context-sensitive: show only keys valid in the current view. The create/detail views suppress nav keys.
- `?` full help overlay (future): modal showing all bindings, styled as a table with `bold` key names and `fg=muted` descriptions.

---

## 10. Motion and animation

gitcake is a static-render TUI — no continuous animation except:

1. **Blinking cursor on setup inputs** — 500 ms toggle, CPU-free (toggle a bool in state, redraw on tick).
2. **Git operation spinner** — a 4-frame sequence (`/ - \ |`) shown in the info bar during pull/push. Style: `fg=accent`.
3. **No other motion.** Smooth scrolling, fade transitions, and animated progress bars are out of scope and add noise.

---

## 11. Anti-patterns to avoid

| Pattern | Why |
|---|---|
| Bold on all list items | Kills emphasis — bold is meaningless if universal |
| Red for anything not error/blocked | Trains the eye to ignore danger signals |
| Dim as the only differentiation | Dim renders inconsistently across terminals; pair with explicit muted fg |
| Color as the only signal | Fails colorblind users and monochrome terminals; always add a symbol |
| Borders as decoration | gitcake removed all borders intentionally — they add noise without conveying information |
| Blinking text other than setup cursor | Poor accessibility; distracts focus |
| More than 4 accent colors visible simultaneously | Palette fatigue; reserve accent slots for meaningful semantic roles |
| Hard-coded hex in widget render code | Makes theming fragile; all colors go through the `Theme` struct |

---

## 12. ratatui implementation notes

### Theme struct

```rust
pub struct Theme {
    pub text:           Style,
    pub muted:          Style,
    pub accent:         Style,
    pub highlight:      Style,
    pub warning:        Style,
    pub danger:         Style,
    pub success:        Style,
    pub header:         Style,
    pub info_bar:       Style,
    pub cursor:         Style,    // ▶ symbol style
    pub border:         Style,
    pub border_focused: Style,
}
```

Build once at startup from `config.toml`. Pass as `&Theme` into every widget render function. Never call `Style::new()` with raw colors inside a widget.

### Selection highlight

```rust
// Preferred: explicit bg wash
.highlight_style(theme.highlight)
.highlight_symbol("  ")   // indent, no symbol needed

// Fallback if theme.highlight is reset:
.highlight_style(Style::new().add_modifier(Modifier::REVERSED))
```

### Focused vs unfocused panes

```rust
fn block_style(focused: bool, theme: &Theme) -> Style {
    if focused { theme.border_focused } else { theme.border }
}
```

Apply to `Block::new().title_style(block_style(focused, theme))`.

### Dim for background views

```rust
let style = if in_foreground { Style::default() } else {
    Style::default().add_modifier(Modifier::DIM)
};
```
