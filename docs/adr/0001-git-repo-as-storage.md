# Git repository as task storage backend

Tasks are stored as markdown files with YAML frontmatter in a dedicated git repository, not in a local database. Each developer owns a folder named after their git username. The remote is the team's shared view — pulling is how you see teammates' work.

This was chosen over a local database (SQLite, etc.) because the storage is inherently portable, human-readable, and requires no migration tooling. The git history is a free audit trail. The team-visibility model falls out naturally from normal git push/pull without any server infrastructure.

## Considered Options

- SQLite with a sync mechanism — rejected because it requires a custom sync protocol and a server or conflict-resolution strategy beyond what git already provides.
- Plain files without git — rejected because versioning, team sharing, and conflict detection are git's core value.
