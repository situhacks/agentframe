# Template: Decision Log

## Purpose

The append-only, dated record of every material decision: what was decided, why, and who owns it. A **living doc** in the project's `knowledge/`. It is the engagement's memory of *why* things are the way they are — the antidote to re-litigating settled calls.

## Inputs

- Decisions as they happen, from meetings, status calls, and operator direction.
- The `charter` and `raid-log` for context on what's being decided against.

## Sections

One **append-only** table, newest at the bottom. Per entry:

- `date` · **decision** (one line) · **rationale** (the why, short) · **owner** (who made/owns it) · **status** (`active` / `superseded by #N`).

A superseded decision is **not** deleted — mark it `superseded by #N` and add the new entry. The chain stays readable.

## Hard Constraints

- Append-only. Never edit or delete a prior entry's decision/rationale; supersede instead.
- Every entry has a date, an owner, and a rationale. A decision with no recorded why is incomplete.
- One decision per entry — don't bundle.

## Draft Frontmatter Convention

```yaml
---
status: drafting   # living doc — stays drafting through the engagement
last_updated: <ISO-8601 timestamp>
---
```

## Lock Criteria

Living — does not lock. Current when every material decision taken to date has an entry with date, rationale, and owner. Read at closeout for the decision trail.
