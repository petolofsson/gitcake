# completed/ folder for done and synced slices

When a slice is marked done and synced, its file is moved from `{username}/` to `completed/{username}/` via `git mv`. It is not deleted and not hidden only in the UI.

A UI-only filter was rejected because the active folder would accumulate done slices indefinitely, making it noisy for anyone browsing the repo directly. Deletion was rejected because it removes team visibility — a team lead pulling the repo would see nothing. The `completed/` folder keeps done work permanently visible to the team while keeping each developer's active folder clean. The move happens on sync (not on mark-done) so the file is never in an inconsistent location between a status change and a push.

## Consequences

- ID assignment must scan both `{username}/` and `completed/{username}/` to determine the next sequential ID.
- The app must look in both folders when loading a developer's full slice history.
- Backlog slices (`backlog/`) do not currently move to a completed folder — policy TBD.
