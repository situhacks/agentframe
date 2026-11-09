# Loop sweep

Shapes `workspace/life/systems/loop-sweep-report.md`, the record of the morning sweep of his open loops, and `workspace/life/systems/loop-sweep-overdue.json`, the overdue list it hands the 08:30 push. `system/tools/loop_sweep.py` writes both; `system/tools/loop-sweep.ps1` runs it once a day at 08:20 (`register-loop-sweep.ps1`), after the 07:50 [loop watch](loop-watch.md) and ahead of the 08:30 loop push.

The sweep makes no judgement calls. A to-do is never passed because its date went by: it is closed on proof, or he is reminded and main checks it with its own tools.

## What it does

1. **Closes on proof, and only on proof.** See the rule below. Nothing else closes, re-dates, merges or ends a loop.
2. **Reminds.** Every overdue loop it did not close, other than needs items, `Bills` loops and loops under `Waiting on others`, stays open and due and leads the 08:30 push.
3. **Reports.** Appends a run section for main: the closes, each overdue loop with no proof, and what the model noticed.
4. **Publishes**, in a live run only. Rewrites the Discord #open board source, `workspace/life/systems/discord/boards/open.md`, and posts it with `system/tools/discord_board.py board open`.

## Modes

| Mode | Command | Writes |
|---|---|---|
| Report-only, the default | `loop-sweep.ps1`, `loop_sweep.py` | The report, the overdue handoff, the log line. Changes no loop; each close it would make is a `Would close (report-only, not applied)` line in the report, since Task Scheduler discards the output |
| Live | `loop-sweep.ps1 -Live`, `loop_sweep.py --live` | Also each close on proof through `af open close`, and the board |
| Dry run | `loop-sweep.ps1 -DryRun`, `loop_sweep.py --dry-run [--today <date>]` | Nothing: prints the run section and what the push would lead with. `--today` previews a later morning |

`register-loop-sweep.ps1` registers report-only; `register-loop-sweep.ps1 -Live` registers the live run. `system/tools/laptop/timers.ps1` with no switch shows which mode the registered job is in, and its `-Register` registers report-only.

## The proof rule

A loop closes only on a `done` verdict in today's loop watch report (a verdict the watch capped to `update` proves nothing) whose email comes from an allowlisted sender and ties to the loop:

| Sender | Ties when |
|---|---|
| Interac (`interac.ca`) | The counterparty's name from Interac's subject is a capitalised word of the loop, or an amount the email states equals an amount in the loop |
| A card issuer in the bills ledger's Cards list | The loop names that card or its issuer |
| A ticketing platform (Eventbrite, Ticketmaster, Luma, OpenTable and the like) | The loop names the platform, or shares two distinctive words with the email's subject; ticket words such as `tickets`, `order` and `booking` do not count |

- **Amounts** compare as numbers on whole tokens of the loop, once its days, times and source ids are cut: `40` does not match `400`, `40.00` matches `40`, and `#40` or `40k` is no amount.
- **One loop only.** The email is checked against every open loop, owned ones included. A proof that ties to more than one closes none of them and is reported as ambiguous.
- **Owned loops never close here.** The bills watch closes `Bills` loops; needs items and `Waiting on others` are main's.
- **Not proof:** a passed date, a line in his files, a file that exists, a free mail address, or a sender at a domain the loop mentions. A `done` from any sender outside the table is a `Not proof` line.
- The report is read by Python; the model never sees email. A close's outcome starts `Sweep <YYYY-MM-DD>:` and names the date, the sender's domain and the tie, never email text.

**Applying.** A live run closes each loop by its whole text, which af matches exactly. A loop whose text another open loop shares, or that af would read as a number or a flag, is not closed. Each af call stops after 120 seconds. A close af refuses, or that raises, is a `- Failed:` line; the loop stays open, stays on the overdue list, and the run exits 1. A recurring loop closed on proof re-dates one period on, as `af open close` does.

**While it runs.** A run that writes creates `workspace/life/systems/loop-sweep.running` before it reads anything and removes it when it ends. The 08:30 push waits up to 7 minutes for that marker to go, inside its 10-minute limit, then sends either way. When today's handoff is missing it first waits up to 45 seconds for one. A marker older than 30 minutes is a crashed run and is ignored.

## The model

- One `claude -p` call a run, on the Sonnet model at medium effort, capped at $0.60. It runs outside the vault in don't-ask mode with no built-in tools, no MCP servers or connectors (`--strict-mcp-config`), none of his settings, and a replaced system prompt. A run that saw or reached for any tool is discarded.
- Its input is the open loops and today's date, handed in on stdin. Its reply is one JSON object: entries naming loops that look like duplicates, could be merged, or look stale, and one board line per loop.
- None of its words reach a loop or the report: Python writes each entry as loop numbers and its own copy of their text, at most 15 lines a group. An entry naming no real loop is dropped.
- With no usable plan, the run still closes on proof, still hands over the overdue list with each loop's own text, and leaves the board as it was.

