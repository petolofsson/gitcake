# TUI as primary interface (ratatui)

The primary v1 interface is a ratatui terminal UI (`gt` binary), not a GUI desktop app. It is designed to run in a terminal split pane alongside other developer tools — Claude Code, an editor, or a shell — keeping tasks permanently visible in the developer's native workspace.

A GUI (floating widget, Tauri app) was the original design. It was replaced because the target user — a terminal-native developer — already has a terminal open at all times. A split pane in that terminal is more persistently visible than a floating GUI window, works over SSH, requires no desktop permissions, and ships as a single binary with no system dependencies beyond the Rust toolchain.

The GUI is preserved as a future deliverable (`src-tauri/`) and may be built by the community using `git-task-core` as a library.
