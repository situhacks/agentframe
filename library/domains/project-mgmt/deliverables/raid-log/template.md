# Template: RAID Log

## Purpose

Risks, Assumptions, Issues, and Dependencies in one living document—the engagement's standing record of what could go wrong, what's being taken for granted, what's actively blocking, and what it waits on. A **living doc**: continuously updated, never terminal. Lives in the project's `knowledge/`.

## Inputs

- The ready `charter` (scope, constraints, stakeholders).
- Meeting notes and status updates as the engagement runs.

## Sections

Four tables. Every entry carries **one named owner** and a date.

- **Risks** — `id` · description · likelihood · impact · owner · mitigation · status (`open`/`mitigating`/`closed`).
- **Assumptions** — `id` · what's assumed · owner · validation status · date validated.
- **Issues** — `id` · what's actively wrong · owner · action · status (`open`/`in-progress`/`resolved`) · date.
- **Dependencies** — `id` · what we wait on · who owns it (internal/external) · needed-by date · status.

## Hard Constraints

- **One named owner per entry.** No entry owned by "the team".
- Reviewed on a weekly cadence; stale entries (no update in the cadence window) are surfaced, not silently carried.
- An entry is closed/resolved with a dated note, not deleted — the trail stays.
- A standalone "risk register" is not created; the R of RAID is here.

## Draft Frontmatter Convention

```yaml
---
status: drafting   # living doc—stays drafting through the engagement
last_updated: <ISO-8601 timestamp>
---
```

## Readiness Criteria

Living—does not use deliverable readiness state. It is current when every open entry has an owner and a status, and the weekly review ran. The closeout retro reads its final state.
