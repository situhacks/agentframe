---
schema_version: 1
created_at: {date}
last_activity: {ts}
applications: {{}}
---

# Job Pipeline

The board. Frontmatter `applications:` rows are the single owner of funnel stage state — `af pipe` verbs write them, `af pipe board` renders them, `af doctor` reconciles them against `applications/` folders.

Stage flow: `saved → preparing → applied → interviewing → offer → accepted | declined`, with `rejected | ghosted | dropped` as the other exits. `af pipe close-search` ends the campaign: it drops every live row with one reason and stamps `search_status: closed` here; the next `af pipe save` reopens it.

## Search

The active campaign lives in `library/context/operator/career/search-profile.md` (targets, dealbreakers, and the watchlist that drives `job-scout` sweeps). Watch rule: any row with a deadline inside 14 days is the next action.

## Notes
