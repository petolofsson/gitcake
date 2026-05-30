# Cargo workspace with a shared core library

The project is structured as a Cargo workspace. `gitcake-core` contains all business logic. All other crates — TUI, CLI, MCP server, GUI — are frontends that depend on core and contain no business logic.

```
gitcake-core    ← business logic, no UI dependencies
gitcake-tui     ← ratatui TUI (v1)
gitcake-cli     ← CLI subcommands (planned)
gitcake-mcp     ← MCP server for AI integration (planned)
gitcake-gui     ← Tauri GUI (future)
```

This makes the open-source extension model explicit: anyone can build a new frontend — a VS Code extension, a Neovim plugin, a web dashboard, an AI agent — by depending on `gitcake-core` without forking the whole project. It also allows each frontend to be developed and released independently, and keeps the core library testable in isolation.

The alternative — a single crate with logic and UI intertwined — was rejected because it would make community frontends impractical and the core logic harder to test.

## Rule

`gitcake-core` must never import `ratatui`, `crossterm`, `tauri`, or any other UI/platform crate. See CODERULES.md Rule 9.
