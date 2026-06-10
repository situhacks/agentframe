# Template: Post FINAL

## Purpose

The post's assembly record. Each post is built from ingredient deliverables (slide copy, body copy, image prompts, video — whatever the campaign manifest names), each with its own version trail and lock. `post-FINAL.md` is where the locked ingredients accumulate: shaped over time, complete when every manifest ingredient has landed, and finally the as-shipped record once the post publishes.

## Inputs

- The campaign's `post_manifest` in `campaign.md` (which ingredients this post is assembled from)
- The locked head version of each ingredient as it locks

## Artifact Shape

One file per post: `post-FINAL.md` in the post folder, created in the same turn the post's first ingredient starts drafting. The campaign tracker's `post-{n}.file` points here from then on. Not versioned — the ingredient files carry the version trails; this file only ever holds locked content.

- One section per manifest ingredient, copied verbatim from the locked head version in the lock turn (the lock-event procedure owns this step).
- Ingredients not yet locked simply aren't there yet.

## Draft Frontmatter Convention

```yaml
status: <drafting | locked | shipped>
last_updated: <ISO-8601 date>
```

`drafting` while ingredients are still landing; `locked` when every manifest ingredient is in; `shipped` after publish.

## Publish / Export Mechanics

When the operator confirms the live URL ("posted post-1, here's the link"), update this file in one turn:

1. **Reconcile shipped copy.** If what shipped differs materially from the locked ingredient, write the next ingredient version with the as-shipped text, re-lock it, and refresh its section here. If verbatim, no change.
2. **Voice fallback.** If copy materially differed, run the publish/back-fill fallback per [`library/process/voice-mini-retro.md`](../../process/voice-mini-retro.md).
3. **Record the publish block** in this file's frontmatter:

```yaml
shipped_at: <ISO-8601 date>
published:
  platform: <linkedin | x | substack | ...>
  url: <full permalink>
  posted_at: <ISO-8601 datetime with timezone>
shipped_media:
  - visuals/post-1-cover-final.png
```

4. **Tracker and activity updates.** Set the tracker row to `status: shipped`, bump `posts_published`, set lifecycle `shipped_at` if this is the campaign's first publish, and append a `post_published` event to `activity.md` — all in the same turn.

## Lock Criteria

- Every ingredient named by the manifest for this post is locked and present.
- Cross-check across ingredients: the body copy doesn't retell the slides, the cover aligns with the hook, CTA appears once and in the right place.
