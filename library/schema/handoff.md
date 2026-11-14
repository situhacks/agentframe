# Brief and handoff

Shapes the files in `system/sessions/<name>/` (runtime state, gitignored). The chat writes them by hand; `af hook session-start` prints the brief and the page, and `af hook stop` checks the page.

**`brief.md`**, written by the chat or the operator that starts it, before it starts: frontmatter `goal` (one line), `done_when` (the check that ends the job), `write_surface` (a list of vault-relative paths), `model` (`opus` or `sonnet`), and optionally `class` (`main`, `side`, or `build`; [`sessions-registry.md`](sessions-registry.md#session-classes) says what each means); the body is free.

**`handoff.md`** is the chat's page: what is current, and only that. The name stays `handoff.md`. Finished items go to `archive.md` beside it, which is never printed at session start.

Copy this shape. Each heading line holds only the heading:

```
---
written: 2026-10-25T17:38
session_id: <uuid>
---
## State
## Doing
## To do
## Gated
## Done
## Rules
## Open questions
## Next step
## Files touched
```

What goes under each heading:

- `State`: where the work is, 150 words or fewer. Free text.
- `Doing`: what is in progress now.
- `To do`: what is next and not blocked.
- `Gated`: what waits on something; each item ends `· waits on: <condition>`.
- `Done`: finished items, newest first, at most 10 and none older than 14 days.
- `Rules`: standing rules this chat follows, each with the date it was set or last confirmed.
- `Open questions`: questions for him. Free text.
- `Next step`: free text.
- `Files touched`: one vault-relative path per line.

The sections come in this order, and no other `## ` heading is allowed. `State`, `Next step` and `Files touched` are required; the others appear when they have content. A page with only the four original sections (State, Open questions, Next step, Files touched) is valid.

An item in `Doing`, `To do`, `Gated`, `Done` or `Rules` is one line, `- YYYY-MM-DD · <text>`. The date is when it was added; for `Done` when it was finished, for `Rules` when it was set or last confirmed. An indented line under an item belongs to it. A fenced code block (``` or ~~~) is never a heading, an item, or the end of a section: a `## ` or `- ` line inside one is plain text, and a block under an item belongs to it.

`written:` is required (`YYYY-MM-DDTHH:MM`, local time); a page without one is refused.

## The archive

`archive.md` takes plain lines, each `- YYYY-MM-DD · <text>` as it stood in `Done`, under a `## YYYY-MM` heading for the month of its date (added at the end of the file when missing). A line goes at the end of its month's section. No check reads it; a chat opens it only for old history. A retired rule is deleted, not archived: the order it came from is in `workspace/life/sources/orders/`.

## The checks

Every issue is one of two kinds. A `[fix]` issue is mechanical, and the Stop hook blocks a stop for it. A `[review]` issue needs the chat's judgment and never blocks a stop.

`[fix]` issues:

1. The body (frontmatter not counted) is over 1,200 words.
2. A `## ` heading outside the list, a required section missing, or sections out of order.
3. An item line that does not start with `- YYYY-MM-DD · `.
4. A `Gated` item with no `waits on:`.
5. `Done` holds more than 10 items, or an item older than 14 days.

`[review]` issues:

6. `State` is over 150 words: cut it to where the work is and move the rest to Doing, To do or Done.
7. A `To do` item dated more than 21 days ago: do it, re-date it and say why it still stands, gate it, or delete it.
8. A `Gated` item dated more than 45 days ago: check the condition; re-date or delete.
9. A `Rules` item dated more than 30 days ago: confirm it still applies and re-date it, or delete it.

The limits are constants in `system/af/src/page.rs`. Every issue names the section, quotes the first 40 characters of the item, and says the fix.

## Where the checks show

- **`af session tidy [<name>] [--check] [--all]`**: no name is the calling chat; a name is that chat, whatever its status; `--all` is every chat whose row has not ended. It keeps the 10 newest `Done` items by date (a tie keeps the page's order), moves the rest and any older than 14 days into `archive.md`, and saves the page, changing nothing else (`written:` included). An item with no usable date stays. Each file is replaced through a temp file, the archive first, and an item already standing verbatim in its month is not filed twice, so a re-run after a crash is harmless. `--check` changes nothing. It prints the page path, its word count, and one `[fix]` or `[review]` line per issue (or `clean`), and exits 1 when any issue remains.
- **Stop hook** (`af hook stop`): blocks once, as for a stale page, when the page has a `[fix]` issue: run `af session tidy`, fix the rest, then set `written:`. `[review]` issues never block. A `main` row is never blocked by the hook; the `af session stop` verb reports for any row.
- **Session start**: after the page, a block lists every issue, for every role. It starts `af: your page needs tidying before other work:` when the page has a `[fix]` issue, and `af: your page has items to review when this task allows:` when it has only `[review]` issues. A page over 1,200 words is printed cut at a line end, with a line giving the words shown and the page's total.
- **`af doctor`**: one note per issue for every chat whose row has not ended.
- **`af brief`** counts a page as current unless it is missing, has no `written:`, or is stale; a page that only needs tidying is current.

## Staleness

A page is stale when the newest mtime among `Files touched` (or, with none listed, under the brief's `write_surface`), floored to the minute, is later than `written`. The Stop hook then blocks once for `project/*`, `system/*`, and `worker/*` chats, never for `main`, and staleness is asked about before the `[fix]` issues. `af doctor` notes a page older than its folder's newest file; `archive.md` is not counted.
