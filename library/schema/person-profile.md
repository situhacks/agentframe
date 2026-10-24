# Shape: Person Profile

The generic shape for a stakeholder/person. Tracks identity, persistent relationships, and project-specific roles across all engagements.

## Frontmatter Schema

```yaml
---
name: <human-readable full name>
title: <global job title / organization>
email: <email address>
type: person
---
```

## Sections

- **Global Identity & Relationship** — Core background, persistent preferences, communication styles, and ongoing context.
- **Engagement History** — Dated timeline of roles, contributions, and links to per-project stakeholder overlays.
  - `{YYYY-MM-DD} — {Project Name} (Role: {Role})` -> Link to `workspace/projects/{slug}/knowledge/people/{person-slug}.md`
