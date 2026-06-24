# Composio Notes

Canonical Composio/Rube behavior for AgentFrame Marketing. Owns publish-prep and performance-capture procedures plus durable platform quirks. Loaded by any flow phase that wants to coordinate with connected tools.

## Publish Prep

When a post's copy is locked and publish media exists or has been selected, offer to prepare the draft/post through connected tools. If the operator accepts, ask the live Composio/Rube MCP surface what publishing tools are connected for the target platform and use the smallest tool that produces a draft.

Publish coordination is manual in POC (no auto-publishing). Connected tools can prepare drafts when available, but draft prep is not delivered state. When the operator publishes and provides the live URL plus the media that actually went out, reconcile the post's canonical `-v{N}.md` per the relevant deliverable template, increment `posts_published`, and append a `post_published` entry to `activity.md`.

LinkedIn via Composio can prepare text/image drafts; PDF/document carousels stay manual: prepare the text, then have the operator upload the PDF in LinkedIn.

## Performance Capture

Performance capture is connector-first, then manual gap-fill. Nudge around 14 days after each post's `published.posted_at`; for multi-post campaigns, capture can happen for early delivered posts while later posts are still in production, before system retro, or between system retro and campaign retro.

For each platform named in delivered frontmatter:

1. Ask the live Composio/Rube MCP surface what tools are connected for that platform and try the smallest read-only analytics/content tools available.
2. If the connector returns only partial metrics, write the partial row and mark unsupported fields blank with a note such as `unknown, not zero`.
3. Ask the operator only for the missing fields needed for Campaign Retro.

Keep the CSV raw and MECE:

```csv
post_id,platform,post_url,content_type,posted_at,captured_at,window,primary_exposure,likes,comments,shares,saves,clicks,avg_watch_time_seconds,total_watch_time_seconds,source,notes
```

Use `primary_exposure` for the platform's best top-level post distribution number (for example LinkedIn impressions, TikTok views, or Instagram reach/views). Leave unsupported cells blank. Do not store derived fields such as engagement rate in the raw CSV; calculate them in the campaign retro if needed.

If platform mapping needs a durable note, keep it tiny (LinkedIn / Instagram / TikTok bullets only) under "Platform Quirks" below, not an analytics framework.

## Platform Quirks

When a Composio run exposes a durable platform/tool limitation, append the smallest reusable note here: platform, limitation, workaround, date observed.

### LinkedIn

- 2026-05-09: Personal-post performance capture exposed only reaction totals via `LINKEDIN_LIST_REACTIONS`; impressions, comments, shares/reposts, and clicks were not available through the connected tools. Saved public share URLs may not contain the API-usable activity ID; if direct URN calls fail, fetch the public page and use the canonical `activity` ID from LinkedIn's sign-in redirect, then mark unavailable CSV fields as `unknown, not zero`.
