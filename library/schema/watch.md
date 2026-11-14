# Watch queue

Shapes `workspace/life/watch.md`, the queue of timed and conditional checks. Owned by `af watch`; headings and prose around the rows are free. A chat queues a check with `af watch add` and never sets its own timer. The batch run, `system/tools/watch_batch.py`, works through the queue three times a day, raises a loop and sends a phone message when a condition is met, and removes the row.

- If he must do something, it is a [loop](open-loops.md), "remind me" included; if a chat must check, it is a watch.
- The queue is checked at 10:00, 15:00 and 19:00 and never oftener. At 10:00 the run also does the morning jobs (bills watch, loop watch, loop sweep, the harness review, then the loop push); 15:00 and 19:00 check the queue only.
- The v1 actions are `notify` and `loop`; `notify+loop` does both.
- A phone message goes to the `#reminders` channel through `system/tools/notify.py`, so `REMINDERS_DISCORD_CHANNEL_ID` must be in `system/tools/.env`. Without it the run still checks and raises loops, and the report says the phone is not configured.

## Row grammar

One list line, fields joined by ` · ` in this order; `after` and `then` are optional.

```
- [ ] w-20261113-04 · owner life/travel · added 2026-11-13 · after 2026-11-15 · expires 2026-12-01 · when email "from:rail.example subject:refund" means "the rail company confirms the refund for the cancelled trip was issued" · do notify+loop "Rail refund confirmed" · then "Check the card shows the credit" · tier haiku · checked 2026-11-16 10:04 · result not met · status open
```

- `id` is `w-YYYYMMDD-NN`: one more than the highest number used that day, in the queue or the month log, so a closed id is not reused.
- `owner` is the chat that queued it: `af-main` when `--owner` is omitted (af cannot tell which chat called), otherwise a name in the session registry in any status (`af session list`); never `worker/*`.
- `expires` defaults to 14 days out, or a later date's day plus one, and is at most 90 days out: a computed default past that is clamped to 90 days and `add` says so, while an explicit `--expires` past it is refused. An open row past its expiry is swept into a loop.
- Free text (`do`'s message, `then`, a query, a statement, a path, a loop's text) is a JSON string literal holding no ` · ` and no newline. The message is 120 characters at most, `then` 160, and the rest 300.
- `tier` follows the kind: `email` is `haiku`, every other kind is `code`.
- `checked` is `never` or `YYYY-MM-DD HH:MM`; `result` is `none`, `not met`, `met: <detail>`, `error: <detail>` or `error twice: <detail>` (a detail is one line of 200 characters at most, and every `·` in it becomes `;`); `status` is `open`, `firing` or `met`.

## Conditions

