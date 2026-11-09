# Two-Machine Sync

## Purpose

How the operator's two machines stay in step through the hub folder with `af sync`, and how a chat resolves a conflict it meets. The hub is a folder on his home network holding two bare git repositories (`agentframe.git`, `workspace.git`) and `files/`, the copy list's mirror.

## When To Load

`af sync` printed `SYNC_CONFLICT`; the session-start line says to run `af sync`; he asks to sync or to set up a machine; or `af doctor` flags a remote on `workspace/.git`.

## Procedure

Set up, once per machine:

1. `af sync init <hub-folder> --dry-run`, then `af sync init <hub-folder>`. The hub is a folder path: `D:\agentframe-hub` or `\\<machine>\<share>`. A web, SSH or WebDAV (`\\<machine>@SSL\...`) address is refused, as is a path with one leading slash. The first machine creates the hub and pushes both repositories; on a joining machine with no `workspace/`, it clones the private layer.
2. From then on the hooks run by themselves: session start pulls the private layer and stop pushes it. A hook that cannot sync skips with one run-log line. Session start tells the chat when a merge is waiting, a lock was left behind, or several syncs in a row were skipped for a reason he must fix (an unreachable hub is not one); then run `af sync`, which says what to do.

Run `af sync`. It commits and merges the private layer, merges the public code only on a clean `master` (never commits it), copies the copy list newer-wins (never deletes), re-renders `needs.md`, and refreshes the index. A `merge skipped` line is information, not a failure. When the copy list replaces a file, it saves the old copy under `system/logs/sync-backups/` on this machine and prints where; if both machines added to the same file, merge the backup's lines back by hand.

`af sync` refuses when the hub remote resolves to anything but a folder (a `pushurl` or an `insteadOf` rule pointing at GitHub, for example) or sets a command (`receivepack`, `uploadpack`, `vcs`), and names what to remove. If it reports that git refuses the hub's repository for its ownership, run the one command it prints, once per machine.

Resolve a conflict (exit 3, one `SYNC_CONFLICT <path>` line per file, the merge left in progress):

1. Open each file. Between `<<<<<<<` and `=======` is this machine's text, between `=======` and `>>>>>>>` the other machine's. Keep every fact from both: for a loop edited on both, keep the later intent; for prose, combine the two.
2. Delete every marker line. For a binary file, pick one side with `git -C workspace checkout --ours <path>` (or `--theirs`), then `git -C workspace add <path>`. A file deleted on one machine and edited on the other comes back as the edited copy; to keep the deletion, delete the file (or `git -C workspace rm <path>`).
3. Rerun `af sync`. It stages each text file that has no markers left, records each deleted file as deleted, commits the merge, and pushes. In your reply, name each file and what you kept.

Rendered views (`life/needs.md`, `life/tags.md`) never conflict: the sync keeps this machine's copy, then re-renders. Append-only files (`sources/INDEX.md`, the month log, `tags-log.md`) keep both sides' lines. A public-code conflict is undone and reported; a builder chat merges `hub/master` by hand.

## Verification

`af sync` ends with `Done.` and exit 0, `git -C workspace status` is clean, and `af doctor` reports no remote issue.

## Boundaries

`system/sessions/`, the index, and `system/logs/` stay per machine. `workspace/.git` keeps no remote except `hub`, so GitHub never receives the private layer. A push of the public code to `origin` still needs his explicit yes.

What `workspace/.gitignore` ignores (video, audio, archives, Office files, project images, tax and finance documents) stays on the machine that has it; other images and PDFs under `life/` and `pipeline/` are tracked and travel. A link to a file that did not travel is not an error.

Backups under `system/logs/sync-backups/` are never pruned and hold plain copies of replaced files, `.env` included; he may delete old ones.
