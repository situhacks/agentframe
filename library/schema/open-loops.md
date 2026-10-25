# Open loops

Shapes `workspace/life/open.md` and the closed lines in `workspace/life/log/<YYYY-MM>.md`. Owned by `af open`; headings and prose around the lines are free.

```
# Open loops

## Waiting on the operator

- [ ] 2026-10-24 · renew the lease · nudge 2026-10-27 · from: src-2026-10-23-02

## Waiting on others
```

- A loop is one list item: `- [ ] <opened YYYY-MM-DD> · <text> · nudge <YYYY-MM-DD> · from: <source id | chat name>`. `nudge` is optional; without it the loop never nudges.
- Every list item under any heading must parse; `af doctor` reports one that does not, and any `[x]` line left in the file.
- Nothing closed stays in `open.md`. `af open close` appends `- YYYY-MM-DD HH:MM · closed: <text> → <outcome>` to the month log, then removes the line.
- The month log is append-only: `- YYYY-MM-DD HH:MM · <kind>: <text>`, kind one of `closed`, `redated`, `decision`, `event`, `project_closed`, `session_ended`.
- Only the operator closes or re-dates a loop; an agent proposes the closure.

| Verb | Effect |
|---|---|
| `af open add "<text>" [--nudge <date or +Nd>] [--from <id>] [--section "<heading>"]` | Appends a line with today's date (default section `Waiting on the operator`) |
| `af open close <n or text prefix> --outcome "<text>"` | Moves the loop to the month log; refuses an ambiguous prefix |
| `af open redate <n or text prefix> --nudge <date or +Nd>` | Changes the nudge date in place; opened date, text, section, and source stay; the log records it |
| `af open due [--json]` | Loops nudged today or earlier, oldest first, with their age |