| `when` | Met when |
|---|---|
| `date D [HH:MM]` | the first run at or after that moment; a bare date is midnight |
| `loop closed "<exact text>"` | the month log holds `closed: <exact text> → …` written on or after the day the item was added, which `af open close` and `af needs close` write (a recurring loop's close too). A loop that was only re-dated, reworded, moved or deleted is not closed |
| `loop open-on D "<text>"` | today is on or after D and an open loop has exactly that text |
| `file exists "<path>"` | the path exists |
| `file changed "<path>" sha256 <hex\|none>` | the file's streamed hash differs from the stored one (`none`: the file was absent), or the file was deleted after a hash was stored; a path that cannot be read is never met |
| `email "<query>" means "<statement>"` | a Haiku judge finds a thread in the Gmail search that proves the statement, and the email it names passes the runner's checks in code (below) |

Paths are absolute or relative to the vault. `loop open-on` reads open.md through the same parser as `af open`.

An email is met only on the evidence the [loop watch](loop-watch.md) needs to settle a loop: the quote is in the email's own text, not quoted history; its sender and subject are the ones the judge named; it dates from the item's window; no newer message in its thread was fetched; and it is known not to be his own mail (a `SENT` label or an address in `workspace/life/systems/loop-watch-self.txt` makes it his; with neither label nor file nothing can tell, and that is not met). When the query has a `from:` term, the email's own sender address must match it as Gmail does (the full address, or the domain or one under it), or the answer fails. The detail is `email dated <YYYY-MM-DD>`; the sender, subject and quote appear only in the run's report, inside code spans.

## Status

`open` is queued. A run that finds the condition met records `met: <detail>` and sets `firing`; it then raises the loop and sends the phone line, and marks the row `fired`, which sets `met`. The next sweep removes a `met` row after logging it. A `firing` row left by a crashed run is finished by the next run without a second phone message.

A met item never vanishes. Its loop is raised whenever its phone message did not provably go out, whatever its action says: the phone is not configured, the send or the loop push failed, or its line was kept off Discord or fell past the 15-line cap. A message provably went out only when the notify call (or the loop push carrying it) returned success with its line shown. That loop's text ends `phone message not sent`. Recovery of a `firing` row raises its loop whatever its action, ending `phone message not confirmed sent` for a notify action; the `[<id>]` check keeps both from raising a second loop.

## Slots

The batch run, `system/tools/watch_batch.py` (task `AgentFrame batch`, started by `watch-batch.ps1`), goes at 10:00, 15:00 and 19:00 and does this in order:

1. `af watch sweep`: expired rows become loops, `met` rows are removed.
2. Recovery: a `firing` row gets its loop if open.md lacks `[<id>]`, whatever its action, then `fired`; never a phone message.
3. Morning jobs, on the first run from 09:55 that has not done them today (`system/logs/watch-batch-state.json`): the bills watch, the loop watch, the loop sweep. Each is its own process with its own caps, report and log; one failing does not stop the next. A step that exits 3 finished partly (the loop watch, when some loops failed on their own and others were checked): the batch report's step line says `partial (exit 3)` with the step's footer, a note points at the step's own report, and it does not fail the run. Any other non-zero exit is a failure. Each is killed with every process it started past its limit: bills watch 130 minutes, loop watch 105, loop sweep 30, loop push 10. The loop watch is started with `--deadline-minutes 90`, so it stops starting loops first and still writes its report.
4. The queue: code rows by `code_met`; email rows oldest-checked first, 12 at most, one model call each (Haiku, then one retry on Sonnet), the answer validated in code against what the call fetched. A second failure is `error twice`, which sends one phone line and is not repeated on later slots. The queue's model calls may spend $0.75 a run: a call starts only if what is spent plus that call's own budget stays within it. A first try the cap stops is deferred with `checked` unchanged; a retry it stops is `error`.
5. A met row is set `firing`, raises the loop `Watch <id> met for <owner>: <say>; <detail> [<id>]` (nudge today, from `watch`, section `Watch`; the `then` text follows the detail), and joins the run's phone lines; then the message goes, any row whose message did not provably go out gets its loop, and the rows are marked `fired`. Email words never reach a loop or the phone.
6. The phone message: at the morning run the loop push (`push_due_loops.py --lead`) carries the watch lines first, so the day is one message; other runs send only when there are lines. Each line passes the off-Discord filter or shows only in a count, and at most 15 go out. A run that began before 09:55 and finds it past 09:55 once the queue is done (the scheduler drops a trigger that comes while a run is going) does the morning jobs then, and the push carries its lines.
7. Last, on every run and after everything above: `af index update` (`system/tools/index_update.py`, 240 seconds at most, no model call), so the search index is never more than a run old. If it fails or times out the report gets one line, `search index refresh failed (exit N): <last line>`; the exit code and every other step are as they would have been. A dry run does not start it.

A run takes `system/logs/watch-batch.lock`, holding its pid and start; a lock under six hours old makes the next run skip, and a run removes the lock only while it is still its own. The task's execution limit is also six hours, over every step at its limit plus 30 minutes for the queue. A run writes `system/logs/watch-batch/<date-time>.md` (the last 60 stay) and one line in `watch-batch.log`, and exits 1 if a step failed (not one that exited 3, partial), an item hit `error twice`, or the queue could not be read. A missing phone channel changes none of that: the report says `phone: not configured (REMINDERS_DISCORD_CHANNEL_ID)` and the loop push waits for a run that can send it. `--dry-run` lists what is due, an email item as `would judge` with its query; it calls no model, reads no Gmail, cleans no shared folder, takes no lock, and writes and sends nothing.

## Verbs

| Verb | Effect |
|---|---|
| `af watch add <one condition> --do <notify\|loop\|notify+loop> --say "<text>" [--expires <date or +Nd>] [--after <date or +Nd>] [--then "<text>"] [--owner <chat>]` | Appends a row and prints it. A condition is one of `--date <YYYY-MM-DD[THH:MM]>`, `--loop-closed <n or text prefix>`, `--loop-open <n or text prefix> --on <date>`, `--file-exists <path>`, `--file-changed <path>`, `--email "<query>" --means "<statement>"`. |
| `af watch list [--owner X] [--due] [--json]` | Rows; `--due` keeps open rows whose `after` is today or earlier and whose expiry is today or later, plus every `firing` row. The JSON gives every field and `code_met` (true or false for a code kind, null for email). |
| `af watch result <id> <not-met\|error\|error-twice\|met\|fired> [--detail "<text>"]` | The runner only. Each outcome but `fired` stamps `checked`; `met` sets `firing`; `fired` moves `firing` to `met`. |
| `af watch close <id> --outcome "<text>"` | Logs `event: watch <id> closed: <say> → <outcome>` in the month log and removes the row; a `firing` row is refused. |
| `af watch sweep [--json]` | Each open row past its expiry is logged, raised as the loop `Watch <id> expired without firing for <owner>: <say> [<id>]` (nudge today, from `watch`, section `Watch`), and removed; each `met` row is logged and removed; `firing` rows stay. A loop already carrying `[<id>]` is not raised again. |

`add` refuses: no condition or several; an expiry in the past, an explicit one over 90 days out, or one before `--after` or the condition's date; a condition date over 90 days out; `--after` with `--date`; a date already passed; a file that already exists; a loop that is missing or matches several; empty, over-long or ` · ` text; a bad, unregistered or `worker/*` owner; a file that cannot be read; and an open row with the same owner and condition (the refusal names its id).

Every verb that writes `watch.md`, and every `af open` and `af needs` verb that writes `open.md`, holds a lock file beside it (`watch.md.lock`) for its read, change and write, writes through a temp file and a rename, and waits about five seconds for a lock another writer holds before refusing, naming the lock file; a lock over two minutes old is stale and is replaced. A row is written only if it parses back to the same item, and `list` says on stderr how many rows did not parse.

`af doctor` notes a row that does not parse, an open row more than a day past its expiry, a `firing` row older than a day (the note names `af watch result <id> fired`, to run once its loop is confirmed in open.md), no batch run in 30 hours (`system/logs/watch-batch.log`), and a missing phone key. Main's session start carries one counts-only line when the queue is stuck.
