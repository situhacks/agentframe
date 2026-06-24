# Template: Slide Copy

## Purpose

The carousel's slide-by-slide text — the copy, a few notes, nothing else. This is what the operator reads top-to-bottom and approves. It locks before body copy drafts and before imagery work consumes its word counts.

## Inputs

- **Campaign Architecture** locked (this post's job in the arc, hook angle, callbacks)
- **Voice**: mandatory — load the voice system at `library/context/operator/voice/README.md` before drafting
- **Design Language** only when slide text interacts with the visual treatment (text-over-image contrast, title placement)

## Artifact Shape

One block per slide:

- **Title** (when the slide has one) and **body text**, written exactly as they will appear on the slide.
- One optional note line per slide when a reviewer needs it (weight target, what this slide must not touch).

Options the operator is choosing between stay in the file until narrowed; remove the losers once picked.

## Hard Constraints

- Slide jobs are agreed before full prose is written. No two slides own the same job; each slide's content stays inside its job.
- The cover carries the complete thesis as flowing prose — not a headline plus mystery.
- Weight curve is managed: density alternates across the deck, the heaviest slide is the load-bearing argument beat, and no two adjacent slides sit at similar density.
- One recurring load-bearing noun threads the deck at structural anchors; the closer mirrors the cover (bookends).
- Slide text stays light. Nuance that doesn't fit moves to body copy — which drafts after this locks.
- This file carries copy only. Prompts and imagery live in the post's image-prompts file; the platform caption lives in body copy.

## Draft Frontmatter Convention

`status` + `last_updated` per [`library/process/deliverable-versioning.md`](../../../../process/deliverable-versioning.md). File: `slide-copy-v{N}.md` in the post folder named by the active flow.

## Lock Criteria

- Operator approves the full deck read top-to-bottom.
- Humanizer pass run on slide prose per [`library/process/humanizer-integration.md`](../../../../process/humanizer-integration.md).
- Lock-event mechanics per [`library/process/lock-event.md`](../../../../process/lock-event.md); the locked copy lands in the post's `post-FINAL.md` in the lock turn.
