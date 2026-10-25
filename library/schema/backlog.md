# Backlog

Shapes `system/backlog.md`, the open rows of system friction, and `system/backlog-completed.md`, the done rows. Both are private. The operator and any agent append rows; `/upgrade` hands them to a `builder` and moves them on merge.

```
# Backlog

## BB-2026-10-24-01 · af open has no way to re-date a loop
Surfaced: main, 2026-10-24, re-dating three loops during a catch-up
Fix: a verb that changes a loop's nudge date and keeps its opened date
Surfaces: system/af/src/open.rs, library/schema/open-loops.md
Verify: a test re-dates a loop; the line keeps its opened date and parses
```

- One row is the heading `## BB-YYYY-MM-DD-NN · <title>` and the four lines `Surfaced:`, `Fix:`, `Surfaces:`, `Verify:`, in that order. No YAML, no status line, no counters.
- `YYYY-MM-DD` is the day the row is written; NN is the next free number for that day across both files.
- `Surfaced:` is one line: which chat or file, when, and the evidence pointer (a log line, a transcript, a path).
- `Fix:` says what should change in words the operator can approve, not a patch.
- `Surfaces:` lists the vault-relative paths the fix will likely touch.
- `Verify:` is how a builder proves the fix: a test, a command and its expected output, a check shown failing first.
- `backlog.md` holds open rows only. A done row moves whole to `backlog-completed.md`, newest at the bottom, with one added line: `Done: YYYY-MM-DD · <commit>`.
- A row in an older shape (an em dash title, a `Status:` line, a `- id:` YAML item) is rewritten into this shape the next time it is touched; a `Status: done` row moves to the completed file.
