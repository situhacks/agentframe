# Loop sweep

Shapes `workspace/life/systems/loop-sweep-report.md`, the record of the morning sweep of his open loops; `workspace/life/systems/loop-sweep-overdue.json`, the overdue list it hands the 08:30 push; and the two private files it reads. `system/tools/loop_sweep.py` writes the report; `system/tools/loop-sweep.ps1` runs it once a day at 08:20 (`register-loop-sweep.ps1`), after the 07:50 [loop watch](loop-watch.md) and ahead of the 08:30 loop push, so the push and the board carry the swept list. Its job: a to-do list that stays current without his reply.

## What it does

1. **Checks, then closes on evidence.** A passed date is never evidence that something was done.
2. **Tidies.** Ends duplicates, merges small asks of one kind into one loop, and moves an idle to-do with zero stakes.
3. **Reminds.** An overdue loop with money to settle or a real deadline, and no evidence, stays open and due. It leads the 08:30 push each morning until it is settled.
4. **Publishes.** Rewrites the Discord #open board source, `workspace/life/systems/discord/boards/open.md`, and posts it with `system/tools/discord_board.py board open`.
5. **Records.** Appends a run section to the report and a line of counts to `system/logs/loop-sweep.log`.

## How each kind is verified

Closing is decided by an allowlist, not a list of risky words. A loop has **zero stakes** only when its text, normalised (NFKC, invisible characters removed, Cyrillic and Greek lookalikes read as Latin), passes all three:

- none of these as a substring: `pay`, `owe`, `back`, `bucks`, `dollar`, `money`, `venmo`, `zelle`, `paypal`, `cheque`, `check`, `settle`, `split`, `refund`, `buy`, `book`, `ticket`, `order`, `rent`, `tax`, `form`, `deadline`, `due`, `expire`, `renew`, `submit`, `appointment`, `appt`. Substrings, so `Repay`, `PayPal` and `prepay` fail on `pay`;
- no currency sign;
- no digit once the days it names (`2026-03-08`, `2026-03`, `03-08`, `3/8`, `Mar 8`, `8 Mar`, `the 8th`) and its source ids are cut.

Everything else is **guarded**, as is any loop the model gives a money or booking kind or says something rides on. Separately, a loop is **urgent** when its words name money to settle (`pay` and its compounds, `owe`, `refund`, `rent`, `tax`, `Venmo`, `cheque`, `settle`, a claim, an e-transfer, an amount of money in any notation that is not a price) or a real deadline (`due`, `deadline`, `expires`, `submit`, `renew`, `by <day>`), or the model says a deadline or a person waits on it. An urgent loop is reminded daily when overdue.

