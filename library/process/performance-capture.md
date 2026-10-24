# Performance Capture

## Purpose

Own the post-publication metrics procedure and the raw CSV shape that retros and hook scoring read. Packs and flows name where the CSV lives; this file owns how a row gets filled.

## When To Load

Load around 14 days after a post's `published.posted_at` (or `posted_at` on a studio post, where `af doctor studio` raises the nudge), and at a flow's performance-capture step. For multi-post projects, capture early posts while later posts are still in production, before system retro, or between system retro and project retro.

## Procedure

For each platform named in published frontmatter:

1. Check the connected MCP servers for read-only analytics or content tools for that platform and use the smallest one that returns the post's metrics.
2. If the connector returns only partial metrics, write the partial row and leave unsupported fields blank with a note such as `unknown, not zero`.
3. Ask the operator only for the missing fields the retro or hook scoring needs.

Keep the CSV raw and MECE:

```csv
post_id,platform,post_url,content_type,posted_at,captured_at,window,primary_exposure,likes,comments,shares,saves,clicks,avg_watch_time_seconds,total_watch_time_seconds,source,notes
```

Use `primary_exposure` for the platform's best top-level distribution number (LinkedIn impressions, TikTok views, Instagram reach or views). Leave unsupported cells blank. Do not store derived fields such as engagement rate in the raw CSV; calculate them in the retro if needed.

## Verification Or Logging

Every captured row names its `source` (connector tool or `operator`) and `captured_at`. A studio post stamps `metrics_captured_at` in its `post.md` once its row lands.

## Boundaries

- Publishing mechanics belong to the channel's own process file or pack runbook, and published state to `af publish` / `af studio post`.
- Scoring belongs to the closeout retro template or the pack's runbook (hook scoring in `hooks.md` for studio posts).
