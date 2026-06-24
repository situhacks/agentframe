# Template: Workback Schedule

## Purpose

Milestones and the work breakdown planned **backward from the deadlines** — so the engagement knows what must be true by when, and the weekly/standup status is a *view* of this plus the RAID, not a separately maintained artifact. A **living doc** in the project's `knowledge/`; ever-evolving, so it is not a locked deliverable.

## Inputs

- The locked `charter` (fixed dates, constraints, objectives).
- The `raid-log` dependencies (what gates what).

## Sections

- **Milestones** — planned backward from the final deadline. Per milestone: `milestone` · target date · owner · depends-on (milestone ids or RAID dependency ids) · status (`not-started` / `in-progress` / `done` / `at-risk`).
- **Work breakdown** — the tasks under each milestone, enough to see the critical path. Not a full Gantt unless the engagement earns one.
- **Critical path note** — the chain that can't slip without moving the deadline.

## Hard Constraints

- Planned **backward** from deadlines, not forward from today — the deadline is the anchor.
- Every milestone has an owner and a target date.
- Slips are reflected here and surfaced (an `at-risk` milestone is a RAID issue candidate), not hidden.
- The weekly status is a derived **view** of this + the RAID; do not maintain a parallel status doc.

## Draft Frontmatter Convention

```yaml
---
status: drafting   # living doc — stays drafting through the engagement
last_updated: <ISO-8601 timestamp>
---
```

## Lock Criteria

Living — does not lock. Current when every milestone has an owner, a date, and a status, and the critical path is named. The status cadence reads from it.
