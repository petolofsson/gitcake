# CODERULES.md

Adapted from [The Power of 10: Rules for Developing Safety-Critical Code](https://en.wikipedia.org/wiki/The_Power_of_10:_Rules_for_Developing_Safety-Critical_Code) by Gerard Holzmann (NASA/JPL, 2006).

The original rules were written for flight software — where a bug means a dead spacecraft. gitcake is not flight software, but it runs against a practitioner's own data, daily, without a safety net. Complexity here means data loss, corrupted repos, and a tool no one trusts. These rules apply the same discipline: keep the code small enough to reason about, correct enough to trust.

---

## Rule 1 — Simple control flow

No recursion. No nested loops deeper than two levels. No `break` or `continue` in an inner loop that affects outer loop state. If the control flow of a function requires a comment to follow, extract a named function instead.

## Rule 2 — Bounded loops

Every loop must have a termination condition that is obvious to a reader at a glance. Prefer iterator-based loops (`for item in collection`) over index-based loops. The main event loop must have exactly one exit point per iteration body, reached by a single `break` or `return`. No loop that could spin forever without a documented bound.

## Rule 3 — Short functions

No function longer than 50 lines (blank lines and comment-only lines excluded). This applies to render functions, event handlers, and core methods equally. If a function approaches the limit, extract a helper whose name explains what it does — not what it is called from.

## Rule 4 — No allocation in the render path

Construct `Vec`s and `String`s in event handlers and data-loading code, not inside render functions. Render functions receive references to already-prepared state and produce draw calls. A frame that allocates on every tick will degrade under load and obscures where data is prepared versus where it is displayed.

## Rule 5 — Handle every error

No `unwrap()` in non-test code. Every `Result` is either propagated with `?` or explicitly matched. Discarding an error with `let _ = expr` requires an inline comment explaining why the failure is acceptable. `expect()` is allowed only in tests and for states that are provably unreachable — document why they are unreachable.

## Rule 6 — Minimal scope

Declare every variable as close as possible to its point of use. Mutable bindings (`mut`) require justification — prefer a functional transformation over in-place mutation. All mutable application state lives in `App`; no `static mut`, no `thread_local!` mutation, no hidden global state.

## Rule 7 — Validate at boundaries, trust inside

Validate user input and external data (files, git command output) at the exact point they enter the system. Once data has passed validation, trust it within the call chain — do not re-validate internally. Re-validation is noise that hides where the real boundary is.

## Rule 8 — Compile clean

All code compiles with zero warnings under `cargo build`. No `#[allow(dead_code)]` or `#[allow(unused_*)]` without a comment explaining why. Warnings are fixed immediately — a warning today is a misread type or a stale branch tomorrow. CI must treat warnings as errors.

## Rule 9 — core/ has no UI dependencies

`gitcake-core` must never import `ratatui`, `crossterm`, or any frontend crate. The core library is the open API: any developer should be able to build a frontend — CLI, GUI, MCP server, web — by depending on `core/` alone. Every behavioral rule (status transitions, ID assignment, sync logic, error types) belongs in `core/`, not in `tui/`, `cli/`, or `src-tauri/`.

## Rule 10 — Sync is always explicit

The application never initiates a git operation automatically. Every `git pull`, `git push`, and `git commit` must be triggered by a deliberate user action or a user-confirmed prompt. No background threads, no timers, no auto-save on close. A practitioner's task repo is their own data — the tool may not touch it without permission.

---

## Additional — No AI attribution in commits

Commit messages must not reference AI tools, Claude, or any automated author. No `Co-Authored-By:` lines. Every commit in this repository should read as a normal human developer commit.

---

These rules are not aspirational. They are the baseline. A pull request that violates any rule does not merge.
