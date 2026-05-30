# git-task — Known Issues & Improvements

Captured during 100-user stress analysis. Ordered by impact.

---

## Critical — breaks core usage

- [ ] **Editor trap on first use**
  `$EDITOR`/`$VISUAL` not set → falls back to `vi`. Users unfamiliar with vi have no way out.
  Fix: default to `nano`; detect if neither env var is set and show a one-time hint on how to save/quit.

- [ ] **Silent auth failures at startup**
  Pull fails if SSH key not loaded, VPN not connected, or token expired. Error flashes in header and is cleared by the next keypress — user has no idea their state is stale and push will also fail later.
  Fix: keep auth/pull errors visible until explicitly dismissed; distinguish "no network" from "auth failure".

- [ ] **One bad task file kills the entire list**
  Malformed YAML frontmatter in a single `.md` file causes `list_tasks()` to fail — all tasks vanish with no explanation.
  Fix: skip unreadable files, collect parse errors, and show a warning ("1 task could not be read: alice-smith/003.md").

- [ ] **Terminal close bypasses push prompt**
  Alt+F4 or closing the terminal tab skips Ctrl+Q entirely. Work since last sync is silently lost.
  Fix: document prominently; consider writing a `.lock` file on open and cleaning up on exit so users can detect incomplete sessions.

- [ ] **Git conflict has no in-app resolution path**
  Two users editing the same backlog task, or same user on two machines, causes push to fail with a raw git error string. No retry, no guidance.
  Fix: detect conflict errors specifically, show a clear message with resolution steps ("run `git pull` then `gt` again").

---

## High — degrades experience at scale

- [ ] **Cursor resets to position 0 after every action**
  Every status cycle, edit, create, sync, or delete resets the selected index to the top of the list. Marking 10 tasks done in a row means jumping back to the top each time.
  Fix: remember selected task by ID, restore position after reloading the list.

- [ ] **Non-ASCII usernames break folder names**
  `git config user.name = "José García"` → `josé-garcía`. Fails on some filesystems (older macOS HFS+, some Windows configs). Emoji or CJK names are worse.
  Fix: normalize to ASCII (transliterate or strip), or at minimum validate and block with a clear error.

- [ ] **Long task titles overflow the row layout**
  A 200-char title runs past the terminal edge. Emoji titles (double-width characters) misalign columns.
  Fix: truncate titles to available width with `…`; handle double-width characters in column calculations.

- [ ] **No manual pull mid-session**
  If a teammate pushes while you're working, there's no way to refresh without restarting `gt`.
  Fix: add `Ctrl+P` to pull and reload the task list.

- [ ] **Small terminal windows garble the UI**
  Below ~60 columns, block borders overlap content and command bars wrap unpredictably.
  Fix: detect terminal size on render; show a "terminal too small" message below minimum dimensions (e.g. 60×20).

---

## Medium — noticeable friction

- [ ] **"DONE (local)" label is wrong for synced tasks**
  After sync, tasks move to `completed/` and `is_completed = true` but still appear under "DONE (local)".
  Fix: separate into "DONE (local)" (pending sync) and "DONE (synced)" or just "DONE" — removing the label when already synced.

- [ ] **No way to view other team members' tasks**
  Core product promise is team visibility, but there's no UI to browse other users' folders.
  Fix: add a "team view" mode that lists all user folders and their active tasks (read-only).

- [ ] **No way to change repo path without editing config manually**
  Once set, the repo path in `~/.config/git-task/config.toml` can only be changed by editing the file directly.
  Fix: add a "change repo" option reachable from within the app (e.g. from the setup screen or a settings screen).

- [ ] **Backlog assignee not validated**
  You can assign a backlog task to a username that has no folder in the repo. Silently succeeds.
  Fix: picker already enforces valid users — ensure backlog assign always uses the picker, never free text.

- [ ] **No scroll position memory when returning from detail view**
  Viewing a task detail and pressing back returns to position 0 in the list, not to the task you came from.
  Fix: pass the current selected index back when returning from detail view.

- [ ] **Repo name and current user not shown in header**
  The header just says `git-task`. You don't know which repo you're connected to or who you are.
  Fix: show `git-task · repo-name · username` in the header.

- [ ] **No task count per group**
  No indication of how many tasks are in each status group.
  Fix: show counts in section headers, e.g. `● IN PROGRESS (3)`.

---

## Low — polish

- [ ] **No retry button after failed push/sync**
  When sync fails, the error shows in the header but the user has to know to press Ctrl+R again.
  Fix: show explicit retry hint in the header error message.

- [ ] **Backlog done tasks never move to completed/**
  Personal done tasks move to `completed/{username}/` on sync. Backlog done tasks stay in `backlog/` forever.
  Fix: decide policy — move to `completed/backlog/` on sync, or leave in backlog and just mark done.

- [ ] **`launched_editor` flag is unused in create form**
  Minor dead code warning suppressed. Cleanup.

- [ ] **`render_help` only used by two screens**
  Setup and init-repo use the old right-aligned border help pattern. Should be migrated to the standard nav/ctrl bars for full uniformity.

- [ ] **Concurrent gt instances see stale state**
  Two `gt` windows open on the same repo — changes in one don't appear in the other until sync.
  Fix: document as known limitation; optionally add a file-watcher to detect external changes.
