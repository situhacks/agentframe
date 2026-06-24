# Template: Stakeholder Map

## Purpose

The engagement-level relationship view: who matters, where they sit, how to work with each. A **living doc** in the project's `knowledge/`. It **references** the global people entities rather than duplicating them — durable identity lives once globally; this map holds the engagement-specific role, influence, and strategy.

## Inputs

- The locked `charter` (named stakeholders).
- The global people entities (`library/context/people/{slug}/`) where they exist — referenced by slug, not copied.

## Sections

- **Stakeholder table** — per person: `name` (link to the global entity by slug) · role on this engagement · influence (high/med/low) · interest (high/med/low) · engagement strategy (one line: how to keep them informed/bought-in).
- **Map notes** — relationships that matter (who defers to whom, where tension sits, the real decision-maker vs the titular one).

## Hard Constraints

- Every row links to or names a person; no anonymous "the finance team" rows for individuals who matter.
- Influence and interest are filled — they drive the engagement strategy.
- The per-engagement role/detail lives here; durable identity stays in the global people entity (no duplication).

## Draft Frontmatter Convention

```yaml
---
status: drafting   # living doc — stays drafting through the engagement
last_updated: <ISO-8601 timestamp>
---
```

## Lock Criteria

Living — does not lock. Current when every charter-named stakeholder has a row with influence, interest, and a strategy. Feeds the engagement's status cadence.
