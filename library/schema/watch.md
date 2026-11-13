# Watch queue

Shapes `workspace/life/watch.md`, the queue of timed and conditional checks. Owned by `af watch`; headings and prose around the rows are free. A chat queues a check with `af watch add` and never sets its own timer. The batch run, `system/tools/watch_batch.py`, works through the queue three times a day, raises a loop and sends a phone message when a condition is met, and removes the row.

- If he must do something, raise a [loop](open-loops.md). If a chat must check or ping, add a watch.
- The queue is checked at 10:00, 15:00 and 19:00 and never oftener. At 10:00 the run also does the morning jobs (bills watch, loop watch, loop sweep, then the loop push); 15:00 and 19:00 check the queue only.
- The v1 actions are `notify` and `loop`; `notify+loop` does both.
- A phone message goes to the `#reminders` channel through `system/tools/notify.py`, so `REMINDERS_DISCORD_CHANNEL_ID` must be in `system/tools/.env`. Without it the run still checks and raises loops, and the report says the phone is not configured.

## Row grammar

One list line, fields joined by ` · ` in this order; `after` and `then` are optional.

```
- [ ] w-20261113-04 · owner life/travel · added 2026-11-13 · after 2026-11-15 · expires 2026-12-01 · when email "from:rail.example subject:refund" means "the rail company confirms the refund for the cancelled trip was issued" · do notify+loop "Rail refund confirmed" · then "Check the card shows the credit" · tier haiku · checked 2026-11-16 10:04 · result not met · status open
```

- `id` is `w-YYYYMMDD-NN`: one more than the highest number used that day, in the queue or the month log, so a closed id is not reused.
- `owner` is the chat that queued it: `af-main` when `--owner` is omitted (af cannot tell which chat called), otherwise a registered chat name; never `worker/*`.
- `expires` defaults to 14 days out, or a later date's day plus one, and is at most 90 days out. An open row past it is swept into a loop.
- Free text (`do`'s message, `then`, a query, a statement, a path, a loop's text) is a JSON string literal holding no ` · ` and no newline. The message is 120 characters at most, `then` 160, and the rest 300.
- `tier` follows the kind: `email` is `haiku`, every other kind is `code`.
- `checked` is `never` or `YYYY-MM-DD HH:MM`; `result` is `none`, `not met`, `met: <detail>`, `error: <detail>` or `error twice: <detail>`; `status` is `open`, `firing` or `met`.

## Conditions

| `when` | Met when |
|---|---|
| `date D [HH:MM]` | the first run at or after that moment; a bare date is midnight |
| `loop closed "<exact text>" nudge <date\|none>` | no open loop has that text, or its nudge has moved past the stored one (a recurring loop that was closed) |
| `loop open-on D "<text>" nudge <date\|none>` | today is on or after D and the loop is still open with its stored nudge |
| `file exists "<path>"` | the path exists |
| `file changed "<path>" sha256 <hex\|none>` | the file's hash differs from the stored one (`none`: the file was absent) |
| `email "<query>" means "<statement>"` | a Haiku judge finds a thread in the Gmail search that proves the statement; the runner validates the quote in code |

Paths are absolute or relative to the vault. Loop conditions read open.md through the same parser as `af open`; a loop line that no longer parses is not read as closed.

## Status

`open` is queued. A run that finds the condition met records `met: <detail>` and sets `firing`; it then raises the loop and sends the phone line, and marks the row `fired`, which sets `met`. The next sweep removes a `met` row after logging it. A `firing` row left by a crashed run is finished by the next run without a second phone message.

## Verbs

| Verb | Effect |
|---|---|
| `af watch add <one condition> --do <notify\|loop\|notify+loop> --say "<text>" [--expires <date or +Nd>] [--after <date or +Nd>] [--then "<text>"] [--owner <chat>]` | Appends a row and prints it. A condition is one of `--date <YYYY-MM-DD[THH:MM]>`, `--loop-closed <n or text prefix>`, `--loop-open <n or text prefix> --on <date>`, `--file-exists <path>`, `--file-changed <path>`, `--email "<query>" --means "<statement>"`. |
| `af watch list [--owner X] [--due] [--json]` | Rows; `--due` keeps open rows whose `after` is today or earlier and whose expiry is today or later, plus every `firing` row. The JSON gives every field and `code_met` (true or false for a code kind, null for email). |
| `af watch result <id> <not-met\|error\|error-twice\|met\|fired> [--detail "<text>"]` | The runner only. Each outcome but `fired` stamps `checked`; `met` sets `firing`; `fired` moves `firing` to `met`. |
| `af watch close <id> --outcome "<text>"` | Logs `event: watch <id> closed: <say> → <outcome>` in the month log and removes the row; a `firing` row is refused. |
| `af watch sweep [--json]` | Each open row past its expiry is logged, raised as the loop `Watch <id> expired without firing for <owner>: <say> [<id>]` (nudge today, from `watch`, section `Watch`), and removed; each `met` row is logged and removed; `firing` rows stay. A loop already carrying `[<id>]` is not raised again. |

`add` refuses: no condition or several; an expiry in the past, over 90 days out, or before `--after` or the condition's date; `--after` with `--date`; a date already passed; a file that already exists; a loop that is missing or matches several; empty, over-long or ` · ` text; a bad or `worker/*` owner; and an open row with the same owner and condition (the refusal names its id).

`af doctor` notes a row that does not parse, an open row more than a day past its expiry, a `firing` row older than a day, no batch run in 30 hours (`system/logs/watch-batch.log`), and a missing phone key. Main's session start carries one counts-only line when the queue is stuck.
