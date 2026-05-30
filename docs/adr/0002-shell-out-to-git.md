# Shell out to git instead of using the git2 crate

All git operations (pull, add, commit, push, mv) are performed by shelling out to the system `git` binary via `std::process::Command`, not via the `git2` libgit2 bindings.

The target user is a developer who already has git installed and configured — SSH keys, credential helpers, GPG signing, and proxy settings all work automatically when shelling out. Reimplementing credential handling in `git2` is the primary source of pain in git-backed desktop apps, particularly SSH agent integration on Windows and macOS Keychain. The name "git-task" makes git a stated prerequisite; users who cannot install git are not the target audience.
