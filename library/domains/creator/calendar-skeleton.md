---
schema_version: 1
created_at: {date}
last_activity: {ts}
slots: {{}}
---

# Studio Calendar

The board. Frontmatter `slots:` rows are the single owner of post state: `af studio` verbs write them and `af doctor studio` reconciles them against `posts/` folders. One row per post, keyed by the post slug: `date` · `platform` · `state` · `series` · `hook` · `link`.

State flow: `planned → captured → cut → scheduled → posted`, terminal `dropped`. Measurement is a field on the post (`metrics_captured_at`), not a state.

## Channel

The why, the niche, the audience, the formats and the cadence live in `studio.md` beside this file. Read that on a fresh context or a strategy question; read this file for what is coming up and what shipped.

## Notes
