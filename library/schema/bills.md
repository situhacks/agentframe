# Bills

Shapes `workspace/life/systems/finances/bills.md`, the card statement ledger. `system/tools/bills_watch.py` writes all of it except the Cards list; the [10:00 batch run](watch.md) runs it first each morning (`bills-watch.ps1` by hand), ahead of the loop watch and the loop push. The file is a ledger of what the emails said, not a reminder file: each unpaid due date in it is a loop under `Bills` in `open.md`.

```
# Bills

<header line naming the watch and this schema>

Gmail read 2026-03-10 07:45.

| Card | Statement | Balance | Minimum | Due | Paid | Emails |
|---|---|---|---|---|---|---|
| Northwind Rewards | 2026-03-04 | $1,204.50 | $25.00 | 2026-03-25 | unpaid | m-01, m-02 |

## Cards

- Northwind Rewards: northwind-bank.example

## Loops raised

- 2026-03-10 · Pay Northwind Rewards by 2026-03-25: statement of 2026-03-04

## Skipped

- 2026-03-10 · thread t-9 · mcp__example__create_order

## Emails

- 2026-03-09 · Northwind Rewards · balance · closing 2026-03-04 · due 2026-03-25 · statement $1,204.50 · minimum $25.00 · current $1,474.50 · credits $40.00 since 2026-03-05 · id m-02
```

- **Cards**, the one hand-edited part: `- <card name>: <sender domain>[, <domain>]`. A sender matches its domain or a subdomain. A card whose bank sends no email still gets a row, reading `no statement email yet`.
- **Gmail read** is the last clean read; the next search starts 3 days before it (60 days on the first run). A failed read, an unreadable thread, a run that read no figures, or threads left for the next run leave it alone.
- **The table** shows each card's latest statement, and any older one still unpaid. Statement is the closing date, or the date of the first email about it when no closing date is given. Paid is `paid <date> (<evidence>)`, `marked paid in chat` (its loop was closed by hand), `unpaid`, `unpaid, missed (email <date>)`, `nothing due`, or `figures pending` (a statement-ready email with no amounts yet).
- **Emails** holds one line per card email read, with the figures its labels stated; its ids are how the watch skips mail it has seen. Kinds: `statement`, `balance`, `reminder`, `payment`, `missed`, `other`.
- **Loops raised** lists each statement loop the watch has raised, keyed on card and statement date, so a loop closed in chat is not raised again.
- **Skipped** lists threads whose fetch reached for a refused tool; they are never fetched again.

## Statements and paid evidence

- Emails fold into one statement by closing date, else due date. An email with no figures joins the statement whose due date falls within 35 days of it; a missed-payment email with no dates joins the card's latest statement.
- A balance update for the same closing date whose credits since closing cover the whole statement balance pays it. Credits covering only the minimum are not evidence: refunds count as credits.
- A payment email dated after a statement opened, for at least its minimum, pays the unpaid statement whose balance it matches, else the one whose due date is nearest.
- His word in chat: close the loop (`af open close "Pay <card> by <due>" --outcome "paid ..."`); the table then reads `marked paid in chat`.

## Loop timing

| When | Loop |
|---|---|
| The statement's balance and due date are first known | `Pay <card> by <due>: statement of <date>, $X, minimum $Y` raised, nudged that day |
| The run after it pushed | re-dated to 5 days before due, then to 1 day before |
| Past 1 day before, still unpaid | stays due, so the morning push repeats it until paid |
| The due date changes | the loop is closed (`due date moved to ...`) and raised again with the new date at the same stage |
| A missed-payment or late-fee email under 7 days old, statement unpaid | `<card> missed a payment: statement of <date>, ...` raised, nudged that day, with or without figures |
| Paid evidence arrives | both loops closed with the evidence as the outcome |
| Gmail or Claude cannot be reached | one `Bills watch could not read Gmail: ...` loop, closed by the next good read |
| Anything else stops a clean read: an unreadable thread, emails with no readable figures, a changed search format, a missing Cards list, a crash | one `Bills watch failed: ...` loop, closed by the next clean run |
| A fetch reaches for a refused tool | `Bills watch refused a tool call and skipped card email thread <id>: ...`, left for him to check |

A statement due more than 14 days before the run raises no loop; it shows in the table only.

## Route and limits

- Gmail is reachable only through the claude.ai Gmail connector, so the watch starts `claude -p` outside the vault in don't-ask mode: no built-in tools, none of his settings or allow rules loaded, only `search_threads` and `get_thread` allowed, every other Gmail tool and every other connector denied (`--strict-mcp-config` would drop the claude.ai connectors too). One run searches, then one run fetches each thread, so an email that steers the model into a refused call costs only its own thread. A run whose connector list fails to load is retried once.
- The model fetches; the script parses the raw results from each run's stream, so no figure passes through the model.
- The search filters on bill subjects and reads one 50-result page, because the connector returns no next-page token; a full page is noted in `system/logs/bills-watch.log`. At most 20 threads are fetched a run; the rest wait for the next.
- Never stored: card or account numbers, passwords, email bodies. Each run's saved tool results, which hold whole email bodies, are deleted from Claude Code's project folder as soon as they are parsed. Never done: payments, logins, Gmail labels, sends or deletes.
- Some banks' statement-ready emails carry no amounts; the figures arrive with the next balance update or payment reminder, and the table reads `figures pending` until then.
