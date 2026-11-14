# Needs list

Shapes what main needs the operator to decide: marked lines in `workspace/life/open.md`, each carrying the decision's recommendation, options and reason, and the generated view `workspace/life/needs.md`. Owned by `af needs`. The marked line is the only state, so the list cannot drift from the [open loops](open-loops.md), and no file is made per decision.

## The rule

- Review a loop before raising it here: [check first](open-loops.md#the-loop-rule), and close what you can yourself. Only a decision that is genuinely his goes on the list. Everything else stays an ordinary loop.
- Give the recommendation, options and reason on `af needs add`; the question is the line's text. For anything longer, link a document that already exists (a plan, a project file, a knowledge page, a backlog row) with `--more`. Never create a file for one decision.
- Every reply to him, except one that is only a background-task status note, ends with the first line `af needs list` prints, linking `workspace/life/needs.md`. It counts Now and Do, and says none only when both are empty.
- A due plain loop that is his to act on, not decide, stays a plain loop; it shows under Do by itself. A due loop under `Waiting on others` is main's to chase, so it stays out of Do and prints only in `af open due`.
- Never edit `needs.md`; any `af` read or write of the loops overwrites it.

## The line

```
## Decisions needed

- [ ] 2026-11-05 · Which plan to take? · need now · rec: the cheaper plan · options: a) the cheaper plan, $12 a month; b) the bigger plan, $20 a month; c) stay · why: You use about 3 GB a month, and the cheaper plan covers 10. · more: knowledge/phone.md#plans · from: src-2026-11-05-01
- [ ] 2026-11-05 · Renew or move? · need on 2026-12-01 · rec: renew · from: main
- [ ] 2026-11-05 · Accept the quote? · need when the quote arrives · rec: wait for it · from: main
```

- `need` is `now`, `on <YYYY-MM-DD>`, or `when <trigger>`, the threshold or event in a few words.
- The fields come after `need` and before `from:`, in this order. A line with them out of order, or with no `rec:`, does not parse, and `af doctor` reports it with the other `open.md` lines.

| Field | Rule |
|---|---|
| `rec:` | Required. At most 12 words. |
| `options:` | Optional. Up to 5, written `a) ...; b) ...`, each at most 30 words. `af` adds the letters; the caller passes the texts. |
| `why:` | Optional. At most 60 words. |
| `more:` | Optional. A path relative to `workspace/life/` or to the vault root, with an optional `#anchor`. The file must exist and may not be under `workspace/life/needs/`; an absolute path, a drive letter or a `..` is refused, and `.` segments are dropped. |

- A value may not contain ` · `, and an option may not contain `;`. `af` refuses and says to reword.
- A dated item needs him from its date on: the line stays as written and the view moves it to Now. A trigger never fires by itself: when the event happens, `af needs move <target> --now`.
- A marked line carries no nudge. A dated item appears in `af open due`, and so in the phone push, on and after its date; a `now` or `when` item never does.
- Old-style lines, `need ... · detail needs/<slug>.md · from: ...` with the recommendation in that file's frontmatter, still parse and work, and the line's `rec:` wins when it has both. Closing such an item moves its file to `needs/closed/`; fold one in with `af needs set <n> --rec "<a few words>" --drop-detail`.

## The view

`workspace/life/needs.md` has `## Now`, `## Do`, and `## Later` sections, each a table with one row per item, or `Nothing.` when empty. Now and Later are the marked lines; More links the `more:` document (and an old detail file), and When holds the date or the trigger. Do is generated from `af open due`: the plain due loops outside `Waiting on others`. Main reviews those and closes the rest itself, so what stays is his to do; Waiting since is the loop's opened date.

Under the Now and Later tables, an item that has options or a reason gets a block, in the table's order; an item with neither gets none. The view escapes what it prints from the line so it cannot change the page (a backslash before `<` and `>`, and before a leading `#`, `-`, `+`, `*`, `=` or `1.`); the line in `open.md` is never escaped.

```
## Now (1)

| Decision | Recommendation | More |
|---|---|---|
| Which plan to take? | the cheaper plan | [more](knowledge/phone.md#plans) |

**Which plan to take?** Recommended: the cheaper plan.
- a) the cheaper plan, $12 a month
- b) the bigger plan, $20 a month
- c) stay

You use about 3 GB a month, and the cheaper plan covers 10.

## Do (1)

| To do | Due | Waiting since |
|---|---|---|
| Pay the bill | 2026-11-03 | 2026-10-24 |

## Later (2)

| When | Decision | Recommendation | More |
|---|---|---|---|
| 2026-12-01 | Renew or move? | renew |  |
| the quote arrives | Accept the quote? | wait for it |  |
```

A cell is one line, and a `|` or `\` in its text is escaped so it cannot split the row. More is empty when the line has no `more:` and no old detail file; a `more:` document that is gone reads `missing: <path>`, and an old detail file that is gone reads `detail file missing: needs/<slug>.md`. `af needs list`, session start, and `af brief` print plain lines (the question and its recommendation), not tables or blocks; the recommendation comes from the line, and from the old file's frontmatter only for an old-style line without a `rec:`.

Every `af` read or write of the loops rewrites it, `af open due` and `af brief` among them, so a date that passes reaches it with no change to `open.md`; the daily phone push runs one even when no chat does. Session start prints the count and the Now items to a main chat, and `af brief` carries the same section.

## Doctor

`af doctor` reports a field over its cap, a `more:` document that no longer exists, a file in `needs/` (not `closed/`) that no open item names (move it to `needs/closed/` or delete it), and a view it could not rewrite. For an old-style item it also reports a missing detail file, a file with no recommendation when the line has no `rec:`, and two items sharing a file. One note counts the open items that still keep an old detail file and gives the command that folds one in; the count is the measure for the migration.

| Verb | Effect |
|---|---|
| `af needs add "<question>" --rec "<few words>" [--option "<text>"]... [--why "<text>"] [--more <path[#anchor]>] [--on <date or +Nd> \| --when "<trigger>"] [--from <id>]` | Appends a marked line under `## Decisions needed` and makes no file; refuses a missing `--rec`, a value over its cap (saying the cap and the count), a forbidden ` · ` or `;`, or a `--more` that does not exist or sits under `needs/`. The old `--detail needs/<slug>.md` form is still accepted, with a note that it is the old form |
| `af needs set <n, slug, or text prefix> [--rec ...] [--option ...]... [--why ...] [--more ...] [--drop-detail]` | Changes the fields of an open item in place; fields not named stay as they are; `--option` given at all replaces the whole list, and an empty `--why`, `--more` or lone `--option ""` clears it. `--drop-detail` removes the old `detail` reference and moves that file to `needs/closed/`; it refuses unless the line has (or is given) a `rec:`, prints where the file went (a `-2` name after a clash), logs a `folded:` line in the month log, and when the file cannot move changes nothing. The slug form matches old-style lines only |
| `af needs close <n, slug, or text prefix> --outcome "<text>"` | Moves the loop to the month log, and an old detail file, if the item has one, to `needs/closed/`; refuses anything not on the list |
| `af needs move <n, slug, or text prefix> --now \| --on <date or +Nd> \| --when "<trigger>"` | Changes the state in place; the log records it. `af open redate` refuses a marked loop and points here |
| `af needs list` | The counts, then each item with the number `af open close` takes |
| `af needs render` | Rewrites the view from `open.md` and any old detail files |
