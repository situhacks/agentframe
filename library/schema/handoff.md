# Brief and handoff

Shapes the two files in `system/sessions/<name>/` (runtime state, gitignored). The chat writes both by hand; `af hook session-start` prints them and `af hook stop` checks the handoff.

**`brief.md`**, written by main or the operator before the chat starts: frontmatter `goal` (one line), `done_when` (the check that ends the job), `write_surface` (a list of vault-relative paths), `model` (`opus` or `sonnet`), `window` (tokens); the body is free.

**`handoff.md`**, written by the chat before it stops. These four sections, in this order, and no others:

```
---
written: 2026-10-25T17:38
session_id: <uuid>
---
## State
<where the work is, five lines or fewer>
## Open questions
## Next step
## Files touched
<one vault-relative path per line>
```

A handoff is stale when the newest mtime among `Files touched` (or, with none listed, under the brief's `write_surface`), floored to the minute, is later than `written`. The Stop hook then blocks once for `project/*`, `system/*`, and `worker/*` chats, never for `main`. `af doctor` notes a handoff older than its folder's newest file.