## The overdue handoff

One push a day: the sweep sends nothing itself. It writes today's overdue loops to `workspace/life/systems/loop-sweep-overdue.json`, even when there are none:

```
{"date": "2026-03-10", "items": [{"text": "<the loop's text>", "show": "<its board line, or null>", "due": "2026-03-07", "days": 3}]}
```

- `system/tools/push_due_loops.py`, at 08:30, reads the file when its date is today. Those loops lead the message under `Overdue, not confirmed`, in their board words with due date and days over, and are left out of the sections below, so each goes out once.
- `show` is null for a loop the model kept off the board, and the push only counts it. With no plan, `show` is the loop's own text, which the push still filters.
- A stale or missing file changes nothing: the push sends every due loop as before. A handed loop that is no longer due is dropped.

## Off Discord

Discord is a third-party service. `system/tools/off_discord.py` is the one filter for the board and the 08:30 push. A loop matching it shows only in a closing count, `N more kept off Discord; see Do in AgentFrame`. It matches two things:

- **Built-in categories.** Health, medication and meds, pills, prescriptions, pharmacies, doctors (`Dr.`, `doc`), dentists, clinics, urgent care, surgery and surgeons, bloodwork, an annual physical or physical exam, scans and tests (`MRI`, `x-ray`, `-oscopy`), `psych`, `OB/GYN`, allergies, appointments (`appt`), therapy, passwords and account numbers, each as a whole word: `physically` and `pillow` pass. Text is normalised first: fullwidth letters, invisible characters and Cyrillic or Greek lookalikes read as plain Latin, and accents are folded, so `thérapy` matches.
- **His terms.** `workspace/life/systems/discord/boards/open-private.txt`: private, one term a line, `#` for a comment, folded the same way. It holds his names and topics; nothing about him goes in the tracked file.

**With no terms file** nothing that names a loop goes to Discord: the board is not posted, and the push sends only `N due, M overdue and not confirmed; open AgentFrame to see them`.

**Numbers.** Any token holding four or more digits that is not a date or a time, and any grouped digit run (`123-456-789`, `604-555-1234`), is masked as `####` on the board and in the push. An amount of money in any notation (`$40`, `C$40`, `40 CAD`, `USD 1,200`, `40 bucks`) is masked as `$…` in the push and kept on the board. Masked text keeps its accents.

## The board

Generated in a live run, never hand-written: **Overdue, not confirmed**; **Owed to you**, **Bills**, **Coming up** and **When you have time** from the model's lines; **Decisions**, a count of the needs items due now. A line shows only for a loop still open after the run; needs items and loops matching the off-Discord filter never show. `discord_board.py` edits the pinned board in place and stamps the date.

## The report

```
# Loop sweep

<header line naming the tool and this schema>

## Run 2026-03-10 08:20

- Would close (report-only, not applied): #3 Claim the transfer from the club, $12.00 → Sweep 2026-03-10: an email of 2026-03-09 from payments.interac.ca (Interac, for the amount the loop names) proves it done
- Ambiguous proof: the email of 2026-03-09 from payments.interac.ca fits #5 Collect $15 from Robin, #9 Robin owes $15; none closed
- Not proof: #7 Pick up the widget: the email from widget.example is not proof for it

### Passed with no proof

- #2 Pay the gym fee (due 2026-03-07)

### Looks like a duplicate

- #10 Book the dentist cleaning; #11 Book the dentist cleaning soon

### Could be merged

- #5 verdict: the cafe; #6 verdict: the gallery

### Looks stale

- #8 Decide on the winter trip

- Phone: handed 1 overdue to the 08:30 push
- Board: not changed: report-only

Loops 41 · report-only · closed 0 · would close 1 · ambiguous 1 · passed with no proof 1 · duplicates 1 · merges 1 · stale 1 · failures 0 · cost $0.12
```

- Each run appends one section; the last 60 are kept. The counts line always prints; `cost` is what the model call reported, or `unknown`.
- A failure, such as no plan, a failed close or a failed post, is a `- Failed:` line and makes the run exit 1. The report and the log (`system/logs/loop-sweep.log`) are each tried on their own; when the report cannot be written, the run section goes to the scheduler's output.

## Reviewing it

Main reads the newest run section and acts on it with its own tools: checks each passed loop against Gmail, the calendar and his pages, closes or re-dates what they settle, merges or ends duplicates, and asks him only what they cannot answer. It reopens a wrong close with `af open add`, citing the run.
