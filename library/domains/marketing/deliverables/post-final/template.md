# Template: Post FINAL

## Purpose

The post's assembly record. Each post is built from ingredient deliverables (slide copy, body copy, image prompts, video—whatever the campaign manifest names), each with its own version trail and readiness gate. `post-FINAL.md` is where ready ingredients accumulate: shaped over time, complete when every manifest ingredient has landed, and immutable once the post publishes.

## Inputs

- The campaign's `post_manifest` in `project.md` (which ingredients this post is assembled from)
- The ready head version of each ingredient as it becomes ready

## Artifact Shape

One file per post: `post-FINAL.md` in the post folder, created in the same turn the post's first ingredient starts drafting. The campaign tracker's `post-{n}.file` points here from then on. Not versioned—the ingredient files carry the version trails; this file holds assembled ready content and then the immutable published receipt.

- One section per manifest ingredient, copied verbatim from the ready head version in the ready turn (the ready-event procedure owns this step).
- Ingredients not yet ready simply aren't there yet.

## Draft Frontmatter Convention

```yaml
status: <drafting | ready | published>
last_updated: <ISO-8601 date>
```

`drafting` while ingredients are still landing; `ready` when every manifest ingredient is in; `published` after publish.

## Publish / Export Mechanics

When the operator confirms the live URL ("posted post-1, here's the link"):

1. **Land shipped media.** Copy every asset that actually shipped into the post folder's `media/` directory before the state flips.
2. **Run the button.** `python system/af.py publish <campaign> <post> --url <url> [--posted-at <iso>] [--media <path> ...]` — it owns the mechanics atomically: publish block in this file's frontmatter (`shipped_at`, `published.{platform,url,posted_at}`, `shipped_media[]`), tracker row to `published`, derived published-post receipt across tracker + archive, project `shipped_at` on first publish, and the `post_published` activity event.
3. **Reconcile before the immutable transition.** Compare the live result with the ready assembly before running the button. If it differs materially, create a new tracked edition and reconcile there; never mutate a published `post-FINAL.md`.

## Readiness Criteria

- Every ingredient named by the manifest for this post is ready and present.
- Cross-check across ingredients: the body copy doesn't retell the slides, the cover aligns with the hook, CTA appears once and in the right place.
