# Needs list

Shapes what main needs the operator to decide: marked lines in `workspace/life/open.md`, one detail file each in `workspace/life/needs/`, and the generated view `workspace/life/needs.md`. Owned by `af needs`. The marked line is the only state, so the list cannot drift from the [open loops](open-loops.md).

## The rule

- Review a loop before raising it here; close what you can yourself. Only a decision that is genuinely his goes on the list. Everything else stays an ordinary loop.
- Write the detail file first, then `af needs add`. The question is the line's text; the file holds the rest.
- A reply that touches or owes a decision ends with one line: how many need him now, linking `workspace/life/needs.md`. `af needs list` prints the count.
- Never edit `needs.md`; the next render overwrites it. `af doctor` notes one that differs from its sources.

## The line

```
## Decisions needed

- [ ] 2026-11-05 · Which plan to take? · need now · detail needs/plan-choice.md · from: src-2026-11-05-01
- [ ] 2026-11-05 · Renew or move? · need on 2026-12-01 · detail needs/renew-or-move.md · from: main
- [ ] 2026-11-05 · Accept the quote? · need when the quote arrives · detail needs/quote.md · from: main
```

- `need` is `now`, `on <YYYY-MM-DD>`, or `when <trigger>`, the threshold or event in a few words. `detail` is `needs/<kebab-slug>.md`, relative to `workspace/life/`; one item to a file.
- A dated item needs him from its date on: the line stays as written and the view moves it to Now. A trigger never fires by itself: when the event happens, `af needs move <target> --now`.
- A marked line carries no nudge, so `af open due` and the phone push skip it; the list is its one reminder surface.

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

- `recommendation` is a few words; the view prints it beside the question. `af needs add` refuses a file without one.
- The body says what the decision is, main's recommendation and why, the options with what each costs him, the background he needs, and links to the sources.
- The date or trigger lives on the line, never here. Closing moves the file to `needs/closed/`, and the log line names its new path.

## The view

`workspace/life/needs.md` has `## Now` and `## Later` sections, one line per item, each question a relative link to its detail file:

```
- [Which plan to take?](needs/plan-choice.md) · recommend: the cheaper plan
- on 2026-12-01: [Renew or move?](needs/renew-or-move.md) · recommend: renew
```

Every `af needs` change and any `af open close` of a marked loop rewrites it, and so does each session start, which also prints the count and the Now items to a main chat. `af brief` carries the same section.

## Doctor

`af doctor` reports an item whose detail file is missing or has no recommendation, a detail file in `needs/` with no open item, and two items sharing a file. A marked line that breaks the grammar is reported with the other `open.md` lines.

| Verb | Effect |
|---|---|
| `af needs add "<question>" --detail needs/<slug>.md [--on <date or +Nd> \| --when "<trigger>"] [--from <id>]` | Appends a marked line under `## Decisions needed`; refuses a missing file, one with no recommendation, or one already in use |
| `af needs close <n, slug, or text prefix> --outcome "<text>"` | Moves the detail file to `needs/closed/` and the loop to the month log; refuses anything not on the list |
| `af needs move <n, slug, or text prefix> --now \| --on <date or +Nd> \| --when "<trigger>"` | Changes the state in place; the log records it. `af open redate` refuses a marked loop and points here |
| `af needs list` | The counts, then each item with the number `af open close` takes |
| `af needs render` | Rewrites the view from `open.md` and the detail files |
