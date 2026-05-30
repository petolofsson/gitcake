# CODERULES

These rules apply to every crate. A pull request that violates any rule does not merge.

---

**Rule 1 — Simple control flow**
No recursion. No nested loops deeper than two levels. If control flow requires a comment to follow, extract a named function.

**Rule 2 — Bounded loops**
Every loop must have an obvious termination condition. Prefer iterator-based loops over index-based. No loop that could spin forever without a documented bound.

**Rule 3 — Short functions**
No function longer than 50 lines (blanks and comment-only lines excluded). Extract helpers whose names explain what they do.

**Rule 4 — No allocation in the render path**
Construct `Vec`s and `String`s in event handlers and data-loading code, not inside render functions. Render functions receive references and produce draw calls.

**Rule 5 — Handle every error**
No `unwrap()` in non-test code. Every `Result` is propagated with `?` or explicitly matched. `let _ = expr` requires a comment. `expect()` only in tests or provably unreachable states.

**Rule 6 — Minimal scope**
Declare variables as close as possible to their use. `mut` requires justification. All mutable state lives in `App` — no `static mut`, no `thread_local!` mutation.

**Rule 7 — Validate at boundaries, trust inside**
Validate user input and external data at the point they enter the system. Do not re-validate internally.

**Rule 8 — Compile clean**
Zero warnings under `cargo build`. No `#[allow(dead_code)]` or `#[allow(unused_*)]` without a comment. CI treats warnings as errors.

**Rule 9 — core/ has no UI dependencies**
`gitcake-core` must never import `ratatui`, `crossterm`, or any frontend crate. All behavioral logic (status transitions, ID assignment, sync, error types) belongs in `core/`.

**Rule 10 — Sync is always explicit**
The app never initiates a git operation automatically. Every `git pull`, `git push`, and `git commit` must be triggered by a deliberate user action or confirmed prompt.

---

**No AI attribution in commits.** Commit messages must not mention AI, Claude, or contain `Co-Authored-By` lines.
