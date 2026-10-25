---
name: upgrade
description: The self-repair loop. Lists the open backlog rows, hands each picked row to a builder in its own worktree, shows the plain-language report and the diff, and merges on the operator's yes.
disable-model-invocation: true
---

Upgrade mode. The operator picks backlog rows; a `builder` fixes each in its own worktree; nothing lands without his yes. Row shape: [`backlog.md`](../../../library/schema/backlog.md).

1. Read `system/backlog.md`. Every `BB-` row without a `Status: done` line is open; a row in an older shape counts the same. If none is open, say so and stop.
2. Show the open rows as a lettered list (id, title, `Fix:` line) and ask which to run: letters, or `all`. Wait for his answer.
3. For each pick, one at a time, dispatch the `builder` role. Its brief carries:
   - the row text whole, its `Verify:` line as the check (an older row without one: the builder writes one and says so);
   - "Write `system/logs/report.md` in your worktree, in plain language: what changed, why, how it was verified, what to watch. Commit nothing. Return the worktree path, its branch, and the report text."
4. When it returns, run `git -C <worktree> add -A`, then `git -C <worktree> diff --cached --stat`. Show the report and the stat; ask: **a. Merge** or **b. Discard**. Wait for his answer.
5. On **a**:
   1. Copy the report to `system/logs/upgrade-<BB id>.md` in this checkout.
   2. `git -C <worktree> commit -m "<row title>" -m "<BB id>: <the report's what-changed sentence>"`.
   3. From this checkout, `git merge --ff-only <branch>`; if that refuses, `git merge --no-ff <branch> -m "Merge <BB id>: <row title>"`. If the merge stops (a conflict, a local edit it would overwrite), run `git merge --abort` if one started, show him the message, keep the worktree and the row, and go on.
   4. Move the row whole from `system/backlog.md` to the bottom of `system/backlog-completed.md` and add `Done: <today> · <short hash of HEAD>`.
   5. `git worktree remove <worktree>`, then `git branch -d <branch>`.
6. On **b**: copy the report to `system/logs/upgrade-<BB id>.md`, then `git worktree remove --force <worktree>` and `git branch -D <branch>`. The row stays open.
7. After the last pick, give one line per row: merged (hash), discarded, or stopped.

Never push, and never merge without his yes for that row. A builder that returns no worktree changed nothing: show its report and leave the row open.

Not built: a cloud variant. When the operator opts in, the same brief can run as a cloud agent on the tracked repository; it never sees `workspace/` or the backlog, so the brief carries the row text, and the work lands as a branch he reviews and merges.