| Close | Allowed only when |
|---|---|
| Email, fixed sender | A loop watch `done` from Interac (`interac.ca`) for an e-transfer; from a card issuer in the bills ledger's Cards list for a money loop that names that card or its issuer; from a ticketing platform for a booking. Any loop, guarded or not. Bills loops are the [bills watch](bills.md)'s: its paid rule closes them, and the sweep only ends a hand-raised duplicate of one |
| Email, a sender the loop names | The loop has zero stakes |
| Vault line or file | The loop has zero stakes, and the line (with the file's name) or the path's last three parts shares at least two of the loop's own words; numbers do not count |
| Day passed | The loop's text starts with `talk`, `tell`, `ask`, `call`, `text`, `meet`, `say`, `see`, `visit` or `catch up`, it has zero stakes, the model calls it personal, and the day is before today and named in its text. It closes as `day passed, unconfirmed` |

**Which sender a loop names.** The main label of the sender's registered domain, as a whole word of the loop: the last two labels, or three under a two-part suffix such as `co.uk`. A sender at `mail.widget.example` names `widget`; one at `widget.evil.example` names `evil`. Generic labels (`shop`, `mail`, `billing` and the like) and labels under four letters name nothing. An address that is not a plain domain after its last `@` settles nothing.

- The loop watch's evidence counts only from today's report, and only on the exact line `- Verdict: done`. A verdict the watch capped to `update` settles nothing, and neither does a free mail address (Gmail, Outlook, iCloud and the like). The report is read by Python, never by the model.
- A loop's window opens on its opened date; for a recurring loop it opens on its current nudge, so last period's entry cannot settle this one. A recurring loop closed on evidence re-dates one period on, as `af open close` does.
- The calendar is not checked yet. A calendar check needs its own headless lockdown, built like the loop watch's with only the calendar's read tools allowed, before it can count as evidence.

## The rules Python holds

The model proposes a JSON plan; Python applies only what passes. Each refusal is in the report, naming the rule.

- **Owned loops.** Needs items, `Bills` loops and loops under `Waiting on others` take only a flag, or the kept side of a duplicate. The bills watch owns its loops, main chases what others owe, and a decision is his.
- **Duplicates.** The two loops share at least two distinctive words. The kept loop is the newer, or the one under Bills or on the needs list. An urgent loop never ends as the duplicate of one that is not. A guarded loop ends only as the duplicate of another guarded loop whose text is near-identical (0.8 or more alike once case, lookalikes and punctuation are folded). A recurring duplicate ends with `--final`.
- **Merges.** Two or more plain loops, none recurring. The new text keeps a word of every loop it replaces and contains no `·`. Its nudge is the earliest the merged loops had, never later, and today when that has passed. Its source lists theirs. The new text keeps every number the originals had outside their dates, and a merge cannot make an urgent or guarded loop into one that is not. The originals close first and the merged loop is added last: with none closed nothing is added, and if the add fails the closed originals are added back.
- **Re-dates.** Only a to-do with zero stakes that names no day of its own, three or more days overdue, moved to a day within 14 days. Each loop gets one move: a loop the report shows re-dated before stays due, in Do. A guarded loop is never re-dated, so a move cannot hide it past its real date.
- **One action a loop.** A plan that changes more than 25 loops is refused whole.
- **The right loop.** Before applying, af numbers a throwaway copy of `open.md` with every loop made due, and the run stops if its numbering differs from the sweep's. Each change re-reads `open.md` and targets the loop by its whole text when that is unique, else by number. A line that changed since the plan is skipped.

Every close goes through `af open close` with an outcome that starts `Sweep <YYYY-MM-DD>:` and says what settled it, so the month log holds each one. Every change is reversible: reopen a loop with `af open add`, or move it back with `af open redate`.

**Failures partway.** Each af call stops after 120 seconds. An af refusal or a hang is a `- Failed:` line, and the run goes on to the next change. An error af or the file system raises (af missing, `open.md` unreadable) stops applying where it is. Either way the report and the log name only the changes af made, list the rest as failures, and the run exits 1. The report and the log are each tried on their own; when the report cannot be written, the run section goes to the scheduler's output.

**While it runs.** A live run writes `workspace/life/systems/loop-sweep.running` before it reads anything, and removes it when it ends. The 08:30 push, which can start alongside it at logon, waits up to 7 minutes for that marker to go, inside the push task's 10-minute limit, then sends either way. When today's handoff is missing it first waits up to 45 seconds for one, in case the sweep has not started yet. A marker older than 30 minutes is a crashed run and is ignored.

## The model

- One `claude -p` call a run, on the Sonnet model at medium effort, capped at $0.60. It runs outside the vault in don't-ask mode with no built-in tools, no MCP servers or connectors (`--strict-mcp-config`), none of his settings, and a replaced system prompt. A run that saw or reached for any tool is discarded.
- Its input is the open loops, today's date, and the last 200 lines of each evidence file, all handed in by Python on stdin. No email reaches it.
- Its reply is one JSON object: a kind, event day and stakes for each loop, the proposed actions, and one board line per loop.
- With no usable plan, the run still closes on loop watch evidence, still hands over the overdue list, and leaves the board as it was.

## The overdue handoff

One push a day: the sweep sends nothing itself. It writes today's urgent overdue loops to `workspace/life/systems/loop-sweep-overdue.json`, even when there are none:

```
{"date": "2026-03-10", "items": [{"text": "<the loop's text>", "show": "<its board line, or null>", "due": "2026-03-07", "days": 3, "why": ["money", "deadline"]}]}
```

- `system/tools/push_due_loops.py`, at 08:30, reads the file when its date is today. Those loops lead the message under `Overdue, not confirmed`, in their board words with due date, days over, and why. They are left out of the sections below, so each goes out once.
- `show` is null for a loop the model kept off the board, and the push only counts it. With no plan, `show` is the loop's own text, which the push still filters.
- A stale or missing file changes nothing: the push sends every due loop as before. A handed loop that is no longer due is dropped.
- The loops stay due, so they come back the next morning, and in Do, until evidence or main closes them.

## Off Discord

Discord is a third-party service. `system/tools/off_discord.py` is the one filter for the board and the 08:30 push. A loop matching it shows only in a closing count, `N more kept off Discord; see Do in AgentFrame`. It matches two things:

- **Built-in categories.** Health, medication and meds, prescriptions, pharmacies, doctors (`Dr.`, `doc`), dentists, clinics, urgent care, scans and tests (`MRI`, `x-ray`, `-oscopy`), `psych`, `OB/GYN`, allergies, appointments (`appt`), therapy, passwords and account numbers. These always apply. Text is normalised first, so fullwidth letters, invisible characters and Cyrillic or Greek lookalikes do not slip past.
- **His terms.** `workspace/life/systems/discord/boards/open-private.txt`: private, one term a line, `#` for a comment. It holds his names and topics; nothing about him goes in the tracked file.

**With no terms file** nothing that names a loop goes to Discord: the board is not posted, and the push sends only `N due, M overdue and not confirmed; open AgentFrame to see them`.

**Numbers.** Any token holding four or more digits that is not a date or a time, and any grouped digit run (`123-456-789`, `604-555-1234`, `4520 1234 5678`), looks like an account, card, phone or confirmation number, and is masked as `####` on the board and in the push. An amount of money in any notation (`$40`, `C$40`, `40 CAD`, `USD 1,200`, `40 EUR`, `¥5000`, fullwidth signs, `40 bucks`) is masked as `$…` in the push and, for now, kept on the board. The push shows as a notification preview on his lock screen; the board is a pinned message in his own server that he opens, and what his bills come to is its point.

## The board

The #open board is generated, not hand-written. It has these sections:

- **Overdue, not confirmed**, from the push's list.
- **Owed to you**, **Bills**, **Coming up**, and **When you have time**, from the model's lines.
- **Decisions**, a count of the needs items due now.

A line shows only for a loop still open after the run. Needs items never show.

- A loop whose text or line matches the off-Discord filter stays off the board. The board is written and posted only once the private terms file exists, empty or not. The model is also told to leave off health, medication, therapy, a person's private details, account numbers and passwords.
- `discord_board.py` edits the pinned board in place and stamps the date.

## His evidence files

`workspace/life/systems/loop-sweep-files.txt`: private, optional, one vault path a line, `#` for a comment. Only files under `workspace/` count, such as a fitness log or a groceries inventory. With no file, vault evidence is limited to files a loop asked for.

## The report

```
# Loop sweep

<header line naming the tool and this schema>

## Run 2026-03-10 08:20

- Closed: Claim the transfer from the club → Sweep 2026-03-10: the loop watch found an email of 2026-03-09 from payments.interac.ca (Interac) showing it done
- Closed: Ask the neighbour about the parcel → Sweep 2026-03-10: day passed (03-08), unconfirmed (a one-day chat with nothing riding on it)
- Closed: verdict: the cafe → Sweep 2026-03-10: merged with 2 like it into one loop: Verdicts: the cafe, the gallery, the bar
- Merged into a new loop (three small asks of one kind): Verdicts: the cafe, the gallery, the bar
- Re-dated 2026-03-02 → 2026-03-14: Buy a winter hat
- Overdue, reminded: Pay the gym fee (due 2026-03-08; money)
- Flag #12: Renew or move?: the trigger looks met
- Refused close_day_passed #9: #9 has money riding on it, so a passed date settles nothing
- Phone: handed 1 overdue to the 08:30 push
- Board: posted to #open

Loops 41 · closed 4 · new merged loops 1 · re-dated 1 · overdue 1 · flags 1 · refused 1 · failures 0 · cost $0.12
```

- Each run appends one section; the last 60 are kept. A run with nothing to do says `- Nothing to change.`
- The counts line always prints. `cost` is what the model call reported, or `unknown`.
- A failure, such as no plan, an af refusal or a failed post, is a `- Failed:` line and makes the run exit 1.

## Reviewing it

Main reads the newest run section when the list looks wrong, and reopens any close that was wrong with `af open add`, citing the run. A refusal that repeats points at a loop to rewrite or a rule to change.

`-DryRun` (`--dry-run`) plans and prints everything a run would do, the board included, and changes, sends and writes nothing. Like the real run it calls the model once, and it lists each due loop with its kind and stakes.

| Verb | Effect |
|---|---|
| `python system/tools/loop_sweep.py --dry-run [--vault <root>] [--today <date>]` | The plan, what the 08:30 push would lead with, and the board, printed; nothing changed. `--today` previews a later morning |
| `powershell -File system\tools\register-loop-sweep.ps1` | Registers the daily task; `system/tools/laptop/timers.ps1` moves it with the others |
