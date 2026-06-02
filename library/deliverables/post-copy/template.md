# Template: Post Copy

## Purpose

The actual post text the user publishes. Hook, body, CTA, alt text. Highlight reel — does NOT repeat what the carousel already shows. Earns its CTA. This is the most voice-heavy deliverable in the system.

## Inputs

- **Campaign Architecture** locked (post role, hook angle, callbacks)
- **Campaign Architecture audience extraction** locked (language tells, top objections, disconfirmation)
- **Voice Loading**: Mandatory. Load the voice system at `library/context/operator/voice/` (always-load `identity.md`, `anti-patterns.md`, `voice-profile.md`; retrieve the matching `pairs/` by register). `positioning.md` IS loaded. This is the most voice-heavy deliverable in the system.
- **Research Artifact** (for citation-backed claims)
- **Visual/Video Spec**: Carousel Spec or Video Spec at least drafted when the post has a visual/video component (so copy doesn't duplicate what the visual/video already carries) — soft dependency, can iterate in parallel.

## Sections

Per post:

- **Hook** (2-3 lines max) — first thing the reader sees. Specific claim or specific observation, never a platitude. Earns the rest.
- **Body** — flowing prose, no bullets. The hand-off from hook to payoff. Highlights what makes this campaign matter; does not narrate the carousel. For framework/explainer posts, include at least one concrete working example from the campaign or system.
- **CTA** — context-appropriate. Match the post's role from Campaign Architecture (tease / prove / ask / callback).
- **Alt text** — for the cover slide and any image. Says WHY the image matters, not "image of...".
- **Hook variants** (2-3) — included in the draft so the user picks. Not a separate file. The recommended pick is named.

## Hard Constraints

- No banned words from `voice/anti-patterns.md` (or banned word flagged + user explicitly overrode)
- Hook makes a specific claim or observation (not a platitude)
- Hook-first — first 2 lines earn the rest
- No bullets in prose (flowing sentences)
- Copy does NOT duplicate carousel/image content — copy + visuals divide labour
- Keep the artifact lean: write the post, do not restate upstream campaign direction, goals, audience, or strategy that already live in Phase 1/2/3 deliverables.
- The head copy file (`copy-v{N}.md`, highest `N`) owns the full LinkedIn caption, recommended hook, body, and CTA when a separate copy file exists. For carousel-first posts, the operator may choose a combined carousel-and-caption package where the head `carousel-spec-v{N}.md` owns the caption and publish record.
- CTA matches post's role per Campaign Architecture
- Reads as user's voice (per `voice/voice-profile.md` and `voice/pairs/`)
- Canadian English where applicable
- Ties back to a content pillar (one identifiable from `positioning.md`)
- Claims that cite external facts have a source in the campaign's Research Artifact
- 2-3 hook variants surfaced; the weakest variant is not the recommended pick

## Draft Frontmatter Convention

Standard deliverable frontmatter per [`library/process/campaign-frontmatter.md`](../../process/campaign-frontmatter.md). Versioning shape (filenames, head selection, iteration flow) lives in [`library/process/deliverable-versioning.md`](../../process/deliverable-versioning.md).

Template-specific edit-shape examples:
- Typo fixes, CTA wording swaps, shipped-state frontmatter updates, and small paragraph edits are surgical edits (no new version file).
- A net-new caption structure, full-body rewrite, or new post angle is a replacement (writes the next `copy-v{N+1}.md`).

## Lock Criteria

- Final version selected from variants
- Framework/explainer posts include a concrete working example, not only abstract mechanism labels
- Cross-checked vs the relevant visual/video artifact: carousel cover or video opening aligns with the hook; CTA placement aligns; no body↔slide/video duplication
- User-approved
- Standard lock-event mechanics apply per [`library/process/lock-event.md`](../../process/lock-event.md)
- For video posts: final or publish-candidate render path recorded in the head `video-spec-v{N}.md`

## Review Path

- **Required before downstream**: post can publish only when Copy + the relevant visual/video artifacts are locked and cross-checked for hook/CTA/no-duplication alignment. For carousel posts this means Carousel Spec and any Image Prompt. For video posts this means Video Spec and the final render path.

## Humanizer Pass

Required. See [`library/process/humanizer-integration.md`](../../process/humanizer-integration.md). Highest-stakes deliverable for AI-tells (copy goes to the public feed); the lock gate maps to publish.

## Publish / Export Mechanics

When the operator publishes the post and tells the agent ("posted post-1, here's the link"), the agent updates the head `copy-v{N}.md` in one turn:

1. **Reconcile final copy.** Treat differences from locked copy as a replacement (write the next `copy-v{N+1}.md` with the shipped copy). If verbatim, no new version.
2. **Voice fallback.** If copy materially differs, run publish/back-fill fallback per [`library/process/voice-mini-retro.md`](../../process/voice-mini-retro.md).
3. **Connect shipped media.** Record shipped media files in the head `copy-v{N}.md` frontmatter (relative to post folder).
4. **Update frontmatter to shipped state.** Add the publish record and media references to standard frontmatter:
```yaml
shipped_at: <ISO-8601 date>
published:
  platform: <linkedin | x | substack | ...>
  url: <full permalink>
  posted_at: <ISO-8601 datetime with timezone>
shipped_media:
  - visuals/post-1-cover-final.png
```
5. **Tracker & Activity updates.** Update campaign tracker (`status: shipped`, bump `posts_published`, update `shipped_at` if first post) and append a `post_published` event to `activity.md`.
6. **Performance capture.** Use Composio/connector tools per [`library/process/composio-notes.md`](../../process/composio-notes.md) to append raw, post-level metrics to `phase-5-launch-and-learn/performance-data.csv`. Record `source` precisely.

## Exceptions / Branches

- **Trending hook opportunity mid-campaign**: Copywriting can propose an out-of-arc post, but flag back to Campaign Architecture for arc integrity. Do not write it without the brief catching up.
- **Campaign Architecture has no hook angle for this post**: Stop. Route upstream to resolve the gap. Do not make it up here.
- **User picks the weakest variant**: Push back ONCE with reasoning, then honor the pick (Anti-sycophancy heuristic 5).