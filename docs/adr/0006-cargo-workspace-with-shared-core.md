# Cargo workspace with a shared core library

The project is structured as a Cargo workspace with three crates: `git-task-core` (business logic), `git-task-tui` (TUI frontend), and `git-task-gui` (Tauri GUI frontend). All business logic lives in `core/` with no dependency on any frontend framework.

This was chosen to make the open-source extension model explicit: anyone can build a new frontend — a VS Code extension, a Neovim plugin, a web dashboard — by depending on `git-task-core` without forking the whole project. It also allows the TUI and GUI to be developed and released independently, and keeps the core library testable in isolation without a running Tauri or terminal environment.

The alternative — a single crate with the GUI and logic intertwined — was rejected because it would make community frontends impractical and the core logic harder to test.
