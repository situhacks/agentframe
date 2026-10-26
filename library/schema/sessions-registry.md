# Sessions registry

Shapes `system/sessions/registry.md` (runtime state, gitignored). Owned by `af session`; never edit it by hand.

```
| name | role | session_id | model | started | last_seen | status | handoff |
|---|---|---|---|---|---|---|---|
| af-main | main | <uuid> | opus | 2026-10-25T09:12 | 2026-10-25T17:40 | live | system/sessions/af-main/handoff.md |
```

- One row per name. Re-registering a name replaces its row and keeps its folder.
- `name`: the chat's name as other chats address it with `SendMessage`: the VS Code session title (set with `/rename`), or `claude --name` from a terminal. One of `af-main`, `life/<topic>`, `project/<slug>`, `system/<topic>`, or `worker/<slug>`; a segment is `[A-Za-z0-9][A-Za-z0-9_.-]*`. The main chat is never named `main`, which `SendMessage` reserves for the sender itself; a legacy `main` row still resolves, and its session start asks for the rename to `af-main`.
- The main chat is the row whose role is `main` and whose name has no `/`; `life/*` rows share the role but are side chats.
- `role` follows the prefix: `af-main`, a legacy `main`, and `life/*` are `main`, `project/*` is `operator`, `system/*` is `builder`, `worker/*` is `worker`.
- `model`: what main proposed, or `-`.
- `status`: `live` (a hook touched the row within two hours), `idle` (older, not ended), or `ended <ts> <reason>` with reason `clear`, `resume`, `logout`, `exit`, or `other`.
- Timestamps are local ISO minutes.

Each name has a folder, `system/sessions/<name>/`, holding an optional `brief.md` ([`handoff.md`](handoff.md) has both shapes), the chat's `handoff.md`, and for `system/*` and `worker/*` an optional `report.md`.

| Verb | Effect |
|---|---|
| `af session register <name> --session <id> [--model <m>] [--brief <path>]` | Writes the row, creates the folder, copies the brief |
| `af session list [--json]` | Prints the rows with their effective status |
| `af session stop`, `af hook stop` | Stamp `last_seen`; ask for a handoff when it is stale (never for `main`) |
| `af session end`, `af hook session-end` | Set `ended` |
| `af hook session-start`, `af hook notification` | Stamp `last_seen`; session start also revives an ended row |
