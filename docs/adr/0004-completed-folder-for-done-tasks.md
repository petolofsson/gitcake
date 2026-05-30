# completed/ folder for done and synced slices

**Status: SUPERSEDED** by the v2 storage redesign (type-based folders + owner field).

---

When a slice is marked done and synced, its file was moved from `{username}/` to `completed/{username}/` via `git mv`. It was not deleted and not hidden — only moved.

A UI-only filter was rejected because the active folder would accumulate done slices indefinitely, making it noisy for anyone browsing the repo directly. Deletion was rejected because it removes team visibility — a team lead pulling the repo would see nothing. The `completed/` folder kept done work permanently visible to the team while keeping each developer's active folder clean. The move happened on sync (not on mark-done) so the file was never in an inconsistent location between a status change and a push.

## Why superseded

The v2 storage redesign (see the `feat: v2 storage` commit) eliminated personal folders and the completed/ folder entirely. All slices now live in type-based folders (`tasks/`, `bugs/`, `incidents/`). Status is frontmatter-only — files never move after creation. "Done" is simply `status: done` in the frontmatter; no git mv is needed and history is cleaner. Ownership is an `owner:` frontmatter field rather than a directory.

The original concerns (noise in active folder, team visibility) are addressed differently: the personal view filters by owner field, and all slices remain permanently in their type folder regardless of status.
