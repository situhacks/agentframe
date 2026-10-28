# Open loops

Shapes `workspace/life/open.md` and the closed lines in `workspace/life/log/<YYYY-MM>.md`. Owned by `af open`; headings and prose around the lines are free. Open loops are the system's one reminder mechanism.

```
# Open loops

## Waiting on the operator

- [ ] 2026-10-24 · renew the lease · nudge 2026-10-27 · from: src-2026-10-23-02
- [ ] 2026-10-24 · pay the credit card · nudge 2026-11-15 · every monthly · from: src-2026-10-24-01

## Waiting on others
```

## The loop rule

- Raise a promise, a question for someone, or a waiting item with `af open add`, citing its source or chat in `--from`. A recurring need is one loop with `--every`.
- Close a loop yourself with `af open close` and an outcome that says how, once the work is done or the question answered, and name it in your reply; re-date one whose date has plainly moved. A loop that needs the operator's decision waits for him.
- An interactive chat (main or a side chat) re-raises due loops at the end of each reply, with their background, until resolved. A worker writes no replies; it records a due loop in its handoff instead.
- Never keep a deadlines or reminders file inside a system or project folder; each date there is a loop.

Due loops print at the start of every registered interactive chat and after compaction, and reach his phone each morning through `system/tools/push-due-loops.ps1`, which `system/tools/register-daily-reminders.ps1` schedules.

## Grammar

- A loop is one list item: `- [ ] <opened YYYY-MM-DD> · <text> · nudge <YYYY-MM-DD> · every <period> · from: <source id | chat name>`. `nudge` is optional; without it the loop never nudges. `every` is optional, follows a nudge, and is `monthly`, `weekly`, or `Nd` (N days).
- Every list item under any heading must parse; `af doctor` reports one that does not, and any `[x]` line left in the file.
- Nothing closed stays in `open.md`. `af open close` appends `- YYYY-MM-DD HH:MM · closed: <text> → <outcome>` to the month log, then removes the line; on a recurring loop it re-dates the line one period on instead and the log line ends `(next nudge YYYY-MM-DD)`, and `--final` ends it, the log line ending `(final)`.
- Monthly is the same day next month, clamped to the month's last day; a loop dated the 29th to 31st settles on the 28th after a short month, so put bills on the 28th or earlier.
- The month log is append-only: `- YYYY-MM-DD HH:MM · <kind>: <text>`, kind one of `closed`, `redated`, `decision`, `event`, `project_closed`, `session_ended`.

| Verb | Effect |
|---|---|
| `af open add "<text>" [--nudge <date or +Nd>] [--every <monthly, weekly, or Nd>] [--from <id>] [--section "<heading>"]` | Appends a line with today's date (default section `Waiting on the operator`); `--every` needs `--nudge` |
| `af open close <n or text prefix> --outcome "<text>" [--final]` | Moves the loop to the month log, or re-dates a recurring one a period on; `--final` ends a recurring loop; refuses an ambiguous prefix |
| `af open redate <n or text prefix> --nudge <date or +Nd>` | Changes the nudge date in place; opened date, text, period, section, and source stay; the log records it |
| `af open due [--json]` | Loops nudged today or earlier, oldest first, with their age and period |
