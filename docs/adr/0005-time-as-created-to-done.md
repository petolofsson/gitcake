# Time measured as created→done, no running timer

Task duration is derived from the `created` and `done` timestamps already in the frontmatter. There is no running timer, no `time_spent` field, and no play/pause tracking.

The app's core purpose is focus — a visible reminder of what you are working on — not time tracking. A running timer adds implementation complexity (timer persistence across sessions, display, accumulation logic) for a metric that is secondary to the product's value. The `created`→`done` delta gives a rough cycle time suitable for retrospectives without any extra machinery.
