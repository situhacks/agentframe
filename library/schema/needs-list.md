# Needs list

Shapes what main needs the operator to decide: marked lines in `workspace/life/open.md`, one detail file each in `workspace/life/needs/`, and the generated view `workspace/life/needs.md`. Owned by `af needs`. The marked line is the only state, so the list cannot drift from the [open loops](open-loops.md).

## The rule

- Review a loop before raising it here: [check first](open-loops.md#the-loop-rule), and close what you can yourself. Only a decision that is genuinely his goes on the list. Everything else stays an ordinary loop.
- Write the detail file first, then `af needs add`. The question is the line's text; the file holds the rest.
- Every reply to him, except one that is only a background-task status note, ends with the first line `af needs list` prints, linking `workspace/life/needs.md`. It counts Now and Do, and says none only when both are empty.
- A due plain loop that is his to act on, not decide, stays a plain loop; it shows under Do by itself. A due loop under `Waiting on others` is main's to chase, so it stays out of Do and prints only in `af open due`.
- Never edit `needs.md`; any `af` read or write of the loops overwrites it.

## The line

```
## Decisions needed

- [ ] 2026-11-05 · Which plan to take? · need now · detail needs/plan-choice.md · from: src-2026-11-05-01
- [ ] 2026-11-05 · Renew or move? · need on 2026-12-01 · detail needs/renew-or-move.md · from: main
- [ ] 2026-11-05 · Accept the quote? · need when the quote arrives · detail needs/quote.md · from: main
```

- `need` is `now`, `on <YYYY-MM-DD>`, or `when <trigger>`, the threshold or event in a few words. `detail` is `needs/<kebab-slug>.md`, relative to `workspace/life/`; one item to a file.
- A dated item needs him from its date on: the line stays as written and the view moves it to Now. A trigger never fires by itself: when the event happens, `af needs move <target> --now`.
- A marked line carries no nudge. A dated item appears in `af open due`, and so in the phone push, on and after its date; a `now` or `when` item never does.

## The detail file

`workspace/life/needs/<slug>.md`, the file he opens from the list:

```
---
private: true
recommendation: the cheaper plan
---
# Which plan to take?

## The decision
## Recommendation
## Options
## Background
## Sources
```

- `recommendation` is a few words; the view prints it in its own column. `af needs add` refuses a file without one.
- The body says what the decision is, main's recommendation and why, the options with what each costs him, the background he needs, and links to the sources.
- The date or trigger lives on the line, never here. Closing moves the file to `needs/closed/`, and the log line names its new path.

## The view

`workspace/life/needs.md` has `## Now`, `## Do`, and `## Later` sections, each a table with one row per item, or `Nothing.` when empty. Now and Later are the marked lines; Notes links the detail file, and When holds the date or the trigger. Do is generated from `af open due`: the plain due loops outside `Waiting on others`. Main reviews those and closes the rest itself, so what stays is his to do; Waiting since is the loop's opened date.

```
## Now (1)

| Decision | Recommendation | Notes |
|---|---|---|
| Which plan to take? | the cheaper plan | [open](needs/plan-choice.md) |

## Do (1)

| To do | Due | Waiting since |
|---|---|---|
| Pay the bill | 2026-11-03 | 2026-10-24 |

## Later (2)

| When | Decision | Recommendation | Notes |
|---|---|---|---|
| 2026-12-01 | Renew or move? | renew | [open](needs/renew-or-move.md) |
| the quote arrives | Accept the quote? | wait for it | [open](needs/quote.md) |
```

A cell is one line, and a `|` or `\` in its text is escaped so it cannot split the row. A missing detail file reads `detail file missing: needs/<slug>.md` in Notes. `af needs list`, session start, and `af brief` print plain lines, not tables.

Every `af` read or write of the loops rewrites it, `af open due` and `af brief` among them, so a date that passes reaches it with no change to `open.md`; the daily phone push runs one even when no chat does. Session start prints the count and the Now items to a main chat, and `af brief` carries the same section.

## Doctor

`af doctor` reports an item whose detail file is missing or has no recommendation, a detail file in `needs/` with no open item, two items sharing a file, and a view it could not rewrite. A marked line that breaks the grammar is reported with the other `open.md` lines.

| Verb | Effect |
|---|---|
| `af needs add "<question>" --detail needs/<slug>.md [--on <date or +Nd> \| --when "<trigger>"] [--from <id>]` | Appends a marked line under `## Decisions needed`; refuses a missing file, one with no recommendation, or one already in use |
| `af needs close <n, slug, or text prefix> --outcome "<text>"` | Moves the detail file to `needs/closed/` and the loop to the month log; refuses anything not on the list |
| `af needs move <n, slug, or text prefix> --now \| --on <date or +Nd> \| --when "<trigger>"` | Changes the state in place; the log records it. `af open redate` refuses a marked loop and points here |
| `af needs list` | The counts, then each item with the number `af open close` takes |
| `af needs render` | Rewrites the view from `open.md` and the detail files |
