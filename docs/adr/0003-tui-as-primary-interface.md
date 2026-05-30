# TUI as primary v1 interface (ratatui)

The primary v1 interface is a ratatui terminal UI (`cake` binary), not a GUI desktop app. It is designed to run in a terminal split pane alongside other developer tools — Claude Code, an editor, or a shell — keeping slices permanently visible in the developer's native workspace.

A GUI (floating widget, Tauri app) was the original design. It was replaced because the target user — a terminal-native developer — already has a terminal open at all times. A split pane in that terminal is more persistently visible than a floating GUI window, works over SSH, requires no desktop permissions, and ships as a single binary with no system dependencies beyond the Rust toolchain.

The GUI is preserved as a future deliverable (`src-tauri/`) and may be built by the community using `gitcake-core` as a library.

## Interface roadmap

1. **TUI** (`cake`) — v1, interactive, humans
2. **CLI subcommands** (`cake list`, `cake create`, etc.) — planned, non-interactive, scripts and AI agents
3. **MCP server** (`gitcake-mcp`) — planned, typed AI tool integration
4. **GUI** (`gitcake-gui`) — future, floating widget
